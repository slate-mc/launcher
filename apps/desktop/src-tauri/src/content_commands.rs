use super::*;

#[tauri::command]
pub(super) async fn modpack_providers(
    state: tauri::State<'_, DesktopState>,
) -> Result<ProvidersResponse, AppError> {
    state.modpacks.providers().await.map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn modpacks_search(
    state: tauri::State<'_, DesktopState>,
    request: ModpackSearchRequest,
) -> Result<SearchResponse, AppError> {
    state
        .modpacks
        .search(&SearchOptions {
            query: request.query,
            provider: request.provider,
            minecraft_version: request.minecraft_version,
            loader: request.loader,
            category: request.category,
            sort: match request.sort {
                ModpackSortDto::Relevance => SearchSort::Relevance,
                ModpackSortDto::Downloads => SearchSort::Downloads,
                ModpackSortDto::Updated => SearchSort::Updated,
                ModpackSortDto::Newest => SearchSort::Newest,
            },
            cursor: request.cursor,
            page: request.page,
            limit: request.limit.unwrap_or(20),
        })
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn mods_search(
    state: tauri::State<'_, DesktopState>,
    request: ModSearchRequest,
) -> Result<SearchResponse, AppError> {
    let instance = state
        .database
        .get_instance(InstanceId::from_uuid(request.instance_id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let (loader, _) = instance_mod_target(&instance)?;
    state
        .modpacks
        .search_mods(&SearchOptions {
            query: request.query,
            provider: request.provider,
            minecraft_version: Some(instance.minecraft_version),
            loader: Some(loader),
            category: None,
            sort: match request.sort {
                ModpackSortDto::Relevance => SearchSort::Relevance,
                ModpackSortDto::Downloads => SearchSort::Downloads,
                ModpackSortDto::Updated => SearchSort::Updated,
                ModpackSortDto::Newest => SearchSort::Newest,
            },
            cursor: request.cursor,
            page: request.page,
            limit: request.limit.unwrap_or(20),
        })
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn instance_mods_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceModsRequest,
) -> Result<Vec<InstanceModSummary>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let stored_mods = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load installed mods."))?;
    let paths = paths_for_instance(state.inner(), &instance);
    let files = tokio::task::spawn_blocking(move || scan_instance_mods(&paths, instance_id))
        .await
        .map_err(|_| {
            AppError::new(
                "local.mod_inventory_failed",
                "slate could not load the installed mods for this instance.",
            )
        })?
        .map_err(|_| {
            AppError::new(
                "local.mod_inventory_failed",
                "slate could not load the installed mods for this instance.",
            )
        })?;
    let mut stored_by_path = stored_mods
        .into_iter()
        .map(|record| (normalized_content_path(&record.file_path), record))
        .collect::<HashMap<_, _>>();
    let untracked_origin = if instance.modpack_source.is_some() {
        InstanceModOriginDto::Modpack
    } else {
        InstanceModOriginDto::Local
    };
    Ok(files
        .into_iter()
        .map(|file| {
            let record = stored_by_path.remove(&normalized_content_path(&file.file_path));
            instance_mod_summary(record, file, untracked_origin.clone())
        })
        .collect())
}

#[tauri::command]
pub(super) async fn instance_mod_import(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: ImportLocalModRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    ensure_local_mod_import_target(&instance)?;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Add a mod JAR")
            .add_filter("Minecraft mod", &["jar"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| local_mod_import_error(LocalContentImportError::InvalidArchive))?;
    let Some(source) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(instance_summary(instance));
    };

    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    ensure_local_mod_import_target(&instance)?;
    let loader = instance.loader_kind;
    let import_kind = LocalContentImportKind::Mod(loader);
    let paths = paths_for_instance(state.inner(), &instance);
    let paths_for_validation = paths.clone();
    let source_for_validation = source.clone();
    let validation_kind = import_kind.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), LocalContentImportError> {
        validate_local_content_source(&source_for_validation, &validation_kind)?;
        ensure_local_content_not_installed(
            &paths_for_validation,
            instance_id,
            &source_for_validation,
            &validation_kind,
        )
    })
    .await
    .map_err(|_| local_mod_import_error(LocalContentImportError::InvalidArchive))?
    .map_err(local_mod_import_error)?;
    create_automatic_snapshot_if_enabled(state.inner(), &instance).await?;

    let imported = tauri::async_runtime::spawn_blocking(move || {
        import_local_content(&paths, instance_id, &source, &import_kind)
    })
    .await
    .map_err(|_| local_mod_import_error(LocalContentImportError::InvalidArchive))?
    .map_err(local_mod_import_error)?;
    if let Err(error) = state
        .database
        .advance_instance_revision(instance_id, request.expected_revision)
        .await
    {
        rollback_imported_content(imported).await?;
        return Err(map_storage_error(
            error,
            "slate could not record the imported mod.",
        ));
    }
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not refresh the instance."))
}

fn ensure_local_mod_import_target(instance: &InstanceRecord) -> Result<(), AppError> {
    if instance.loader_kind == LoaderFamily::Vanilla {
        return Err(AppError::new(
            "mod.loader_required",
            "Choose Fabric or NeoForge before adding mod JARs.",
        ));
    }
    if instance.setup_state != slate_domain::InstanceSetupState::Ready {
        return Err(AppError::new(
            "local.instance_not_ready",
            "Finish installing or repair this instance before adding local mods.",
        ));
    }
    Ok(())
}

fn local_mod_import_error(error: LocalContentImportError) -> AppError {
    match error {
        LocalContentImportError::AlreadyInstalled => AppError::new(
            "mod.already_installed",
            "A mod JAR with that file name is already installed.",
        ),
        LocalContentImportError::WrongLoader => AppError::new(
            "mod.wrong_loader",
            "That JAR does not contain mod metadata for this instance's loader.",
        ),
        LocalContentImportError::TooLarge => AppError::new(
            "mod.file_too_large",
            "That mod JAR is larger than the 1 GB import limit.",
        ),
        LocalContentImportError::InvalidArchive => AppError::new(
            "mod.invalid_jar",
            "Choose a valid mod JAR for this instance's loader.",
        ),
        LocalContentImportError::Io(_) => AppError::new(
            "local.mod_import_failed",
            "slate could not safely copy that mod JAR.",
        ),
    }
}

async fn rollback_imported_content(imported: ImportedLocalFile) -> Result<(), AppError> {
    let rollback = tauri::async_runtime::spawn_blocking(move || imported.rollback()).await;
    if matches!(rollback, Ok(Ok(()))) {
        Ok(())
    } else {
        Err(AppError::new(
            "local.content_rollback_failed",
            "slate could not remove the copied content after the change failed. The instance needs attention.",
        ))
    }
}

#[tauri::command]
pub(super) async fn instance_worlds_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceWorldsRequest,
) -> Result<Vec<String>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let paths = paths_for_instance(state.inner(), &instance);
    tauri::async_runtime::spawn_blocking(move || scan_instance_worlds(&paths, instance_id))
        .await
        .map_err(|_| content_file_error())?
        .map_err(|_| content_file_error())
}

