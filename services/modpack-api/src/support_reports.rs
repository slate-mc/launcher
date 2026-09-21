use crate::config::SupportReportStorageConfig;
use axum::body::Bytes;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path;
use object_store::{ObjectStore, PutMode, PutOptions, PutPayload};
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

pub const MAX_SUPPORT_REPORT_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct SupportReportStore {
    inner: Option<Arc<dyn ObjectStore>>,
    prefix: String,
}

impl SupportReportStore {
    pub fn new(config: &SupportReportStorageConfig) -> Result<Self, SupportReportStoreError> {
        let Some(bucket) = config.bucket.as_deref() else {
            return Ok(Self {
                inner: None,
                prefix: config.prefix.clone(),
            });
        };
        let store = AmazonS3Builder::from_env()
            .with_bucket_name(bucket)
            .build()?;
        Ok(Self {
            inner: Some(Arc::new(store)),
            prefix: config.prefix.clone(),
        })
    }

    #[must_use]
    pub const fn available(&self) -> bool {
        self.inner.is_some()
    }

    pub async fn put(
        &self,
        report_id: Uuid,
        archive: Bytes,
    ) -> Result<(), SupportReportStoreError> {
        let store = self
            .inner
            .as_ref()
            .ok_or(SupportReportStoreError::Unavailable)?;
        let now = OffsetDateTime::now_utc();
        let path = Path::from(format!(
            "{}/{:04}/{:02}/{:02}/{report_id}.zip",
            self.prefix,
            now.year(),
            u8::from(now.month()),
            now.day()
        ));
        store
            .put_opts(
                &path,
                PutPayload::from_bytes(archive),
                PutOptions {
                    mode: PutMode::Create,
                    ..PutOptions::default()
                },
            )
            .await?;
        Ok(())
    }

    #[cfg(test)]
    fn for_test(store: Arc<dyn ObjectStore>) -> Self {
        Self {
            inner: Some(store),
            prefix: "support-reports".to_owned(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SupportReportStoreError {
    #[error("support report storage is unavailable")]
    Unavailable,
    #[error("support report storage could not be configured")]
    Configuration(#[from] object_store::Error),
}

#[cfg(test)]
mod tests {
    use super::SupportReportStore;
    use axum::body::Bytes;
    use futures_util::StreamExt;
    use object_store::ObjectStore;
    use object_store::memory::InMemory;
    use object_store::path::Path;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn stores_private_report_under_generated_partition()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = Arc::new(InMemory::new());
        let reports = SupportReportStore::for_test(store.clone());
        let report_id = Uuid::new_v4();

        reports
            .put(report_id, Bytes::from_static(b"PK\x03\x04archive"))
            .await?;

        let objects = store
            .list(Some(&Path::from("support-reports")))
            .collect::<Vec<_>>()
            .await;
        assert_eq!(objects.len(), 1);
        let object = objects
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("stored report was not listed"))??;
        assert!(
            object
                .location
                .as_ref()
                .ends_with(&format!("{report_id}.zip"))
        );
        Ok(())
    }
}
