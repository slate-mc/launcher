use super::*;
use futures_util::{StreamExt, stream};

const MAX_CONTENT_SELECTIONS: usize = 50;
const CONTENT_PLAN_CONCURRENCY: usize = 8;

#[tauri::command]
pub(super) async fn content_search(
    state: tauri::State<'_, DesktopState>,
    request: ContentSearchRequest,
) -> Result<SearchResponse, AppError> {
    let instance = state
        .database
        .get_instance(InstanceId::from_uuid(request.instance_id))
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    state
        .modpacks
        .search_content(
            provider_content_kind(request.kind),
            &SearchOptions {
                query: request.query,
                provider: Some(slate_modpack_api_contracts::Provider::Modrinth),
                minecraft_version: Some(instance.minecraft_version),
                loader: None,
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
            },
        )
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn instance_content_install(
    state: tauri::State<'_, DesktopState>,
    request: InstallContentRequest,
) -> Result<InstallJobSummary, AppError> {
    instance_content_install_inner(state.inner(), request).await
}

pub(super) async fn instance_content_install_inner(
    state: &DesktopState,
    request: InstallContentRequest,
) -> Result<InstallJobSummary, AppError> {
    if request.content.is_empty() || request.content.len() > MAX_CONTENT_SELECTIONS {
        return Err(AppError::new(
            "content.invalid_selection",
            format!("Choose between 1 and {MAX_CONTENT_SELECTIONS} items."),
        ));
    }
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state, instance_id, request.expected_revision).await?;
    if instance.setup_state != slate_domain::InstanceSetupState::Ready {
        return Err(AppError::new(
            "local.instance_not_ready",
            "Finish installing or repair this instance before adding content.",
        ));
    }
    let kind = provider_content_kind(request.kind);
    let world_name =
        validate_content_world(state, &instance, request.kind, request.world_name).await?;
    let (loader, loader_version) = instance_runtime_target(&instance);
    let mut unique = BTreeMap::new();
    for selection in &request.content {
        validate_content_selection(selection)?;
        unique
            .entry((selection.provider, selection.project_id.trim().to_owned()))
            .or_insert_with(|| selection.clone());
    }
    let installed = state
        .database
        .list_instance_provider_content(instance_id, kind)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed content."))?;
    let installed_identities = installed
        .iter()
        .map(|item| (item.provider, item.project_id.clone()))
        .collect::<HashSet<_>>();
    let pending = unique
        .into_iter()
        .filter(|(identity, _)| !installed_identities.contains(identity))
        .map(|(_, selection)| selection)
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Err(AppError::new(
            "content.already_installed",
            "Every selected item is already installed.",
        ));
    }

    let api = state.modpacks.clone();
    let minecraft_version = instance.minecraft_version.clone();
    let plans = stream::iter(pending.into_iter().map(|selection| {
        let api = api.clone();
        let minecraft_version = minecraft_version.clone();
        let loader_version = loader_version.clone();
        async move {
            let plan = api
                .content_install_plan(
                    kind,
                    selection.project_id.trim(),
                    &ContentInstallPlanRequest {
                        minecraft_version,
                        loader,
                        loader_version,
                        version_id: None,
                    },
                )
                .await
                .map_err(modpack_api_error)?;
            Ok::<_, AppError>((selection, plan))
        }
    }))
    .buffer_unordered(CONTENT_PLAN_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    let installed_content = installed
        .iter()
        .map(|item| ((item.provider, item.project_id.clone()), item))
        .collect::<HashMap<_, _>>();
    let installed_mods = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed mods."))?;
    let installed_mods_by_identity = installed_mods
        .iter()
        .map(|item| ((item.provider, item.project_id.clone()), item))
        .collect::<HashMap<_, _>>();
    let mod_index = installed_mod_index(state, &instance).await?;
    let paths = paths_for_instance(state, &instance);
    let internal_kind = content_kind(request.kind);
    let scanned = tokio::task::spawn_blocking(move || {
        scan_instance_content(&paths, instance_id, internal_kind)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let scanned_paths = scanned
        .into_iter()
        .map(|item| normalized_content_path(&item.file_path))
        .collect::<HashSet<_>>();
    let mut planned_paths = HashMap::<String, (u64, Hashes)>::new();
    let mut downloads = Vec::new();
    let mut records = BTreeMap::new();
    let mut mod_records = BTreeMap::new();
    let mut replaced_content_paths = BTreeSet::new();
    let mut replaced_mod_paths = BTreeSet::new();
    let mut merged_plan = None;
    for result in plans {
        let (selection, plan) = result?;
        validate_content_plan(&instance, kind, &selection, &plan)?;
        for mut download in plan.downloads.clone() {
            if let Some((download_kind, provider, project_id, version_id)) =
                parse_content_download_id(&download.id)
            {
                let identity = (provider, project_id.clone());
                let existing = installed_content.get(&identity).copied();
                if existing.is_some_and(|item| {
                    item.version_id == version_id
                        && scanned_paths.contains(&normalized_content_path(&item.file_path))
                }) {
                    continue;
                }
                if existing.is_some_and(|item| item.pinned) {
                    return Err(AppError::new(
                        "content.dependency_pinned",
                        format!(
                            "{} is pinned, but the selected content requires another version.",
                            existing.map_or("A dependency", |item| item.display_name.as_str())
                        ),
                    ));
                }
                if download_kind == ContentKind::DataPack {
                    let world_name = world_name.as_deref().ok_or_else(|| {
                        AppError::new(
                            "content.world_required",
                            "Choose a world before adding a data pack.",
                        )
                    })?;
                    let file_name = download
                        .destination
                        .strip_prefix("datapacks/")
                        .ok_or_else(invalid_content_plan)?;
                    download.destination = format!("saves/{world_name}/datapacks/{file_name}");
                }
                let normalized = normalized_content_path(&download.destination);
                let replaced = existing.map(|item| normalized_content_path(&item.file_path));
                if scanned_paths.contains(&normalized) && replaced.as_deref() != Some(&normalized) {
                    return Err(AppError::new(
                        "content.file_conflict",
                        format!(
                            "{} conflicts with a content file that is already installed.",
                            selection.display_name.trim()
                        ),
                    ));
                }
                if !accept_planned_download(&mut planned_paths, &normalized, &download)? {
                    continue;
                }
                if let Some(existing) = existing {
                    replaced_content_paths.insert(existing.file_path.clone());
                }
                let root = project_id == selection.project_id.trim();
                records.insert(
                    identity,
                    NewInstanceProviderContent {
                        kind: download_kind,
                        provider,
                        project_id,
                        version_id,
                        display_name: if root {
                            selection.display_name.trim().to_owned()
                        } else {
                            existing.map_or_else(
                                || display_name_from_path(&download.destination),
                                |item| item.display_name.clone(),
                            )
                        },
                        icon_url: if root {
                            trusted_content_icon(selection.icon_url.as_deref())
                        } else {
                            existing.and_then(|item| item.icon_url.clone())
                        },
                        file_path: download.destination.clone(),
                        hashes: download.hashes.clone(),
                        pinned: existing.is_some_and(|item| item.pinned),
                    },
                );
                downloads.push(download);
            } else if let Some((provider, project_id, version_id)) =
                parse_mod_download_id(&download.id)
            {
                let identity = (provider, project_id.clone());
                let identity_key = format!("{provider}:{project_id}");
                let existing = installed_mods_by_identity.get(&identity).copied();
                if mod_index.versions.get(&identity_key) == Some(&version_id) {
                    continue;
                }
                if existing.is_some_and(|item| item.pinned) {
                    return Err(AppError::new(
                        "content.dependency_pinned",
                        format!(
                            "{} is pinned, but the selected content requires another version.",
                            existing.map_or("A mod dependency", |item| item.display_name.as_str())
                        ),
                    ));
                }
                let normalized = normalized_content_path(&download.destination);
                let replaced = existing.map(|item| normalized_content_path(&item.file_path));
                if mod_index.artifacts.contains_key(&normalized)
                    && replaced.as_deref() != Some(&normalized)
                {
                    return Err(AppError::new(
                        "content.file_conflict",
                        "A required mod conflicts with a mod file that is already installed.",
                    ));
                }
                if !accept_planned_download(&mut planned_paths, &normalized, &download)? {
                    continue;
                }
                if let Some(existing) = existing {
                    replaced_mod_paths.insert(existing.file_path.clone());
                }
                mod_records.insert(
                    identity,
                    NewInstanceMod {
                        provider,
                        project_id,
                        version_id,
                        display_name: existing.map_or_else(
                            || display_name_from_path(&download.destination),
                            |item| item.display_name.clone(),
                        ),
                        file_path: download.destination.clone(),
                        hashes: download.hashes.clone(),
                        enabled: true,
                        pinned: existing.is_some_and(|item| item.pinned),
                    },
                );
                downloads.push(download);
            } else {
                return Err(invalid_content_plan());
            }
        }
        merged_plan.get_or_insert(plan);
    }
    let mut plan = merged_plan.ok_or_else(invalid_content_plan)?;
    plan.downloads = downloads;
    plan.delete.extend(replaced_content_paths.iter().cloned());
    plan.delete.extend(replaced_mod_paths.iter().cloned());
    plan.delete.sort();
    plan.delete.dedup();
    plan.total_download_size = plan.downloads.iter().try_fold(0_u64, |total, download| {
        total
            .checked_add(download.size)
            .ok_or_else(invalid_content_plan)
    })?;
    queue_instance_install(
        state,
        instance,
        request.expected_revision,
        Some(plan),
        PendingModChanges {
            installed: mod_records.into_values().collect(),
            replaced_paths: replaced_mod_paths.into_iter().collect(),
            provider_content: records.into_values().collect(),
            replaced_content_paths: replaced_content_paths.into_iter().collect(),
            ..PendingModChanges::default()
        },
        None,
        RetryableInstallOperation::ContentInstall {
            kind: request.kind,
            world_name,
            content: request.content,
        },
    )
    .await
}

fn accept_planned_download(
    planned: &mut HashMap<String, (u64, Hashes)>,
    destination: &str,
    download: &InstallPlanDownload,
) -> Result<bool, AppError> {
    if let Some((size, hashes)) = planned.get(destination) {
        if same_mod_artifact(*size, hashes, download.size, &download.hashes) {
            Ok(false)
        } else {
            Err(AppError::new(
                "content.file_conflict",
                "The selected content resolves to conflicting files.",
            ))
        }
    } else {
        planned.insert(
            destination.to_owned(),
            (download.size, download.hashes.clone()),
        );
        Ok(true)
    }
}

fn display_name_from_path(value: &str) -> String {
    let file_name = value.rsplit('/').next().unwrap_or(value);
    let file_name = file_name.strip_suffix(".disabled").unwrap_or(file_name);
    file_name
        .strip_suffix(".zip")
        .or_else(|| file_name.strip_suffix(".jar"))
        .unwrap_or(file_name)
        .to_owned()
}

#[tauri::command]
pub(super) async fn instance_content_versions(
    state: tauri::State<'_, DesktopState>,
    request: InstanceContentVersionsRequest,
) -> Result<ModVersionList, AppError> {
    validate_provider_content_identity(request.provider, &request.project_id)?;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance = state
        .database
        .get_instance(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not load that instance."))?;
    ensure_provider_content_installed(
        state.inner(),
        instance_id,
        request.kind,
        request.provider,
        &request.project_id,
    )
    .await?;
    state
        .modpacks
        .content_versions(
            provider_content_kind(request.kind),
            request.project_id.trim(),
            &instance.minecraft_version,
        )
        .await
        .map_err(modpack_api_error)
}

#[tauri::command]
pub(super) async fn instance_content_history(
    state: tauri::State<'_, DesktopState>,
    request: InstanceContentHistoryRequest,
) -> Result<Vec<InstanceContentHistorySummary>, AppError> {
    validate_provider_content_identity(request.provider, &request.project_id)?;
    let instance_id = InstanceId::from_uuid(request.instance_id);
    ensure_provider_content_installed(
        state.inner(),
        instance_id,
        request.kind,
        request.provider,
        &request.project_id,
    )
    .await?;
    state
        .database
        .list_instance_provider_content_history(
            instance_id,
            provider_content_kind(request.kind),
            request.provider,
            request.project_id.trim(),
        )
        .await
        .map(|items| {
            items
                .into_iter()
                .map(|item| InstanceContentHistorySummary {
                    version_id: item.version_id,
                    changed_at: item.changed_at,
                })
                .collect()
        })
        .map_err(|error| map_storage_error(error, "slate could not load version history."))
}

#[tauri::command]
pub(super) async fn instance_content_update(
    state: tauri::State<'_, DesktopState>,
    request: UpdateInstanceContentRequest,
) -> Result<InstallJobSummary, AppError> {
    instance_content_update_inner(state.inner(), request).await
}

pub(super) async fn instance_content_update_inner(
    state: &DesktopState,
    request: UpdateInstanceContentRequest,
) -> Result<InstallJobSummary, AppError> {
    validate_provider_content_identity(request.provider, &request.project_id)?;
    let target_version_id = request.target_version_id.trim();
    if target_version_id.is_empty() || target_version_id.len() > 128 {
        return Err(AppError::new(
            "content.invalid_version",
            "Choose a valid content version.",
        ));
    }
    let instance_id = InstanceId::from_uuid(request.instance_id);
    let instance =
        prepare_instance_content_change(state, instance_id, request.expected_revision).await?;
    let kind = provider_content_kind(request.kind);
    let records = state
        .database
        .list_instance_provider_content(instance_id, kind)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed content."))?;
    let current = records
        .iter()
        .find(|item| {
            item.provider == request.provider
                && item.project_id == request.project_id.trim()
                && item.file_path == request.file_path
        })
        .cloned()
        .ok_or_else(|| {
            AppError::new(
                "content.not_installed",
                "That content file is no longer installed in this instance.",
            )
        })?;
    if current.version_id == target_version_id {
        return Err(AppError::new(
            "content.version_unchanged",
            "That content version is already installed.",
        ));
    }
    let (loader, loader_version) = instance_runtime_target(&instance);
    let mut plan = state
        .modpacks
        .content_install_plan(
            kind,
            request.project_id.trim(),
            &ContentInstallPlanRequest {
                minecraft_version: instance.minecraft_version.clone(),
                loader,
                loader_version,
                version_id: Some(target_version_id.to_owned()),
            },
        )
        .await
        .map_err(modpack_api_error)?;
    let selection = InstallContentSelection {
        provider: request.provider,
        project_id: request.project_id.trim().to_owned(),
        display_name: request.display_name.trim().to_owned(),
        icon_url: current.icon_url.clone(),
    };
    validate_content_plan(&instance, kind, &selection, &plan)?;
    if plan.instance.version_id != target_version_id {
        return Err(invalid_content_plan());
    }
    let installed_content = records
        .iter()
        .map(|item| ((item.provider, item.project_id.clone()), item))
        .collect::<HashMap<_, _>>();
    let installed_mods = state
        .database
        .list_instance_mods(instance_id)
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed mods."))?;
    let installed_mods_by_identity = installed_mods
        .iter()
        .map(|item| ((item.provider, item.project_id.clone()), item))
        .collect::<HashMap<_, _>>();
    let mod_index = installed_mod_index(state, &instance).await?;
    let paths = paths_for_instance(state, &instance);
    let internal_kind = content_kind(request.kind);
    let scanned = tokio::task::spawn_blocking(move || {
        scan_instance_content(&paths, instance_id, internal_kind)
    })
    .await
    .map_err(|_| content_file_error())?
    .map_err(|_| content_file_error())?;
    let scanned_paths = scanned
        .into_iter()
        .map(|item| normalized_content_path(&item.file_path))
        .collect::<HashSet<_>>();
    let display_name = if request.display_name.trim().is_empty() {
        current.display_name.clone()
    } else {
        request.display_name.trim().to_owned()
    };
    let mut planned_paths = HashMap::<String, (u64, Hashes)>::new();
    let mut accepted_downloads = Vec::new();
    let mut content_records = BTreeMap::new();
    let mut mod_records = BTreeMap::new();
    let mut replaced_content_paths = BTreeSet::new();
    let mut replaced_mod_paths = BTreeSet::new();
    let mut root_found = false;
    for mut download in std::mem::take(&mut plan.downloads) {
        if let Some((download_kind, provider, project_id, version_id)) =
            parse_content_download_id(&download.id)
        {
            let identity = (provider, project_id.clone());
            let existing = installed_content.get(&identity).copied();
            let root = provider == request.provider && project_id == request.project_id.trim();
            if root {
                root_found = true;
                if version_id != target_version_id {
                    return Err(invalid_content_plan());
                }
            } else if existing.is_some_and(|item| {
                item.version_id == version_id
                    && scanned_paths.contains(&normalized_content_path(&item.file_path))
            }) {
                continue;
            }
            if !root && existing.is_some_and(|item| item.pinned) {
                return Err(AppError::new(
                    "content.dependency_pinned",
                    format!(
                        "{} is pinned, but this version requires another release.",
                        existing.map_or("A dependency", |item| item.display_name.as_str())
                    ),
                ));
            }
            let file_name = download
                .destination
                .rsplit('/')
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(invalid_content_plan)?;
            let location =
                existing.map_or(current.file_path.as_str(), |item| item.file_path.as_str());
            download.destination = updated_content_destination(request.kind, location, file_name)?;
            if existing.is_some_and(|item| item.file_path.ends_with(".disabled")) {
                download.destination.push_str(".disabled");
            }
            let normalized = normalized_content_path(&download.destination);
            let replaced = existing.map(|item| normalized_content_path(&item.file_path));
            if scanned_paths.contains(&normalized) && replaced.as_deref() != Some(&normalized) {
                return Err(AppError::new(
                    "content.file_conflict",
                    "That version conflicts with a content file that is already installed.",
                ));
            }
            if !accept_planned_download(&mut planned_paths, &normalized, &download)? {
                continue;
            }
            if let Some(existing) = existing {
                replaced_content_paths.insert(existing.file_path.clone());
            }
            content_records.insert(
                identity,
                NewInstanceProviderContent {
                    kind: download_kind,
                    provider,
                    project_id,
                    version_id,
                    display_name: if root {
                        display_name.clone()
                    } else {
                        existing.map_or_else(
                            || display_name_from_path(&download.destination),
                            |item| item.display_name.clone(),
                        )
                    },
                    icon_url: if root {
                        current.icon_url.clone()
                    } else {
                        existing.and_then(|item| item.icon_url.clone())
                    },
                    file_path: download.destination.clone(),
                    hashes: download.hashes.clone(),
                    pinned: existing.is_some_and(|item| item.pinned),
                },
            );
            accepted_downloads.push(download);
        } else if let Some((provider, project_id, version_id)) = parse_mod_download_id(&download.id)
        {
            let identity = (provider, project_id.clone());
            let identity_key = format!("{provider}:{project_id}");
            let existing = installed_mods_by_identity.get(&identity).copied();
            if mod_index.versions.get(&identity_key) == Some(&version_id) {
                continue;
            }
            if existing.is_some_and(|item| item.pinned) {
                return Err(AppError::new(
                    "content.dependency_pinned",
                    format!(
                        "{} is pinned, but this version requires another release.",
                        existing.map_or("A mod dependency", |item| item.display_name.as_str())
                    ),
                ));
            }
            let normalized = normalized_content_path(&download.destination);
            let replaced = existing.map(|item| normalized_content_path(&item.file_path));
            if mod_index.artifacts.contains_key(&normalized)
                && replaced.as_deref() != Some(&normalized)
            {
                return Err(AppError::new(
                    "content.file_conflict",
                    "A required mod conflicts with a mod file that is already installed.",
                ));
            }
            if !accept_planned_download(&mut planned_paths, &normalized, &download)? {
                continue;
            }
            if let Some(existing) = existing {
                replaced_mod_paths.insert(existing.file_path.clone());
            }
            mod_records.insert(
                identity,
                NewInstanceMod {
                    provider,
                    project_id,
                    version_id,
                    display_name: existing.map_or_else(
                        || display_name_from_path(&download.destination),
                        |item| item.display_name.clone(),
                    ),
                    file_path: download.destination.clone(),
                    hashes: download.hashes.clone(),
                    enabled: existing.is_none_or(|item| item.enabled),
                    pinned: existing.is_some_and(|item| item.pinned),
                },
            );
            accepted_downloads.push(download);
        } else {
            return Err(invalid_content_plan());
        }
    }
    if !root_found || accepted_downloads.is_empty() {
        return Err(invalid_content_plan());
    }
    plan.downloads = accepted_downloads;
    plan.delete.extend(replaced_content_paths.iter().cloned());
    plan.delete.extend(replaced_mod_paths.iter().cloned());
    plan.delete.sort();
    plan.delete.dedup();
    plan.total_download_size = plan.downloads.iter().try_fold(0_u64, |total, download| {
        total
            .checked_add(download.size)
            .ok_or_else(invalid_content_plan)
    })?;
    queue_instance_install(
        state,
        instance,
        request.expected_revision,
        Some(plan),
        PendingModChanges {
            installed: mod_records.into_values().collect(),
            replaced_paths: replaced_mod_paths.into_iter().collect(),
            provider_content: content_records.into_values().collect(),
            replaced_content_paths: replaced_content_paths.into_iter().collect(),
            ..PendingModChanges::default()
        },
        None,
        RetryableInstallOperation::ContentUpdate {
            kind: request.kind,
            provider: request.provider,
            project_id: request.project_id,
            file_path: request.file_path,
            display_name,
            target_version_id: target_version_id.to_owned(),
        },
    )
    .await
}

async fn ensure_provider_content_installed(
    state: &DesktopState,
    instance_id: InstanceId,
    kind: InstanceContentKindDto,
    provider: slate_modpack_api_contracts::Provider,
    project_id: &str,
) -> Result<(), AppError> {
    let installed = state
        .database
        .list_instance_provider_content(instance_id, provider_content_kind(kind))
        .await
        .map_err(|error| map_storage_error(error, "slate could not inspect installed content."))?;
    if installed
        .iter()
        .any(|item| item.provider == provider && item.project_id == project_id.trim())
    {
        Ok(())
    } else {
        Err(AppError::new(
            "content.not_installed",
            "That content file is no longer installed in this instance.",
        ))
    }
}

fn validate_provider_content_identity(
    provider: slate_modpack_api_contracts::Provider,
    project_id: &str,
) -> Result<(), AppError> {
    if provider != slate_modpack_api_contracts::Provider::Modrinth
        || project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Err(AppError::new(
            "content.provider_unavailable",
            "Updates are not available for that content file.",
        ))
    } else {
        Ok(())
    }
}

fn updated_content_destination(
    kind: InstanceContentKindDto,
    current_path: &str,
    file_name: &str,
) -> Result<String, AppError> {
    match kind {
        InstanceContentKindDto::ResourcePack => Ok(format!("resourcepacks/{file_name}")),
        InstanceContentKindDto::ShaderPack => Ok(format!("shaderpacks/{file_name}")),
        InstanceContentKindDto::DataPack => {
            let normalized = current_path.replace('\\', "/");
            let marker = "/datapacks/";
            let position = normalized.find(marker).ok_or_else(invalid_content_plan)?;
            Ok(format!("{}{marker}{file_name}", &normalized[..position]))
        }
    }
}

fn validate_content_selection(selection: &InstallContentSelection) -> Result<(), AppError> {
    if selection.provider != slate_modpack_api_contracts::Provider::Modrinth
        || selection.project_id.is_empty()
        || selection.project_id.len() > 128
        || !selection
            .project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || selection.display_name.trim().is_empty()
        || selection.display_name.chars().count() > 160
        || selection.display_name.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "content.invalid_selection",
            "One of the selected items has invalid details.",
        ));
    }
    Ok(())
}