#[tauri::command]
pub(super) async fn instance_content_file_import(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: ImportLocalContentFileRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    local_pack_import_kind(&instance, request.kind, request.world_name.as_deref())?;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Add a content pack")
            .add_filter("Minecraft content pack", &["zip"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| local_pack_import_error(LocalContentImportError::InvalidArchive))?;
    let Some(source) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(instance_summary(instance));
    };

    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let import_kind =
        local_pack_import_kind(&instance, request.kind, request.world_name.as_deref())?;
    let paths = paths_for_instance(state.inner(), &instance);
    if let LocalContentImportKind::DataPack(world_name) = &import_kind {
        let paths_for_worlds = paths.clone();
        let worlds = tauri::async_runtime::spawn_blocking(move || {
            scan_instance_worlds(&paths_for_worlds, instance_id)
        })
        .await
        .map_err(|_| content_file_error())?
        .map_err(|_| content_file_error())?;
        if !worlds.contains(world_name) {
            return Err(AppError::new(
                "content.world_unavailable",
                "That world is no longer available. Choose another world and try again.",
            ));
        }
    }
    let paths_for_validation = paths.clone();
    let source_for_validation = source.clone();
    let validation_kind = import_kind.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), LocalContentImportError> {
        validate_local_content_source(&source_for_validation, &validation_kind)?;
        ensure_local_content_not_installed(
            &paths_for_validation,
            instance_id,
            &source_for_validation,
            &validation_kind,
        )
    })
    .await
    .map_err(|_| local_pack_import_error(LocalContentImportError::InvalidArchive))?
    .map_err(local_pack_import_error)?;
    create_automatic_snapshot_if_enabled(state.inner(), &instance).await?;

    let imported = tauri::async_runtime::spawn_blocking(move || {
        import_local_content(&paths, instance_id, &source, &import_kind)
    })
    .await
    .map_err(|_| local_pack_import_error(LocalContentImportError::InvalidArchive))?
    .map_err(local_pack_import_error)?;
    if let Err(error) = state
        .database
        .advance_instance_revision(instance_id, request.expected_revision)
        .await
    {
        rollback_imported_content(imported).await?;
        return Err(map_storage_error(
            error,
            "slate could not record the imported content pack.",
        ));
    }
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not refresh the instance."))
}

