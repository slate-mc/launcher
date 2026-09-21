use super::*;
use slate_modpack_api_contracts::LauncherFeatureConfig;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub(super) struct FeatureControls {
    database: Database,
    client: ModpackApiClient,
    installation_id: Uuid,
    current: Arc<RwLock<FeatureSnapshot>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EvaluatedFeatures {
    pub installs_enabled: bool,
    pub launch_enabled: bool,
    pub authentication_enabled: bool,
    pub support_reports_enabled: bool,
    pub discover_preview_enabled: bool,
}

#[derive(Clone, Copy, Debug)]
struct FeatureSnapshot {
    features: EvaluatedFeatures,
    expires_at_unix: i64,
}

impl FeatureControls {
    pub(super) async fn new(
        database: Database,
        client: ModpackApiClient,
    ) -> Result<Self, StorageError> {
        let installation_id = database.get_or_create_telemetry_installation_id().await?;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let snapshot = database
            .cached_launcher_feature_config(now)
            .await?
            .map_or_else(
                || FeatureSnapshot {
                    features: evaluate(LauncherFeatureConfig::default(), installation_id),
                    expires_at_unix: i64::MAX,
                },
                |(config, expires_at_unix)| FeatureSnapshot {
                    features: evaluate(config, installation_id),
                    expires_at_unix,
                },
            );
        Ok(Self {
            database,
            client,
            installation_id,
            current: Arc::new(RwLock::new(snapshot)),
        })
    }

    pub(super) fn snapshot(&self) -> EvaluatedFeatures {
        self.current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .features
    }

    pub(super) fn require(&self, access: FeatureAccess) -> Result<(), AppError> {
        let features = self.snapshot();
        let enabled = match access {
            FeatureAccess::Install => features.installs_enabled,
            FeatureAccess::Launch => features.launch_enabled,
            FeatureAccess::Authentication => features.authentication_enabled,
            FeatureAccess::SupportReport => features.support_reports_enabled,
        };
        if enabled {
            Ok(())
        } else {
            Err(AppError::new(
                "service.temporarily_unavailable",
                "This feature is temporarily unavailable. Try again later.",
            )
            .retryable(true))
        }
    }

    pub(super) async fn run(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            self.refresh().await;
        }
    }

    async fn refresh(&self) {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        match self.client.launcher_feature_config().await {
            Ok(config) => {
                let expires_at = self
                    .database
                    .save_launcher_feature_config(&config, now)
                    .await
                    .unwrap_or_else(|_| now.saturating_add(i64::from(config.cache_seconds)));
                let mut current = self
                    .current
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *current = FeatureSnapshot {
                    features: evaluate(config, self.installation_id),
                    expires_at_unix: expires_at,
                };
            }
            Err(_) => {
                let mut current = self
                    .current
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if current.expires_at_unix <= now {
                    *current = FeatureSnapshot {
                        features: evaluate(LauncherFeatureConfig::default(), self.installation_id),
                        expires_at_unix: i64::MAX,
                    };
                }
                tracing::debug!("remote feature controls will be refreshed later");
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum FeatureAccess {
    Install,
    Launch,
    Authentication,
    SupportReport,
}

fn evaluate(config: LauncherFeatureConfig, installation_id: Uuid) -> EvaluatedFeatures {
    let rollout_bucket = u8::try_from(installation_id.as_u128() % 100).unwrap_or_default();
    EvaluatedFeatures {
        installs_enabled: config.installs_enabled,
        launch_enabled: config.launch_enabled,
        authentication_enabled: config.authentication_enabled,
        support_reports_enabled: config.support_reports_enabled,
        discover_preview_enabled: rollout_bucket < config.discover_preview_rollout.min(100),
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate;
    use slate_modpack_api_contracts::LauncherFeatureConfig;
    use uuid::Uuid;

    #[test]
    fn rollout_assignment_is_stable_and_respects_boundaries() {
        let id = Uuid::from_u128(42);
        let disabled = evaluate(LauncherFeatureConfig::default(), id);
        assert!(!disabled.discover_preview_enabled);
        let enabled = evaluate(
            LauncherFeatureConfig {
                discover_preview_rollout: 100,
                ..LauncherFeatureConfig::default()
            },
            id,
        );
        assert!(enabled.discover_preview_enabled);
        assert!(enabled.installs_enabled);
    }
}