fn instance_runtime_target(instance: &InstanceRecord) -> (LoaderKind, Option<String>) {
    match instance.loader_kind {
        LoaderFamily::Vanilla => (LoaderKind::Vanilla, None),
        LoaderFamily::Fabric => (LoaderKind::Fabric, instance.loader_version.clone()),
        LoaderFamily::NeoForge => (LoaderKind::NeoForge, instance.loader_version.clone()),
    }
}

async fn validate_content_world(
    state: &DesktopState,
    instance: &InstanceRecord,
    kind: InstanceContentKindDto,
    world_name: Option<String>,
) -> Result<Option<String>, AppError> {
    if kind != InstanceContentKindDto::DataPack {
        return Ok(None);
    }
    let world_name = world_name
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 240
                && !matches!(value.as_str(), "." | "..")
                && !value.contains(['/', '\\', ':'])
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| {
            AppError::new(
                "content.world_required",
                "Choose a world before adding a data pack.",
            )
        })?;
    let paths = paths_for_instance(state, instance);
    let instance_id = instance.id;
    let worlds = tokio::task::spawn_blocking(move || scan_instance_worlds(&paths, instance_id))
        .await
        .map_err(|_| content_file_error())?
        .map_err(|_| content_file_error())?;
    if !worlds.contains(&world_name) {
        return Err(AppError::new(
            "content.world_unavailable",
            "That world is no longer available. Choose another world and try again.",
        ));
    }
    Ok(Some(world_name))
}

