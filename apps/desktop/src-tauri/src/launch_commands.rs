use super::*;

#[tauri::command]
pub(super) async fn instance_launch(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, DesktopState>,
    request: LaunchInstanceRequest,
) -> Result<GameSessionSummary, AppError> {
    refresh_exited_sessions(state.inner()).await;
    let instance_id = InstanceId::from_uuid(request.id);
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Minecraft is already running for this instance.",
        ));
    }
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if instance.setup_state != slate_domain::InstanceSetupState::Ready {
        return Err(AppError::new(
            "local.instance_not_ready",
            "Install and verify this instance before launching it.",
        ));
    }
    let instance_paths = paths_for_instance(state.inner(), &instance);
    let log_root = instance.storage_path.clone();
    let retention_days = instance.settings.log_retention_days;
    let _ = tauri::async_runtime::spawn_blocking(move || {
        prune_instance_logs(&log_root, retention_days)
    })
    .await;
    let launch_account = state
        .database
        .account_for_launch(instance_id, request.account_id.map(AccountId::from_uuid))
        .await
        .map_err(|error| {
            map_storage_error(error, "Sign in to a Minecraft account before playing.")
        })?;
    if launch_account.status != AccountStatus::Ready {
        return Err(AppError::new(
            "auth.reauthentication_required",
            "This Minecraft account needs to sign in again before playing.",
        ));
    }
    let minecraft_session = refresh_minecraft_session(
        state.inner(),
        launch_account.id,
        launch_account.profile_id,
        &launch_account.credential_ref,
    )
    .await?;
    let revision = state
        .database
        .get_installed_revision(instance_id)
        .await
        .map_err(|error| {
            map_storage_error(error, "slate could not load the installed revision.")
        })?;
    let manifest_path = installed_manifest_path(&instance_paths, instance_id, revision.id);
    let installed = load_installed_revision(
        &manifest_path,
        instance_id,
        revision.id,
        &revision.manifest_digest,
    )
    .await
    .map_err(|_| {
        AppError::new(
            "local.install_manifest_invalid",
            "The installed revision manifest is missing, changed, or invalid. Reinstall the instance.",
        )
    })?;
    let layers = installed.metadata_layers;
    let runtime = installed.runtime;
    let installed_artifacts = installed.launch_artifacts;
    if runtime.executable != revision.runtime_executable
        || runtime.major_version != revision.runtime_major
    {
        return Err(AppError::new(
            "local.runtime_binding_changed",
            "The runtime binding changed after installation. Reinstall the instance.",
        ));
    }
    let managed_probe_path = runtime.executable.clone();
    let managed_probe =
        tauri::async_runtime::spawn_blocking(move || probe_java_executable(&managed_probe_path))
            .await
            .map_err(|_| {
                AppError::new(
                    "local.runtime_probe_unavailable",
                    "slate could not validate the managed Java runtime.",
                )
            })?;
    if !managed_probe.available || managed_probe.major_version != Some(runtime.major_version) {
        return Err(AppError::new(
            "local.runtime_invalid",
            "The managed Java runtime is missing or no longer matches this instance.",
        ));
    }

    let resolved = ResolvedVersion::resolve(layers).map_err(|_| {
        AppError::new(
            "local.install_manifest_invalid",
            "The installed metadata chain is invalid. Reinstall the instance.",
        )
    })?;
    let architecture = minecraft_architecture(&runtime.architecture)?;
    let selected_executable = match instance.settings.java_mode {
        JavaSelectionMode::Managed => runtime.executable.clone(),
        JavaSelectionMode::Detected | JavaSelectionMode::Custom => instance
            .settings
            .custom_java_path
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(java_selection_error)?,
    };
    let selected_probe_path = selected_executable.clone();
    let selected_probe =
        tauri::async_runtime::spawn_blocking(move || probe_java_executable(&selected_probe_path))
            .await
            .map_err(|_| java_selection_error())?;
    if !selected_probe.available
        || selected_probe.major_version != Some(resolved.java_version().major_version)
        || selected_probe.architecture.and_then(platform_architecture) != Some(architecture)
    {
        return Err(AppError::new(
            "local.runtime_invalid",
            "The selected Java runtime no longer matches this instance's required version and architecture.",
        ));
    }
    let layout = launch_layout(&instance_paths, instance_id, revision.id);
    let runtime_for_plan = JavaRuntime::new(
        selected_executable.clone(),
        resolved.java_version().major_version,
        architecture,
    )
    .map_err(|_| AppError::new("local.runtime_invalid", "The selected runtime is invalid."))?;
    let mut environment: BTreeMap<String, EnvironmentValue> =
        restricted_child_environment(Some(&selected_executable))
            .into_iter()
            .map(|(name, value)| (name, EnvironmentValue::public(value)))
            .collect();
    environment.extend(
        instance
            .settings
            .environment
            .iter()
            .map(|(name, value)| (name.clone(), EnvironmentValue::public(value.clone()))),
    );
    let custom_resolution = match instance.settings.window_mode {
        InstanceWindowMode::Windowed => instance
            .settings
            .resolution_width
            .zip(instance.settings.resolution_height),
        InstanceWindowMode::Maximized => window
            .current_monitor()
            .ok()
            .flatten()
            .map(|monitor| (monitor.size().width, monitor.size().height)),
        InstanceWindowMode::Fullscreen => None,
    };
    let quick_play = instance
        .settings
        .quick_play_server
        .as_ref()
        .map(|server| QuickPlay::Multiplayer(server.clone()));
    let additional_jvm_arguments = launch_jvm_arguments(&instance.settings);
    apply_managed_game_options(
        &layout.game_directory,
        &instance.settings.game_language,
        instance.settings.window_mode == InstanceWindowMode::Fullscreen,
    )
    .await?;
    let preparation = LaunchPlanner::prepare(
        &resolved,
        LaunchRequest {
            runtime: runtime_for_plan,
            layout,
            identity: LaunchIdentity::new(
                minecraft_session.profile.name.clone(),
                minecraft_session.profile.id,
                minecraft_session.access_token(),
                minecraft_session.client_id(),
                minecraft_session.xuid(),
            )
            .map_err(|_| {
                AppError::new(
                    "auth.minecraft_identity_invalid",
                    "Minecraft returned an identity that slate could not launch safely.",
                )
            })?,
            rules: current_rule_context(architecture),
            options: LaunchOptions {
                initial_memory_mib: instance.settings.initial_memory_mb,
                maximum_memory_mib: instance.memory_mb,
                additional_jvm_arguments,
                custom_resolution,
                demo_user: false,
                quick_play,
                ..LaunchOptions::default()
            },
            environment,
        },
    )
    .map_err(|_| {
        AppError::new(
            "local.launch_plan_invalid",
            "slate could not construct a valid launch plan for this revision.",
        )
    })?;
    verify_installed_launch_artifacts(
        state.paths.storage_root(),
        &preparation.required_artifacts,
        &installed_artifacts,
    )
    .await
    .map_err(|_| {
        AppError::new(
            "local.install_corrupt",
            "A required game file is missing, changed, or corrupt. Reinstall the instance.",
        )
    })?;

    let session_id = SessionId::new();
    state
        .database
        .create_session_starting(instance_id, revision.id, session_id, launch_account.id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not create the game session."))?;
    let log_path = state
        .paths
        .instance(instance_id)
        .join("logs")
        .join(format!("{session_id}.log"));
    let started = match state.processes.start(
        instance_id,
        session_id,
        &preparation.plan,
        log_path,
        child_process_priority(instance.settings.process_priority),
        &instance.settings.cpu_affinity,
    ) {
        Ok(started) => started,
        Err(error) => {
            let _ = state.database.fail_session_start(session_id).await;
            return Err(process_start_error(error));
        }
    };
    if let Err(error) = state
        .database
        .mark_session_running(session_id, started.pid)
        .await
    {
        let _ = state
            .processes
            .terminate_after_tracking_failure(instance_id);
        let _ = state.database.fail_session_start(session_id).await;
        return Err(map_storage_error(
            error,
            "The game was stopped because slate could not track its session.",
        ));
    }
    match instance.settings.launcher_behavior {
        LauncherBehavior::KeepOpen => {}
        LauncherBehavior::Minimize => {
            let _ = window.minimize();
        }
        LauncherBehavior::Hide => {
            let _ = window.hide();
            let state = state.inner().clone();
            tauri::async_runtime::spawn(async move {
                restore_window_after_session(state, window, session_id).await;
            });
        }
    }
    Ok(GameSessionSummary {
        id: session_id.as_uuid(),
        instance_id: instance_id.as_uuid(),
        state: GameSessionStateDto::Running,
        mode: "authenticated".to_owned(),
        pid: started.pid,
        log_name: started.log_path.file_name().map_or_else(
            || "session.log".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        ),
    })
}
