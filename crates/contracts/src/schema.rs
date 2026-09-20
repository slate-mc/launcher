use crate::{
    AppError, AppPreferencesDto, ApplyModpackUpdateRequest, AuthFlowStatus, AuthStartResponse,
    BootstrapResponse, CancelInstallJobRequest, CheckModpackUpdateRequest,
    ClearStorageCategoryRequest, CreateInstanceRequest, CreateInstanceSnapshotRequest,
    CreateSavedServerRequest, CreateSupportReportRequest, DeleteInstanceSnapshotRequest,
    DeleteTrashedInstanceRequest, DuplicateInstanceRequest, EmptyInstanceTrashRequest,
    EventEnvelope, ExportInstanceRequest, GameSessionSummary, GetInstanceArtworkRequest,
    GetInstanceGameOptionsRequest, ImportInstanceRequest, InstallInstanceRequest,
    InstallJobSummary, InstallModRequest, InstallModpackRequest, InstanceContentFileSummary,
    InstanceContentFilesRequest, InstanceGameOptionsSummary, InstanceModResolution,
    InstanceModSummary, InstanceModsRequest, InstanceSnapshotSummary, InstanceSnapshotsRequest,
    InstanceSummary, LaunchInstanceRequest, LoaderVersionCatalog, LoaderVersionsRequest,
    MinecraftAccountSummary, MinecraftVersionCatalog, ModSearchRequest, ModpackInstallStarted,
    ModpackProjectRequest, ModpackSearchRequest, ModpackUpdateSummary, ModpackVersionRequest,
    ModpackVersionsRequest, MoveInstanceStorageRequest, OnboardingStateSummary,
    OpenInstanceDirectoryRequest, PingServerRequest, PreflightSummary, RedactedLaunchPlan,
    RemoveInstanceContentFileRequest, RemoveInstanceModRequest, RemoveSavedServerRequest,
    RestoreInstanceSnapshotRequest, RestoreTrashedInstanceRequest, RetryInstallJobRequest,
    SavedServerSummary, SelectInstanceArtworkRequest, SelectInstanceJavaRequest,
    ServerStatusSummary, SessionLogEvent, SessionLogSubscription,
    SetInstanceContentFileEnabledRequest, SetInstanceModEnabledRequest,
    SetInstanceModPinnedRequest, SetInstanceSnapshotPinnedRequest, StopGameSessionRequest,
    StorageCleanupResult, StorageOverview, SubscribeSessionLogRequest, SupportReportExport,
    SupportReportPreview, UnsubscribeSessionLogRequest, UpdateInstanceConfigurationRequest,
    UpdateInstanceGameOptionsRequest, UpdateInstanceSettingsRequest, UpdateSavedServerRequest,
};
use schemars::{Schema, schema_for};
use std::collections::BTreeMap;