fn local_pack_import_kind(
    instance: &InstanceRecord,
    kind: InstanceContentKindDto,
    world_name: Option<&str>,
) -> Result<LocalContentImportKind, AppError> {
    if instance.setup_state != slate_domain::InstanceSetupState::Ready {
        return Err(AppError::new(
            "local.instance_not_ready",
            "Finish installing or repair this instance before adding content packs.",
        ));
    }
    match kind {
        InstanceContentKindDto::ResourcePack => {
            Ok(LocalContentImportKind::Pack(InstanceContentKind::Resource))
        }
        InstanceContentKindDto::ShaderPack => {
            Ok(LocalContentImportKind::Pack(InstanceContentKind::Shader))
        }
        InstanceContentKindDto::DataPack => {
            let world_name = world_name.filter(|value| {
                !value.is_empty()
                    && value.len() <= 240
                    && !matches!(*value, "." | "..")
                    && !value.contains(['/', '\\', ':'])
                    && !value.chars().any(char::is_control)
            });
            world_name
                .map(|value| LocalContentImportKind::DataPack(value.to_owned()))
                .ok_or_else(|| {
                    AppError::new(
                        "content.world_required",
                        "Choose a world before importing a data pack.",
                    )
                })
        }
    }
}

fn local_pack_import_error(error: LocalContentImportError) -> AppError {
    match error {
        LocalContentImportError::AlreadyInstalled => AppError::new(
            "content.already_installed",
            "A content pack with that file name is already installed.",
        ),
        LocalContentImportError::TooLarge => AppError::new(
            "content.file_too_large",
            "That content pack is larger than the 1 GB import limit.",
        ),
        LocalContentImportError::InvalidArchive | LocalContentImportError::WrongLoader => {
            AppError::new(
                "content.invalid_archive",
                "Choose a valid content pack ZIP for this section.",
            )
        }
        LocalContentImportError::Io(_) => AppError::new(
            "local.content_import_failed",
            "slate could not safely copy that content pack.",
        ),
    }
}

