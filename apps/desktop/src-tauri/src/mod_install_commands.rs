use super::*;

#[derive(Clone, Debug)]
struct CurrentInstalledMod {
    version_id: Option<String>,
    display_name: String,
    file_path: String,
    enabled: bool,
    pinned: bool,
}

#[tauri::command]
pub(super) async fn instance_mod_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallModRequest,
) -> Result<InstallJobSummary, AppError> {
    instance_mod_install_inner(state.inner(), request).await
}

pub(super) async fn instance_mod_install_inner(
    state: &DesktopState,
    request: InstallModRequest,
) -> Result<InstallJobSummary, AppError> {
    refresh_exited_sessions(state).await;
    if request.mods.is_empty() || request.mods.len() > 50 {
        return Err(AppError::new(
            "mod.invalid_selection",
            "Select between 1 and 50 mods to install at once.",
        ));
    }
    let mut requested = HashSet::new();
    for selection in &request.mods {
        let display_name = selection.display_name.trim();
        let project_id = selection.project_id.trim();
        if selection.provider == slate_modpack_api_contracts::Provider::Ftb
            || display_name.is_empty()
            || display_name.chars().count() > 160
            || display_name.chars().any(char::is_control)
            || project_id.is_empty()
            || project_id.len() > 128
            || !requested.insert(format!("{}:{project_id}", selection.provider))
        {
            return Err(AppError::new(
                "mod.invalid_selection",
                "The selected mod list contains an invalid or duplicate project.",
            ));
        }
    }
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let installed = installed_mod_index(state, &instance).await?;
    let (loader, loader_version) = instance_mod_target(&instance)?;
    let mut merged_plan: Option<InstallPlan> = None;
    let mut pending_by_identity = BTreeMap::<String, NewInstanceMod>::new();
    let mut planned_destinations = BTreeMap::<String, PlannedModDestination>::new();
    let mut dependency_sets = Vec::new();
    for selection in &request.mods {
        let root_identity = format!("{}:{}", selection.provider, selection.project_id.trim());
        let mut dependencies = BTreeMap::<String, InstanceModDependency>::new();
        if installed.identities.contains(&root_identity) {
            return Err(AppError::new(
                "mod.already_installed",
                format!(
                    "{} is already installed in this instance.",
                    selection.display_name
                ),
            ));
        }
        let mut plan = state
            .modpacks
            .mod_install_plan(
                selection.provider,
                selection.project_id.trim(),
                &ModInstallPlanRequest {
                    minecraft_version: instance.minecraft_version.clone(),
                    loader,
                    loader_version: Some(loader_version.clone()),
                    version_id: None,
                },
            )
            .await
            .map_err(modpack_api_error)?;
        validate_instance_mod_plan(
            &instance,
            selection.provider,
            selection.project_id.trim(),
            &plan,
        )?;
        let mut accepted_downloads = Vec::new();
        for download in plan.downloads {
            let (provider, project_id, version_id) = parse_mod_download_id(&download.id)
                .ok_or_else(|| {
                    AppError::new(
                        "mod.invalid_plan",
                        "The mod service returned an invalid dependency identity.",
                    )
                })?;
            let dependency_identity = format!("{provider}:{project_id}");
            if dependency_identity != root_identity {
                dependencies.insert(
                    dependency_identity.clone(),
                    InstanceModDependency {
                        provider,
                        project_id: project_id.clone(),
                    },
                );
            }
            if installed.identities.contains(&dependency_identity) {
                continue;
            }
            if let Some(existing) = pending_by_identity.get_mut(&dependency_identity) {
                if existing.version_id != version_id {
                    return Err(AppError::new(
                        "mod.dependency_conflict",
                        "The selected mods require conflicting versions of the same dependency.",
                    ));
                }
                if dependency_identity == root_identity {
                    existing.display_name = selection.display_name.trim().to_owned();
                }
                continue;
            }
            let display_name = if dependency_identity == root_identity {
                selection.display_name.trim().to_owned()
            } else {
                let file_name = download
                    .destination
                    .rsplit('/')
                    .next()
                    .unwrap_or(project_id.as_str());
                file_name
                    .strip_suffix(".jar")
                    .unwrap_or(file_name)
                    .to_owned()
            };
            let normalized_destination = normalized_content_path(&download.destination);
            if let Some(existing) = installed.artifacts.get(&normalized_destination) {
                if !installed_artifact_matches(existing, download.size, &download.hashes) {
                    return Err(AppError::new(
                        "mod.file_conflict",
                        format!(
                            "{} conflicts with an existing mod file named {}.",
                            selection.display_name, download.destination
                        ),
                    ));
                }
                pending_by_identity.insert(
                    dependency_identity,
                    NewInstanceMod {
                        provider,
                        project_id,
                        version_id,
                        display_name,
                        file_path: download.destination,
                        hashes: download.hashes,
                        enabled: true,
                        pinned: false,
                    },
                );
                continue;
            }
            if let Some(existing) = planned_destinations.get(&normalized_destination) {
                if !same_mod_artifact(
                    existing.size,
                    &existing.hashes,
                    download.size,
                    &download.hashes,
                ) {
                    return Err(AppError::new(
                        "mod.file_conflict",
                        format!(
                            "{} and {} resolve to different files named {}. Deselect one and try again.",
                            existing.requested_name, selection.display_name, download.destination
                        ),
                    ));
                }
                pending_by_identity.insert(
                    dependency_identity,
                    NewInstanceMod {
                        provider,
                        project_id,
                        version_id,
                        display_name,
                        file_path: download.destination,
                        hashes: download.hashes,
                        enabled: true,
                        pinned: false,
                    },
                );
                continue;
            }
            planned_destinations.insert(
                normalized_destination,
                PlannedModDestination {
                    size: download.size,
                    hashes: download.hashes.clone(),
                    requested_name: selection.display_name.trim().to_owned(),
                },
            );
            pending_by_identity.insert(
                dependency_identity,
                NewInstanceMod {
                    provider,
                    project_id,
                    version_id,
                    display_name,
                    file_path: download.destination.clone(),
                    hashes: download.hashes.clone(),
                    enabled: true,
                    pinned: false,
                },
            );
            accepted_downloads.push(download);
        }
        dependency_sets.push(NewInstanceModDependencySet {
            root_provider: selection.provider,
            root_project_id: selection.project_id.trim().to_owned(),
            dependencies: dependencies.into_values().collect(),
        });
        plan.downloads = accepted_downloads;
        if let Some(merged) = &mut merged_plan {
            merged.downloads.extend(plan.downloads);
        } else {
            merged_plan = Some(plan);
        }
    }
    let mut plan = merged_plan.ok_or_else(|| {
        AppError::new(
            "mod.already_installed",
            "Every selected mod and dependency is already installed.",
        )
    })?;
    if plan.downloads.is_empty() || pending_by_identity.is_empty() {
        return Err(AppError::new(
            "mod.already_installed",
            "Every selected mod and dependency is already installed.",
        ));
    }
    plan.total_download_size = plan.downloads.iter().try_fold(0_u64, |total, download| {
        total.checked_add(download.size).ok_or_else(|| {
            AppError::new(
                "mod.invalid_plan",
                "The selected mod download size is invalid.",
            )
        })
    })?;
    queue_instance_install(
        state,
        instance,
        request.expected_revision,
        Some(plan),
        PendingModChanges {
            installed: pending_by_identity.into_values().collect(),
            replaced_paths: Vec::new(),
            dependency_sets,
        },
        None,
        RetryableInstallOperation::ModInstall { mods: request.mods },
    )
    .await
}