#[must_use]
pub fn schema_documents() -> BTreeMap<&'static str, Schema> {
    BTreeMap::from([
        ("app-error", schema_for!(AppError)),
        ("bootstrap-response", schema_for!(BootstrapResponse)),
        ("event-envelope", schema_for!(EventEnvelope)),
        ("instance-summary", schema_for!(InstanceSummary)),
        (
            "create-instance-request",
            schema_for!(CreateInstanceRequest),
        ),
        (
            "update-instance-configuration-request",
            schema_for!(UpdateInstanceConfigurationRequest),
        ),
        (
            "update-instance-settings-request",
            schema_for!(UpdateInstanceSettingsRequest),
        ),
        (
            "select-instance-java-request",
            schema_for!(SelectInstanceJavaRequest),
        ),
        (
            "select-instance-artwork-request",
            schema_for!(SelectInstanceArtworkRequest),
        ),
        (
            "get-instance-artwork-request",
            schema_for!(GetInstanceArtworkRequest),
        ),
        (
            "get-instance-game-options-request",
            schema_for!(GetInstanceGameOptionsRequest),
        ),
        (
            "update-instance-game-options-request",
            schema_for!(UpdateInstanceGameOptionsRequest),
        ),
        (
            "instance-game-options-summary",
            schema_for!(InstanceGameOptionsSummary),
        ),
        (
            "open-instance-directory-request",
            schema_for!(OpenInstanceDirectoryRequest),
        ),
        (
            "duplicate-instance-request",
            schema_for!(DuplicateInstanceRequest),
        ),
        (
            "move-instance-storage-request",
            schema_for!(MoveInstanceStorageRequest),
        ),
        (
            "export-instance-request",
            schema_for!(ExportInstanceRequest),
        ),
        (
            "import-instance-request",
            schema_for!(ImportInstanceRequest),
        ),
        (
            "instance-snapshot-summary",
            schema_for!(InstanceSnapshotSummary),
        ),
        (
            "instance-snapshots-request",
            schema_for!(InstanceSnapshotsRequest),
        ),
        (
            "create-instance-snapshot-request",
            schema_for!(CreateInstanceSnapshotRequest),
        ),
        (
            "restore-instance-snapshot-request",
            schema_for!(RestoreInstanceSnapshotRequest),
        ),
        (
            "delete-instance-snapshot-request",
            schema_for!(DeleteInstanceSnapshotRequest),
        ),
        (
            "set-instance-snapshot-pinned-request",
            schema_for!(SetInstanceSnapshotPinnedRequest),
        ),
        ("app-preferences", schema_for!(AppPreferencesDto)),
        (
            "onboarding-state-summary",
            schema_for!(OnboardingStateSummary),
        ),
        ("support-report-preview", schema_for!(SupportReportPreview)),
        (
            "create-support-report-request",
            schema_for!(CreateSupportReportRequest),
        ),
        ("support-report-export", schema_for!(SupportReportExport)),
        ("preflight-summary", schema_for!(PreflightSummary)),
        ("storage-overview", schema_for!(StorageOverview)),
        (
            "clear-storage-category-request",
            schema_for!(ClearStorageCategoryRequest),
        ),
        ("storage-cleanup-result", schema_for!(StorageCleanupResult)),
        (
            "restore-trashed-instance-request",
            schema_for!(RestoreTrashedInstanceRequest),
        ),
        (
            "delete-trashed-instance-request",
            schema_for!(DeleteTrashedInstanceRequest),
        ),
        (
            "empty-instance-trash-request",
            schema_for!(EmptyInstanceTrashRequest),
        ),
        (
            "minecraft-version-catalog",
            schema_for!(MinecraftVersionCatalog),
        ),
        (
            "loader-versions-request",
            schema_for!(LoaderVersionsRequest),
        ),
        ("loader-version-catalog", schema_for!(LoaderVersionCatalog)),
        ("redacted-launch-plan", schema_for!(RedactedLaunchPlan)),
        (
            "install-instance-request",
            schema_for!(InstallInstanceRequest),
        ),
        (
            "cancel-install-job-request",
            schema_for!(CancelInstallJobRequest),
        ),
        (
            "retry-install-job-request",
            schema_for!(RetryInstallJobRequest),
        ),
        ("install-job-summary", schema_for!(InstallJobSummary)),
        (
            "launch-instance-request",
            schema_for!(LaunchInstanceRequest),
        ),
        ("game-session-summary", schema_for!(GameSessionSummary)),
        ("game-session-list", schema_for!(Vec<GameSessionSummary>)),
        (
            "stop-game-session-request",
            schema_for!(StopGameSessionRequest),
        ),
        (
            "subscribe-session-log-request",
            schema_for!(SubscribeSessionLogRequest),
        ),
        (
            "unsubscribe-session-log-request",
            schema_for!(UnsubscribeSessionLogRequest),
        ),
        (
            "session-log-subscription",
            schema_for!(SessionLogSubscription),
        ),
        ("session-log-event", schema_for!(SessionLogEvent)),
        ("saved-server-summary", schema_for!(SavedServerSummary)),
        ("saved-server-list", schema_for!(Vec<SavedServerSummary>)),
        (
            "create-saved-server-request",
            schema_for!(CreateSavedServerRequest),
        ),
        (
            "update-saved-server-request",
            schema_for!(UpdateSavedServerRequest),
        ),
        (
            "remove-saved-server-request",
            schema_for!(RemoveSavedServerRequest),
        ),
        ("ping-server-request", schema_for!(PingServerRequest)),
        ("server-status-summary", schema_for!(ServerStatusSummary)),
        ("modpack-search-request", schema_for!(ModpackSearchRequest)),
        (
            "modpack-search-response",
            schema_for!(slate_modpack_api_contracts::SearchResponse),
        ),
        (
            "modpack-project-request",
            schema_for!(ModpackProjectRequest),
        ),
        (
            "modpack-project",
            schema_for!(slate_modpack_api_contracts::Modpack),
        ),
        (
            "modpack-versions-request",
            schema_for!(ModpackVersionsRequest),
        ),
        (
            "modpack-version-page",
            schema_for!(slate_modpack_api_contracts::VersionPage),
        ),
        (
            "modpack-version-request",
            schema_for!(ModpackVersionRequest),
        ),
        (
            "modpack-version",
            schema_for!(slate_modpack_api_contracts::ModpackVersion),
        ),
        (
            "install-modpack-request",
            schema_for!(InstallModpackRequest),
        ),
        (
            "modpack-install-started",
            schema_for!(ModpackInstallStarted),
        ),
        (
            "check-modpack-update-request",
            schema_for!(CheckModpackUpdateRequest),
        ),
        (
            "apply-modpack-update-request",
            schema_for!(ApplyModpackUpdateRequest),
        ),
        ("modpack-update-summary", schema_for!(ModpackUpdateSummary)),
        ("mod-search-request", schema_for!(ModSearchRequest)),
        ("install-mod-request", schema_for!(InstallModRequest)),
        ("instance-mods-request", schema_for!(InstanceModsRequest)),
        (
            "set-instance-mod-enabled-request",
            schema_for!(SetInstanceModEnabledRequest),
        ),
        (
            "set-instance-mod-pinned-request",
            schema_for!(SetInstanceModPinnedRequest),
        ),
        (
            "remove-instance-mod-request",
            schema_for!(RemoveInstanceModRequest),
        ),
        ("instance-mod-list", schema_for!(Vec<InstanceModSummary>)),
        (
            "instance-content-files-request",
            schema_for!(InstanceContentFilesRequest),
        ),
        (
            "instance-content-file-list",
            schema_for!(Vec<InstanceContentFileSummary>),
        ),
        (
            "set-instance-content-file-enabled-request",
            schema_for!(SetInstanceContentFileEnabledRequest),
        ),
        (
            "remove-instance-content-file-request",
            schema_for!(RemoveInstanceContentFileRequest),
        ),
        (
            "instance-mod-resolution-list",
            schema_for!(Vec<InstanceModResolution>),
        ),
        (
            "minecraft-account-summary",
            schema_for!(MinecraftAccountSummary),
        ),
        ("auth-start-response", schema_for!(AuthStartResponse)),
        ("auth-flow-status", schema_for!(AuthFlowStatus)),
    ])
}

#[cfg(test)]
mod tests {
    use super::schema_documents;
    use crate::{BootstrapResponse, CapabilitySummary};

    #[test]
    fn bootstrap_wire_fields_are_camel_case() -> Result<(), serde_json::Error> {
        let value =
            serde_json::to_value(BootstrapResponse::new(vec![CapabilitySummary::available(
                "local.launch",
            )]))?;

        assert_eq!(value["productName"], "slate");
        assert!(value.get("ipcSchemaVersion").is_some());
        assert!(value.get("databaseSchemaVersion").is_some());
        assert!(value.get("product_name").is_none());
        Ok(())
    }

    #[test]
    fn schema_registry_has_stable_names() {
        let schemas = schema_documents();

        assert_eq!(schemas.len(), 87);
        assert!(schemas.contains_key("app-error"));
        assert!(schemas.contains_key("event-envelope"));
    }
}