#[tauri::command]
pub(super) async fn instance_content_files_list(
    state: tauri::State<'_, DesktopState>,
    request: InstanceContentFilesRequest,
) -> Result<Vec<InstanceContentFileSummary>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let internal_kind = content_kind(request.kind);
    let paths = paths_for_instance(state.inner(), &instance);
    let files = tokio::task::spawn_blocking(move || {
        scan_instance_content(&paths, instance_id, internal_kind)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let mut managed_paths = HashSet::new();
    if let Some(source) = &instance.modpack_source {
        match state
            .modpacks
            .version(source.provider, &source.project_id, &source.version_id)
            .await
        {
            Ok(version) => {
                for file in version.files {
                    if pack_file_matches_content(file.kind, request.kind) {
                        managed_paths.insert(normalized_content_path(&file.path));
                    }
                }
            }
            Err(error) => tracing::warn!(
                instance_id = %instance_id,
                error = %error,
                "could not resolve pack-managed content paths"
            ),
        }
    }
    Ok(files
        .into_iter()
        .map(|file| content_file_summary(file, request.kind, &managed_paths))
        .collect())
}

#[tauri::command]
pub(super) async fn instance_content_file_set_enabled(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceContentFileEnabledRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let paths = paths_for_instance(state.inner(), &instance);
    let file_path = request.file_path.clone();
    let kind = content_kind(request.kind);
    let enabled = request.enabled;
    let file_move = tokio::task::spawn_blocking(move || {
        set_instance_content_enabled(&paths, instance_id, kind, &file_path, enabled)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let database_result = state
        .database
        .advance_instance_revision(instance_id, request.expected_revision)
        .await;
    finish_instance_content_change(
        state.inner(),
        instance_id,
        file_move,
        database_result,
        "slate could not update that content file.",
    )
    .await
}

#[tauri::command]
pub(super) async fn instance_content_file_remove(
    state: tauri::State<'_, DesktopState>,
    request: RemoveInstanceContentFileRequest,
) -> Result<InstanceSummary, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let paths = paths_for_instance(state.inner(), &instance);
    let file_path = request.file_path.clone();
    let kind = content_kind(request.kind);
    let file_move = tokio::task::spawn_blocking(move || {
        trash_instance_content(&paths, instance_id, kind, &file_path)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let database_result = state
        .database
        .advance_instance_revision(instance_id, request.expected_revision)
        .await;
    finish_instance_content_change(
        state.inner(),
        instance_id,
        file_move,
        database_result,
        "slate could not remove that content file.",
    )
    .await
}

#[tauri::command]
pub(super) async fn instance_mods_resolve(
    state: tauri::State<'_, DesktopState>,
    request: InstanceModsRequest,
) -> Result<Vec<InstanceModResolution>, AppError> {
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    let stored_mods = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load installed mods."))?;
    let mut references_by_path = stored_mods
        .into_iter()
        .map(|record| {
            (
                normalized_content_path(&record.file_path),
                ProviderReference {
                    provider: record.provider,
                    project_id: record.project_id,
                    version_id: Some(record.version_id),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    if let Some(source) = instance.modpack_source {
        match state
            .modpacks
            .version(source.provider, &source.project_id, &source.version_id)
            .await
        {
            Ok(version) => {
                for file in version.files {
                    if file.kind == PackFileType::Mod
                        && let Some(reference) = file.source
                        && reference.provider != slate_modpack_api_contracts::Provider::Ftb
                    {
                        references_by_path
                            .entry(normalized_content_path(&file.path))
                            .or_insert(reference);
                    }
                }
            }
            Err(error) => tracing::warn!(
                instance_id = %request.instance_id,
                error = %error,
                "could not load modpack sources for installed mod resolution"
            ),
        }
    }

    let unique_projects = references_by_path
        .values()
        .map(|reference| {
            (
                (reference.provider, reference.project_id.clone()),
                ModProjectReference {
                    provider: reference.provider,
                    project_id: reference.project_id.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let resolved = if unique_projects.is_empty() {
        Vec::new()
    } else {
        match state
            .modpacks
            .resolve_mods(&ResolveModsRequest {
                items: unique_projects,
            })
            .await
        {
            Ok(response) => response.items,
            Err(error) => {
                tracing::warn!(
                    instance_id = %request.instance_id,
                    error = %error,
                    "provider mod metadata resolution failed"
                );
                Vec::new()
            }
        }
    };
    let resolved_by_project = resolved
        .into_iter()
        .map(|project| ((project.provider, project.project_id.clone()), project))
        .collect::<BTreeMap<_, _>>();

    Ok(references_by_path
        .into_iter()
        .map(|(file_path, reference)| {
            let project =
                resolved_by_project.get(&(reference.provider, reference.project_id.clone()));
            InstanceModResolution {
                file_path,
                provider: reference.provider,
                project_id: reference.project_id,
                version_id: reference.version_id,
                display_name: project.map(|project| project.name.clone()),
                icon_url: project.and_then(|project| project.icon_url.clone()),
            }
        })
        .collect())
}

#[tauri::command]
pub(super) async fn instance_mod_set_enabled(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceModEnabledRequest,
) -> Result<InstanceSummary, AppError> {
    let reference =
        validate_instance_mod_reference(request.provider, request.project_id.as_deref())?;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let paths = paths_for_instance(state.inner(), &instance);
    let file_path = request.file_path.clone();
    let enabled = request.enabled;
    let file_move = tokio::task::spawn_blocking(move || {
        set_instance_mod_enabled(&paths, instance_id, &file_path, enabled)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let updated_file_path = file_move
        .updated_file_path
        .as_deref()
        .ok_or_else(content_file_error)?;
    let database_result = state
        .database
        .set_instance_mod_enabled(
            instance_id,
            request.expected_revision,
            InstanceModEnabledChange {
                target: InstanceModTarget {
                    provider: reference.map(|(provider, _)| provider),
                    project_id: reference.map(|(_, project_id)| project_id),
                    file_path: &request.file_path,
                },
                updated_file_path,
                enabled: request.enabled,
            },
        )
        .await;
    finish_instance_content_change(
        state.inner(),
        instance_id,
        file_move,
        database_result,
        "slate could not update that mod.",
    )
    .await
}

#[tauri::command]
pub(super) async fn instance_mod_remove(
    state: tauri::State<'_, DesktopState>,
    request: RemoveInstanceModRequest,
) -> Result<InstanceSummary, AppError> {
    let reference =
        validate_instance_mod_reference(request.provider, request.project_id.as_deref())?;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state.inner(), instance_id, request.expected_revision)
            .await?;
    let paths = paths_for_instance(state.inner(), &instance);
    let file_path = request.file_path.clone();
    let file_move =
        tokio::task::spawn_blocking(move || trash_instance_mod(&paths, instance_id, &file_path))
            .await
            .map_err(|_| content_file_error())?
            .map_err(|_| content_file_error())?;
    let database_result = state
        .database
        .remove_instance_mod(
            instance_id,
            request.expected_revision,
            InstanceModTarget {
                provider: reference.map(|(provider, _)| provider),
                project_id: reference.map(|(_, project_id)| project_id),
                file_path: &request.file_path,
            },
        )
        .await;
    finish_instance_content_change(
        state.inner(),
        instance_id,
        file_move,
        database_result,
        "slate could not remove that mod.",
    )
    .await
}

pub(super) fn validate_instance_mod_reference(
    provider: Option<slate_modpack_api_contracts::Provider>,
    project_id: Option<&str>,
) -> Result<Option<(slate_modpack_api_contracts::Provider, &str)>, AppError> {
    match (provider, project_id) {
        (None, None) => Ok(None),
        (Some(provider), Some(project_id))
            if provider != slate_modpack_api_contracts::Provider::Ftb
                && !project_id.is_empty()
                && project_id.len() <= 128
                && project_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) =>
        {
            Ok(Some((provider, project_id)))
        }
        _ => Err(AppError::new(
            "mod.invalid_reference",
            "The installed mod reference is invalid.",
        )),
    }
}

pub(super) async fn prepare_instance_content_change(
    state: &DesktopState,
    instance_id: InstanceId,
    expected_revision: u64,
) -> Result<InstanceRecord, AppError> {
    refresh_exited_sessions(state).await;
    if state
        .processes
        .active_for_instance(instance_id)
        .map_err(process_state_error)?
        .is_some()
    {
        return Err(AppError::new(
            "local.instance_running",
            "Stop Minecraft before changing this instance's content.",
        ));
    }
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    if instance.revision != expected_revision {
        return Err(AppError::new(
            "local.instance_changed",
            "That instance changed in another view. Reload it and try again.",
        ));
    }
    if instance.setup_state == slate_domain::InstanceSetupState::Preparing {
        return Err(AppError::new(
            "local.instance_preparing",
            "Wait for the current installation to finish before changing this instance's content.",
        ));
    }
    Ok(instance)
}

#[tauri::command]
pub(super) async fn instance_mod_set_pinned(
    state: tauri::State<'_, DesktopState>,
    request: SetInstanceModPinnedRequest,
) -> Result<InstanceSummary, AppError> {
    let reference =
        validate_instance_mod_reference(request.provider, request.project_id.as_deref())?
            .ok_or_else(|| {
                AppError::new(
                    "mod.unmanaged_pin",
                    "Updates are not available for this mod file.",
                )
            })?;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    prepare_instance_content_change(state.inner(), instance_id, request.expected_revision).await?;
    state
        .database
        .set_instance_mod_pinned(
            instance_id,
            request.expected_revision,
            InstanceModTarget {
                provider: Some(reference.0),
                project_id: Some(reference.1),
                file_path: &request.file_path,
            },
            request.pinned,
        )
        .await
        .map_err(|error| map_storage_error(error, "slate could not update that mod pin."))?;
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| map_storage_error(error, "slate could not refresh the instance."))
}

pub(super) async fn finish_instance_content_change(
    state: &DesktopState,
    instance_id: InstanceId,
    file_move: FileMove,
    database_result: Result<(), StorageError>,
    fallback: &'static str,
) -> Result<InstanceSummary, AppError> {
    if let Err(error) = database_result {
        let rollback = tokio::task::spawn_blocking(move || file_move.rollback()).await;
        if !matches!(rollback, Ok(Ok(()))) {
            return Err(AppError::new(
                "local.content_rollback_failed",
                "slate could not restore the content file after the change failed. The instance needs attention.",
            ));
        }
        return Err(map_storage_error(error, fallback));
    }
    state
        .database
        .get_instance(instance_id)
        .await
        .map(instance_summary)
        .map_err(|error| {
            map_storage_error(
                error,
                "slate changed the content file but could not refresh the instance.",
            )
        })
}

pub(super) fn content_file_error() -> AppError {
    AppError::new(
        "local.content_file_change_failed",
        "slate could not safely change that content file.",
    )
}

#[tauri::command]
pub(super) async fn modpack_get(
    state: tauri::State<'_, DesktopState>,
    request: ModpackProjectRequest,
) -> Result<Modpack, AppError> {
    state
        .modpacks
        .project(request.provider, &request.project_id)
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn modpack_versions_list(
    state: tauri::State<'_, DesktopState>,
    request: ModpackVersionsRequest,
) -> Result<VersionPage, AppError> {
    state
        .modpacks
        .versions(
            request.provider,
            &request.project_id,
            &VersionOptions {
                minecraft_version: request.minecraft_version,
                loader: request.loader,
                release_type: request.release_type,
                cursor: request.cursor,
                page: request.page,
                limit: request.limit.unwrap_or(20),
            },
        )
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn modpack_version_get(
    state: tauri::State<'_, DesktopState>,
    request: ModpackVersionRequest,
) -> Result<ModpackVersion, AppError> {
    state
        .modpacks
        .version(request.provider, &request.project_id, &request.version_id)
        .await
        .map_err(modpack_api_error)
}
