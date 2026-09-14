use crate::{
    AppError, AppPreferencesDto, AuthFlowStatus, AuthStartResponse, BootstrapResponse,
    CreateInstanceRequest, EventEnvelope, GameSessionSummary, InstallInstanceRequest,
    InstallJobSummary, InstallModRequest, InstallModpackRequest, InstanceModSummary,
    InstanceModsRequest, InstanceSummary, LaunchInstanceRequest, LoaderVersionCatalog,
    LoaderVersionsRequest, MinecraftAccountSummary, MinecraftVersionCatalog, ModSearchRequest,
    ModpackInstallStarted, ModpackProjectRequest, ModpackSearchRequest, ModpackVersionRequest,
    ModpackVersionsRequest, PreflightSummary, RedactedLaunchPlan, SessionLogEvent,
    SessionLogSubscription, StopGameSessionRequest, SubscribeSessionLogRequest,
    UnsubscribeSessionLogRequest, UpdateInstanceConfigurationRequest,
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
        ("app-preferences", schema_for!(AppPreferencesDto)),
        ("preflight-summary", schema_for!(PreflightSummary)),
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
        ("mod-search-request", schema_for!(ModSearchRequest)),
        ("install-mod-request", schema_for!(InstallModRequest)),
        ("instance-mods-request", schema_for!(InstanceModsRequest)),
        ("instance-mod-list", schema_for!(Vec<InstanceModSummary>)),
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

        assert_eq!(schemas.len(), 39);
        assert!(schemas.contains_key("app-error"));
        assert!(schemas.contains_key("event-envelope"));
    }
}