#[tauri::command]
pub(super) async fn instance_mod_update(
    state: tauri::State<'_, DesktopState>,
    request: UpdateInstanceModRequest,
) -> Result<InstallJobSummary, AppError> {
    instance_mod_update_inner(state.inner(), request).await
}

pub(super) async fn instance_mod_update_inner(
    state: &DesktopState,
    request: UpdateInstanceModRequest,
) -> Result<InstallJobSummary, AppError> {
    refresh_exited_sessions(state).await;
    validate_instance_mod_reference(Some(request.provider), Some(&request.project_id))?;
    let display_name = request.display_name.trim();
    if display_name.is_empty()
        || display_name.chars().count() > 160
        || display_name.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "mod.invalid_reference",
            "That installed mod has invalid details.",
        ));
    }
    let requested_path = ManagedRelativePath::parse(request.file_path.trim())
        .map_err(|_| {
            AppError::new(
                "mod.invalid_reference",
                "That installed mod has an invalid file path.",
            )
        })?
        .to_string();
    if !requested_path.starts_with("mods/") {
        return Err(AppError::new(
            "mod.invalid_reference",
            "That installed mod has an invalid file path.",
        ));
    }
    let target_version_id = request.target_version_id.trim();
    if target_version_id.is_empty()
        || target_version_id.len() > 128
        || !target_version_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(AppError::new(
            "mod.invalid_version",
            "Choose a valid mod version and try again.",
        ));
    }

    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let installed_records = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed mods."))?;
    let root_identity = format!("{}:{}", request.provider, request.project_id.trim());
    let stored_current = installed_records.iter().find(|record| {
        record.provider == request.provider && record.project_id == request.project_id.trim()
    });
    let current = if let Some(record) = stored_current {
        if normalized_content_path(&record.file_path) != normalized_content_path(&requested_path) {
            return Err(AppError::new(
                "mod.not_installed",
                "That mod is no longer installed at the selected location.",
            ));
        }
        CurrentInstalledMod {
            version_id: Some(record.version_id.clone()),
            display_name: record.display_name.clone(),
            file_path: record.file_path.clone(),
            enabled: record.enabled,
            pinned: record.pinned,
        }
    } else {
        let source = instance.modpack_source.as_ref().ok_or_else(|| {
            AppError::new(
                "mod.not_installed",
                "That mod is no longer installed in this instance.",
            )
        })?;
        let pack_version = state
            .modpacks
            .version(source.provider, &source.project_id, &source.version_id)
            .await
            .map_err(modpack_api_error)?;
        let normalized_requested = normalized_content_path(&requested_path);
        let version_id = pack_version.files.into_iter().find_map(|file| {
            let reference = file.source?;
            let normalized_pack_path = normalized_content_path(&file.path);
            let path_matches = normalized_pack_path == normalized_requested
                || format!("{normalized_pack_path}.disabled") == normalized_requested;
            (file.kind == PackFileType::Mod
                && path_matches
                && reference.provider == request.provider
                && reference.project_id == request.project_id.trim())
            .then_some(reference.version_id)
            .flatten()
        });
        if version_id.is_none() {
            return Err(AppError::new(
                "mod.not_installed",
                "That mod could not be matched to this modpack installation.",
            ));
        }
        CurrentInstalledMod {
            version_id,
            display_name: display_name.to_owned(),
            file_path: requested_path.clone(),
            enabled: !requested_path.ends_with(".disabled"),
            pinned: false,
        }
    };
    if current.version_id.as_deref() == Some(target_version_id) {
        return Err(AppError::new(
            "mod.version_unchanged",
            "That mod version is already installed.",
        ));
    }

    let installed_index = installed_mod_index(state, &instance).await?;
    if !installed_index
        .artifacts
        .contains_key(&normalized_content_path(&current.file_path))
    {
        return Err(AppError::new(
            "mod.not_installed",
            "That mod file is no longer present in this instance.",
        ));
    }
    let records_by_identity = installed_records
        .iter()
        .map(|record| (format!("{}:{}", record.provider, record.project_id), record))
        .collect::<HashMap<_, _>>();
    let (loader, loader_version) = instance_mod_target(&instance)?;
    let mut plan = state
        .modpacks
        .mod_install_plan(
            request.provider,
            request.project_id.trim(),
            &ModInstallPlanRequest {
                minecraft_version: instance.minecraft_version.clone(),
                loader,
                loader_version: Some(loader_version),
                version_id: Some(target_version_id.to_owned()),
            },
        )
        .await
        .map_err(modpack_api_error)?;
    validate_instance_mod_plan(
        &instance,
        request.provider,
        request.project_id.trim(),
        &plan,
    )?;
    if plan.instance.version_id != target_version_id {
        return Err(AppError::new(
            "mod.resolution_mismatch",
            "The selected mod version could not be confirmed. Nothing was changed.",
        ));
    }

    let mut root_found = false;
    let mut accepted_downloads = Vec::new();
    let mut pending_by_identity = BTreeMap::<String, NewInstanceMod>::new();
    let mut planned_destinations = BTreeMap::<String, PlannedModDestination>::new();
    let mut replaced_paths = BTreeSet::<String>::new();
    let mut dependencies = BTreeMap::<String, InstanceModDependency>::new();
    for mut download in plan.downloads {
        let (provider, project_id, version_id) =
            parse_mod_download_id(&download.id).ok_or_else(|| {
                AppError::new(
                    "mod.invalid_plan",
                    "The mod service returned an invalid dependency identity.",
                )
            })?;
        let identity = format!("{provider}:{project_id}");
        if identity == root_identity {
            root_found = true;
            if version_id != target_version_id {
                return Err(AppError::new(
                    "mod.resolution_mismatch",
                    "The selected mod version could not be confirmed. Nothing was changed.",
                ));
            }
        } else {
            dependencies.insert(
                identity.clone(),
                InstanceModDependency {
                    provider,
                    project_id: project_id.clone(),
                },
            );
        }
        let existing = records_by_identity.get(&identity).copied();
        let existing_version = if identity == root_identity {
            current.version_id.as_deref()
        } else {
            existing
                .map(|record| record.version_id.as_str())
                .or_else(|| installed_index.versions.get(&identity).map(String::as_str))
        };
        if existing_version == Some(version_id.as_str()) {
            continue;
        }
        let pinned = if identity == root_identity {
            current.pinned
        } else {
            existing.is_some_and(|record| record.pinned)
        };
        if pinned && identity != root_identity {
            return Err(AppError::new(
                "mod.dependency_pinned",
                format!(
                    "{} is pinned, but the selected version requires a different release.",
                    existing.map_or("A dependency", |record| record.display_name.as_str())
                ),
            ));
        }

        let existing_path = if identity == root_identity {
            Some(current.file_path.as_str())
        } else {
            existing
                .map(|record| record.file_path.as_str())
                .or_else(|| installed_index.paths.get(&identity).map(String::as_str))
        };
        let enabled = if identity == root_identity {
            current.enabled
        } else {
            existing
                .map(|record| record.enabled)
                .unwrap_or_else(|| existing_path.is_none_or(|path| !path.ends_with(".disabled")))
        };
        if !enabled && !download.destination.ends_with(".disabled") {
            download.destination.push_str(".disabled");
        }
        let destination = normalized_content_path(&download.destination);
        let replaced_destination = existing_path
            .map(normalized_content_path)
            .is_some_and(|path| path == destination);
        if let Some(installed_artifact) = installed_index.artifacts.get(&destination)
            && !replaced_destination
            && !installed_artifact_matches(installed_artifact, download.size, &download.hashes)
        {
            return Err(AppError::new(
                "mod.file_conflict",
                format!(
                    "The selected version conflicts with an existing mod file named {}.",
                    download.destination
                ),
            ));
        }
        if let Some(planned) = planned_destinations.get(&destination) {
            if !same_mod_artifact(
                planned.size,
                &planned.hashes,
                download.size,
                &download.hashes,
            ) {
                return Err(AppError::new(
                    "mod.file_conflict",
                    format!(
                        "The selected version resolves to conflicting files named {}.",
                        download.destination
                    ),
                ));
            }
        } else {
            planned_destinations.insert(
                destination,
                PlannedModDestination {
                    size: download.size,
                    hashes: download.hashes.clone(),
                    requested_name: current.display_name.clone(),
                },
            );
            accepted_downloads.push(download.clone());
        }
        if let Some(path) = existing_path {
            replaced_paths.insert(path.to_owned());
        }
        let display_name = if identity == root_identity {
            current.display_name.clone()
        } else {
            existing.map_or_else(
                || {
                    let file_name = download
                        .destination
                        .rsplit('/')
                        .next()
                        .unwrap_or(project_id.as_str());
                    file_name
                        .strip_suffix(".disabled")
                        .unwrap_or(file_name)
                        .strip_suffix(".jar")
                        .unwrap_or(file_name)
                        .to_owned()
                },
                |record| record.display_name.clone(),
            )
        };
        pending_by_identity.insert(
            identity,
            NewInstanceMod {
                provider,
                project_id,
                version_id,
                display_name,
                file_path: download.destination,
                hashes: download.hashes,
                enabled,
                pinned,
            },
        );
    }
    if !root_found {
        return Err(AppError::new(
            "mod.invalid_plan",
            "The selected version did not include the requested mod.",
        ));
    }
    if accepted_downloads.is_empty() || pending_by_identity.is_empty() {
        return Err(AppError::new(
            "mod.version_unchanged",
            "That mod version is already installed.",
        ));
    }
    plan.downloads = accepted_downloads;
    plan.delete.extend(replaced_paths.iter().cloned());
    plan.delete.sort();
    plan.delete.dedup();
    plan.total_download_size = plan.downloads.iter().try_fold(0_u64, |total, download| {
        total.checked_add(download.size).ok_or_else(|| {
            AppError::new(
                "mod.invalid_plan",
                "The selected mod download size is invalid.",
            )
        })
    })?;

    queue_instance_install(
        state,
        instance,
        request.expected_revision,
        Some(plan),
        PendingModChanges {
            installed: pending_by_identity.into_values().collect(),
            replaced_paths: replaced_paths.into_iter().collect(),
            dependency_sets: vec![NewInstanceModDependencySet {
                root_provider: request.provider,
                root_project_id: request.project_id.trim().to_owned(),
                dependencies: dependencies.into_values().collect(),
            }],
        },
        None,
        RetryableInstallOperation::ModUpdate {
            provider: request.provider,
            project_id: request.project_id,
            file_path: request.file_path,
            display_name: request.display_name,
            target_version_id: target_version_id.to_owned(),
        },
    )
    .await
}
