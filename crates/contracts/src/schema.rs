use crate::{
    AppError, AppPreferencesDto, ApplyModpackUpdateRequest, AuthFlowStatus, AuthStartResponse,
    BootstrapResponse, CancelInstallJobRequest, CheckModpackUpdateRequest,
    ClearStorageCategoryRequest, ContentSearchRequest, CreateInstanceRequest,
    CreateInstanceSnapshotRequest, CreateSavedServerRequest, CreateSupportReportRequest,
    DeleteInstanceSnapshotRequest, DeleteTrashedInstanceRequest, DuplicateInstanceRequest,
    EmptyInstanceTrashRequest, EventEnvelope, ExportInstanceRequest, GameSessionSummary,
    GetInstanceArtworkRequest, GetInstanceGameOptionsRequest, ImportInstanceRequest,
    ImportLocalContentFileRequest, ImportLocalModRequest, InstallContentRequest,
    InstallInstanceRequest, InstallJobSummary, InstallModRequest, InstallModpackRequest,
    InstanceContentFileSummary, InstanceContentFilesRequest, InstanceContentHistoryRequest,
    InstanceContentHistorySummary, InstanceContentVersionsRequest, InstanceGameOptionsSummary,
    InstanceModHistoryRequest, InstanceModHistorySummary, InstanceModResolution,
    InstanceModSummary, InstanceModVersionsRequest, InstanceModsRequest, InstanceSessionsRequest,
    InstanceSnapshotSummary, InstanceSnapshotsRequest, InstanceSummary, InstanceWorldsRequest,
    LaunchInstanceRequest, LoaderVersionCatalog, LoaderVersionsRequest, MinecraftAccountSummary,
    MinecraftVersionCatalog, ModSearchRequest, ModpackInstallStarted, ModpackProjectRequest,
    ModpackSearchRequest, ModpackUpdateSummary, ModpackVersionRequest, ModpackVersionsRequest,
    MoveInstallJobRequest, MoveInstanceResourcePackRequest, MoveInstanceStorageRequest,
    OnboardingStateSummary, OpenInstanceDirectoryRequest, PingServerRequest, PreflightSummary,
    ReadSessionLogRequest, RedactedLaunchPlan, RemoveInstanceContentFileRequest,
    RemoveInstanceModRequest, RemoveSavedServerRequest, ResolveInstanceModRelationshipsRequest,
    RestoreInstanceSnapshotRequest, RestoreTrashedInstanceRequest, RetryInstallJobRequest,
    SavedServerSummary, SelectInstanceArtworkRequest, SelectInstanceJavaRequest,
    ServerStatusSummary, SessionHistorySummary, SessionLogEvent, SessionLogSnapshot,
    SessionLogSubscription, SetInstallJobPausedRequest, SetInstanceContentFileEnabledRequest,
    SetInstanceContentPinnedRequest, SetInstanceModEnabledRequest, SetInstanceModPinnedRequest,
    SetInstanceResourcePackActiveRequest, SetInstanceSnapshotPinnedRequest, StopGameSessionRequest,
    StorageCleanupResult, StorageOverview, SubscribeSessionLogRequest, SupportReportExport,
    SupportReportPreview, SupportReportSubmission, UnsubscribeSessionLogRequest,
    UpdateInstanceConfigurationRequest, UpdateInstanceContentRequest,
    UpdateInstanceGameOptionsRequest, UpdateInstanceModRequest, UpdateInstanceSettingsRequest,
    UpdateSavedServerRequest,
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
        (
            "support-report-submission",
            schema_for!(SupportReportSubmission),
        ),
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
        (
            "set-install-job-paused-request",
            schema_for!(SetInstallJobPausedRequest),
        ),
        (
            "move-install-job-request",
            schema_for!(MoveInstallJobRequest),
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
        (
            "instance-sessions-request",
            schema_for!(InstanceSessionsRequest),
        ),
        (
            "session-history-summary",
            schema_for!(SessionHistorySummary),
        ),
        (
            "session-history-list",
            schema_for!(Vec<SessionHistorySummary>),
        ),
        (
            "read-session-log-request",
            schema_for!(ReadSessionLogRequest),
        ),
        ("session-log-snapshot", schema_for!(SessionLogSnapshot)),
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
        ("content-search-request", schema_for!(ContentSearchRequest)),
        (
            "install-content-request",
            schema_for!(InstallContentRequest),
        ),
        ("install-mod-request", schema_for!(InstallModRequest)),
        ("instance-mods-request", schema_for!(InstanceModsRequest)),
        (
            "instance-mod-versions-request",
            schema_for!(InstanceModVersionsRequest),
        ),
        (
            "instance-mod-version-list",
            schema_for!(slate_modpack_api_contracts::ModVersionList),
        ),
        (
            "instance-mod-history-request",
            schema_for!(InstanceModHistoryRequest),
        ),
        (
            "instance-mod-history-list",
            schema_for!(Vec<InstanceModHistorySummary>),
        ),
        (
            "resolve-instance-mod-relationships-request",
            schema_for!(ResolveInstanceModRelationshipsRequest),
        ),
        (
            "instance-mod-reference-list",
            schema_for!(Vec<crate::InstanceModReferenceSummary>),
        ),
        (
            "update-instance-mod-request",
            schema_for!(UpdateInstanceModRequest),
        ),
        (
            "import-local-mod-request",
            schema_for!(ImportLocalModRequest),
        ),
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
            "instance-worlds-request",
            schema_for!(InstanceWorldsRequest),
        ),
        ("instance-world-list", schema_for!(Vec<String>)),
        (
            "import-local-content-file-request",
            schema_for!(ImportLocalContentFileRequest),
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
            "set-instance-content-pinned-request",
            schema_for!(SetInstanceContentPinnedRequest),
        ),
        (
            "instance-content-versions-request",
            schema_for!(InstanceContentVersionsRequest),
        ),
        (
            "instance-content-history-request",
            schema_for!(InstanceContentHistoryRequest),
        ),
        (
            "instance-content-history-list",
            schema_for!(Vec<InstanceContentHistorySummary>),
        ),
        (
            "update-instance-content-request",
            schema_for!(UpdateInstanceContentRequest),
        ),
        (
            "remove-instance-content-file-request",
            schema_for!(RemoveInstanceContentFileRequest),
        ),
        (
            "set-instance-resource-pack-active-request",
            schema_for!(SetInstanceResourcePackActiveRequest),
        ),
        (
            "move-instance-resource-pack-request",
            schema_for!(MoveInstanceResourcePackRequest),
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
    use crate::{BootstrapResponse, CapabilitySummary, InstanceModeDto};

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

        assert_eq!(schemas.len(), 115);
        assert!(schemas.contains_key("app-error"));
        assert!(schemas.contains_key("event-envelope"));
        assert!(schemas.contains_key("support-report-submission"));
    }

    #[test]
    fn slate_client_mode_has_a_product_level_wire_name() -> Result<(), serde_json::Error> {
        let value = serde_json::to_value(InstanceModeDto::SlateClient)?;

        assert_eq!(value, "slateClient");
        assert_ne!(value, "pvp");
        Ok(())
    }
}
