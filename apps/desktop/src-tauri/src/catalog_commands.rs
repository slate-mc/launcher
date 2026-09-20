use super::*;

#[tauri::command]
pub(super) async fn minecraft_versions_list() -> Result<MinecraftVersionCatalog, AppError> {
    let client = MojangMetadataClient::new().map_err(|_| metadata_error())?;
    let manifest = client
        .fetch_manifest()
        .await
        .map_err(|_| metadata_error())?;
    let versions = manifest
        .versions
        .into_iter()
        .filter(|entry| entry.version_type == "release")
        .take(160)
        .map(|entry| MinecraftVersionOption {
            id: entry.id,
            kind: MinecraftReleaseKindDto::Release,
        })
        .collect();
    Ok(MinecraftVersionCatalog {
        latest_release: manifest.latest.release,
        versions,
    })
}

#[tauri::command]
pub(super) async fn loader_versions_list(
    request: LoaderVersionsRequest,
) -> Result<LoaderVersionCatalog, AppError> {
    let versions = fetch_loader_versions(&request.minecraft_version, request.loader_kind).await?;
    let recommended_version = versions.first().cloned();
    let unavailable_reason = (request.loader_kind != LoaderKindDto::Vanilla && versions.is_empty())
        .then(|| {
            "No compatible loader release was published for this Minecraft version.".to_owned()
        });
    Ok(LoaderVersionCatalog {
        loader_kind: request.loader_kind,
        minecraft_version: request.minecraft_version,
        recommended_version,
        versions,
        unavailable_reason,
    })
}

pub(super) async fn fetch_loader_versions(
    minecraft_version: &str,
    loader_kind: LoaderKindDto,
) -> Result<Vec<String>, AppError> {
    let client = MojangMetadataClient::new().map_err(|_| metadata_error())?;
    let manifest = client
        .fetch_manifest()
        .await
        .map_err(|_| metadata_error())?;
    let known_release = manifest
        .find(minecraft_version)
        .is_some_and(|entry| entry.version_type == "release");
    if !known_release {
        return Err(AppError::new(
            "metadata.minecraft_version_unknown",
            "Choose a Minecraft release from the current catalog.",
        )
        .with_field_error(
            "minecraftVersion",
            "The selected release is not in the catalog.",
        ));
    }

    let versions = match loader_kind {
        LoaderKindDto::Vanilla => Vec::new(),
        LoaderKindDto::Fabric => FabricAdapter::new()
            .map_err(|_| metadata_error())?
            .fetch_loader_versions(minecraft_version)
            .await
            .map_err(|_| metadata_error())?,
        LoaderKindDto::NeoForge => NeoForgeAdapter::new()
            .map_err(|_| metadata_error())?
            .fetch_loader_versions(minecraft_version)
            .await
            .map_err(|_| metadata_error())?,
    };
    Ok(versions)
}

pub(super) async fn validate_selected_loader_version(
    minecraft_version: &str,
    loader_kind: LoaderKindDto,
    selected: Option<&str>,
) -> Result<Option<String>, AppError> {
    if loader_kind == LoaderKindDto::Vanilla {
        return Ok(None);
    }
    let versions = fetch_loader_versions(minecraft_version, loader_kind).await?;
    let selected = selected.map(str::trim).filter(|value| !value.is_empty());
    let Some(selected) = selected else {
        return Err(configuration_app_error(
            ConfigurationValidationError::LoaderVersionRequired,
        ));
    };
    if !versions.iter().any(|version| version == selected) {
        return Err(AppError::new(
            "metadata.loader_version_unknown",
            "Choose a compatible loader version from the current catalog.",
        )
        .with_field_error(
            "loaderVersion",
            "The selected loader version is not compatible with this Minecraft release.",
        ));
    }
    Ok(Some(selected.to_owned()))
}
