use super::*;

pub(super) fn storage_category(
    category: StorageCategoryDto,
    usage: DiskUsage,
) -> StorageCategorySummary {
    StorageCategorySummary {
        category,
        size_bytes: usage.bytes,
        file_count: usage.files,
    }
}

pub(super) fn storage_scan_error() -> AppError {
    AppError::new(
        "local.storage_scan_failed",
        "slate could not finish measuring managed storage. Check that the storage folders are available and try again.",
    )
    .retryable(true)
}

pub(super) fn storage_cleanup_error() -> AppError {
    AppError::new(
        "local.storage_cleanup_failed",
        "slate could not finish clearing those files. Files still in use were left in place.",
    )
    .retryable(true)
}

pub(super) fn stage_trashed_instances(
    records: &[TrashedInstanceRecord],
) -> Result<(Vec<StagedInstanceDeletion>, DiskUsage), std::io::Error> {
    let mut staged = Vec::with_capacity(records.len());
    let mut usage = DiskUsage::default();
    for record in records {
        usage.include(measure_path(&record.storage_path)?);
        match stage_instance_deletion(&record.storage_path, record.id) {
            Ok(Some(item)) => staged.push(item),
            Ok(None) => {}
            Err(error) => {
                for item in staged.into_iter().rev() {
                    let _ = item.rollback();
                }
                return Err(error);
            }
        }
    }
    Ok((staged, usage))
}

pub(super) async fn rollback_staged_deletions(staged: Vec<StagedInstanceDeletion>) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        for item in staged.into_iter().rev() {
            let _ = item.rollback();
        }
    })
    .await;
}

pub(super) async fn purge_staged_deletions(staged: Vec<StagedInstanceDeletion>) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        for item in staged {
            let _ = item.purge();
        }
    })
    .await;
}

pub(super) async fn purge_expired_instance_trash(state: DesktopState) {
    let Ok(preferences) = state.database.get_app_preferences().await else {
        return;
    };
    if preferences.trash_retention_days == 0 {
        return;
    }
    let cutoff = OffsetDateTime::now_utc()
        - time::Duration::days(i64::from(preferences.trash_retention_days));
    let Ok(records) = state.database.list_trashed_instances(1_000).await else {
        return;
    };
    let expired = records
        .into_iter()
        .filter(|record| {
            OffsetDateTime::parse(&record.trashed_at, &Rfc3339).is_ok_and(|value| value <= cutoff)
        })
        .collect::<Vec<_>>();
    if expired.is_empty() {
        return;
    }
    let staged = match tauri::async_runtime::spawn_blocking({
        let expired = expired.clone();
        move || stage_trashed_instances(&expired).map(|(staged, _)| staged)
    })
    .await
    {
        Ok(Ok(staged)) => staged,
        _ => return,
    };
    let revisions = expired
        .iter()
        .map(|record| (record.id, record.revision))
        .collect::<Vec<_>>();
    if state
        .database
        .permanently_delete_trashed_instance_records(&revisions)
        .await
        .is_err()
    {
        rollback_staged_deletions(staged).await;
        return;
    }
    purge_staged_deletions(staged).await;
}
