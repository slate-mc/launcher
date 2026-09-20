use super::*;

pub(super) fn artwork_error() -> AppError {
    AppError::new(
        "local.artwork_unavailable",
        "Choose a PNG, JPEG, or WebP image within the size limit.",
    )
}

pub(super) fn instance_artwork_path(
    paths: &AppPaths,
    instance_id: InstanceId,
    kind: InstanceArtworkKindDto,
) -> PathBuf {
    let filename = match kind {
        InstanceArtworkKindDto::Icon => "profile-icon.bin",
        InstanceArtworkKindDto::Banner => "profile-banner.bin",
    };
    paths.instance(instance_id).join("metadata").join(filename)
}

pub(super) fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

pub(super) struct ManagedFileReplacement {
    destination: PathBuf,
    backup: Option<PathBuf>,
}

impl ManagedFileReplacement {
    pub(super) async fn rollback(self) {
        let _ = tokio::fs::remove_file(&self.destination).await;
        if let Some(backup) = self.backup {
            let _ = tokio::fs::rename(backup, self.destination).await;
        }
    }

    pub(super) async fn commit(self) {
        if let Some(backup) = self.backup {
            let _ = tokio::fs::remove_file(backup).await;
        }
    }
}

pub(super) async fn replace_managed_file(
    destination: PathBuf,
    bytes: Vec<u8>,
) -> Result<ManagedFileReplacement, AppError> {
    replace_file(destination, bytes, artwork_error).await
}

pub(super) async fn replace_file(
    destination: PathBuf,
    bytes: Vec<u8>,
    error: fn() -> AppError,
) -> Result<ManagedFileReplacement, AppError> {
    let parent = destination.parent().ok_or_else(error)?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_| error())?;
    let temporary = destination.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let backup = destination.with_extension(format!("bak-{}", Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes)
        .await
        .map_err(|_| error())?;
    let previous = if tokio::fs::try_exists(&destination)
        .await
        .map_err(|_| error())?
    {
        if let Err(io_error) = tokio::fs::rename(&destination, &backup).await {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(if io_error.kind() == std::io::ErrorKind::PermissionDenied {
                AppError::new(
                    "local.artwork_in_use",
                    "Close any app using that image and try again.",
                )
            } else {
                error()
            });
        }
        Some(backup)
    } else {
        None
    };
    if tokio::fs::rename(&temporary, &destination).await.is_err() {
        if let Some(backup) = previous.as_ref() {
            let _ = tokio::fs::rename(backup, &destination).await;
        }
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error());
    }
    Ok(ManagedFileReplacement {
        destination,
        backup: previous,
    })
}

pub(super) async fn remove_managed_file(
    destination: PathBuf,
) -> Result<ManagedFileReplacement, AppError> {
    let backup = destination.with_extension(format!("bak-{}", Uuid::new_v4()));
    let previous = if tokio::fs::try_exists(&destination)
        .await
        .map_err(|_| artwork_error())?
    {
        tokio::fs::rename(&destination, &backup)
            .await
            .map_err(|_| artwork_error())?;
        Some(backup)
    } else {
        None
    };
    Ok(ManagedFileReplacement {
        destination,
        backup: previous,
    })
}
