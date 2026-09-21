use slate_modpack_api_contracts::{CaptureProductEventRequest, ProductEvent, ProductPlatform};
use slate_modpack_client::ModpackApiClient;
use slate_storage::{Database, StorageError};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Clone, Debug)]
pub(super) struct ProductTelemetry {
    database: Database,
    client: ModpackApiClient,
    installation_id: uuid::Uuid,
    enabled: Arc<AtomicBool>,
    wake: Arc<Notify>,
}

impl ProductTelemetry {
    pub(super) async fn new(
        database: Database,
        client: ModpackApiClient,
    ) -> Result<Self, StorageError> {
        let installation_id = database.get_or_create_telemetry_installation_id().await?;
        let enabled = database.get_app_preferences().await?.telemetry_enabled;
        if !enabled {
            database.clear_product_events().await?;
        }
        Ok(Self {
            database,
            client,
            installation_id,
            enabled: Arc::new(AtomicBool::new(enabled)),
            wake: Arc::new(Notify::new()),
        })
    }

    pub(super) async fn set_enabled(&self, enabled: bool) {
        let was_enabled = self.enabled.swap(enabled, Ordering::AcqRel);
        if enabled && !was_enabled {
            if self
                .capture(ProductEvent::TelemetryEnabled, None, None)
                .await
                .is_err()
            {
                tracing::warn!(
                    "anonymous usage preference was saved but its first event was not queued"
                );
            }
            self.wake.notify_one();
        } else if !enabled && self.database.clear_product_events().await.is_err() {
            tracing::warn!("anonymous usage was disabled; its inactive queue could not be cleared");
        }
    }

    pub(super) async fn capture(
        &self,
        event: ProductEvent,
        loader: Option<&str>,
        provider: Option<&str>,
    ) -> Result<(), StorageError> {
        if !self.enabled.load(Ordering::Acquire) {
            return Ok(());
        }
        self.database
            .enqueue_product_event(&CaptureProductEventRequest {
                installation_id: self.installation_id,
                event,
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
                platform: current_platform(),
                architecture: std::env::consts::ARCH.to_owned(),
                loader: loader.map(str::to_owned),
                provider: provider.map(str::to_owned),
            })
            .await?;
        self.wake.notify_one();
        Ok(())
    }

    pub(super) async fn run(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = self.wake.notified() => {}
            }
            if !self.enabled.load(Ordering::Acquire) {
                continue;
            }
            let events = match self.database.queued_product_events(50).await {
                Ok(events) => events,
                Err(_) => {
                    tracing::warn!("anonymous usage queue could not be read");
                    continue;
                }
            };
            for event in events {
                match self.client.capture_product_event(&event.request).await {
                    Ok(_) => {
                        if self.database.remove_product_event(event.id).await.is_err() {
                            tracing::warn!("anonymous usage queue could not be advanced");
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = self.database.mark_product_event_attempt(event.id).await;
                        tracing::debug!("anonymous usage delivery will be retried later");
                        break;
                    }
                }
            }
        }
    }
}

const fn current_platform() -> ProductPlatform {
    if cfg!(target_os = "windows") {
        ProductPlatform::Windows
    } else if cfg!(target_os = "linux") {
        ProductPlatform::Linux
    } else if cfg!(target_os = "macos") {
        ProductPlatform::Macos
    } else {
        ProductPlatform::Other
    }
}