fn validate_content_plan(
    instance: &InstanceRecord,
    kind: ContentKind,
    selection: &InstallContentSelection,
    plan: &InstallPlan,
) -> Result<(), AppError> {
    let (loader, loader_version) = instance_runtime_target(instance);
    let valid_downloads = !plan.downloads.is_empty()
        && plan.downloads.len() <= 64
        && plan.downloads.iter().all(|download| {
            download.hashes.has_cryptographic_hash()
                && if let Some((download_kind, provider, _, _)) =
                    parse_content_download_id(&download.id)
                {
                    provider == slate_modpack_api_contracts::Provider::Modrinth
                        && download_kind == kind
                        && download
                            .destination
                            .starts_with(content_download_prefix(download_kind))
                } else {
                    parse_mod_download_id(&download.id).is_some()
                        && download.destination.starts_with("mods/")
                }
        })
        && plan.downloads.iter().any(|download| {
            parse_content_download_id(&download.id).is_some_and(
                |(download_kind, provider, project_id, _)| {
                    download_kind == kind
                        && provider == selection.provider
                        && project_id == selection.project_id.trim()
                },
            )
        });
    if plan.schema != 1
        || plan.instance.provider != selection.provider
        || plan.instance.project_id != selection.project_id.trim()
        || plan.runtime.minecraft != instance.minecraft_version
        || plan.runtime.loader.kind != loader
        || plan.runtime.loader.version != loader_version
        || !plan.extract.is_empty()
        || !plan.delete.is_empty()
        || !valid_downloads
    {
        return Err(invalid_content_plan());
    }
    Ok(())
}

pub(super) fn parse_content_download_id(
    value: &str,
) -> Option<(
    ContentKind,
    slate_modpack_api_contracts::Provider,
    String,
    String,
)> {
    let mut parts = value.splitn(5, ':');
    if parts.next()? != "content" {
        return None;
    }
    let kind = match parts.next()? {
        "resource_pack" => ContentKind::ResourcePack,
        "shader_pack" => ContentKind::ShaderPack,
        "data_pack" => ContentKind::DataPack,
        _ => return None,
    };
    let provider = slate_modpack_api_contracts::Provider::from_str(parts.next()?).ok()?;
    let project_id = parts.next()?.trim();
    let version_id = parts.next()?.trim();
    if provider == slate_modpack_api_contracts::Provider::Ftb
        || project_id.is_empty()
        || version_id.is_empty()
    {
        return None;
    }
    Some((kind, provider, project_id.to_owned(), version_id.to_owned()))
}

const fn content_download_prefix(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::ResourcePack => "resourcepacks/",
        ContentKind::ShaderPack => "shaderpacks/",
        ContentKind::DataPack => "datapacks/",
    }
}

fn trusted_content_icon(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    url::Url::parse(value)
        .ok()
        .filter(|url| {
            url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.host_str() == Some("cdn.modrinth.com")
        })
        .map(|_| value.to_owned())
}

fn invalid_content_plan() -> AppError {
    AppError::new(
        "content.resolution_mismatch",
        "A compatible version of the selected content could not be confirmed. Nothing was installed.",
    )
}
