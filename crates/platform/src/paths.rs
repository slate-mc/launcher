use directories::BaseDirs;
use slate_domain::{InstanceId, PRODUCT_NAME};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    app_data: PathBuf,
    storage_root: PathBuf,
    instance_roots: Arc<HashMap<InstanceId, PathBuf>>,
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppPathsError> {
        let base = BaseDirs::new().ok_or(AppPathsError::BaseDirectoryUnavailable)?;
        let app_data = base.data_local_dir().join(PRODUCT_NAME);
        Ok(Self::from_roots(app_data.clone(), app_data.join("storage")))
    }

    #[must_use]
    pub fn from_roots(app_data: PathBuf, storage_root: PathBuf) -> Self {
        Self {
            app_data,
            storage_root,
            instance_roots: Arc::new(HashMap::new()),
        }
    }

    /// Return a request-scoped path view for an instance stored outside the default root.
    /// Shared artifacts and runtimes remain in the primary slate storage root.
    #[must_use]
    pub fn with_instance_root(&self, instance_id: InstanceId, root: PathBuf) -> Self {
        let mut instance_roots = self.instance_roots.as_ref().clone();
        instance_roots.insert(instance_id, root);
        Self {
            app_data: self.app_data.clone(),
            storage_root: self.storage_root.clone(),
            instance_roots: Arc::new(instance_roots),
        }
    }

    #[must_use]
    pub fn app_data(&self) -> &Path {
        &self.app_data
    }

    #[must_use]
    pub fn storage_root(&self) -> &Path {
        &self.storage_root
    }

    #[must_use]
    pub fn state_database(&self) -> PathBuf {
        self.app_data.join("state.sqlite")
    }

    #[must_use]
    pub fn logs(&self) -> PathBuf {
        self.app_data.join("logs")
    }

    #[must_use]
    pub fn jobs(&self) -> PathBuf {
        self.app_data.join("jobs")
    }

    #[must_use]
    pub fn artifacts(&self) -> PathBuf {
        self.storage_root.join("artifacts")
    }

    #[must_use]
    pub fn runtimes(&self) -> PathBuf {
        self.storage_root.join("runtimes")
    }

    #[must_use]
    pub fn java_runtimes(&self) -> PathBuf {
        self.runtimes().join("java")
    }

    #[must_use]
    pub fn java_runtime_family(&self, major: u32, platform_arch: &str) -> PathBuf {
        self.java_runtimes()
            .join(major.to_string())
            .join(platform_arch)
    }

    #[must_use]
    pub fn instances(&self) -> PathBuf {
        self.storage_root.join("instances")
    }

    #[must_use]
    pub fn instance(&self, instance_id: InstanceId) -> PathBuf {
        self.instance_roots
            .get(&instance_id)
            .cloned()
            .unwrap_or_else(|| self.instances().join(instance_id.to_string()))
    }

    #[must_use]
    pub fn media(&self) -> PathBuf {
        self.storage_root.join("media")
    }

    #[must_use]
    pub fn trash(&self) -> PathBuf {
        self.storage_root.join("trash")
    }

    pub fn ensure_base_directories(&self) -> Result<(), AppPathsError> {
        for path in [
            &self.app_data,
            &self.logs(),
            &self.jobs(),
            &self.artifacts(),
            &self.runtimes(),
            &self.java_runtimes(),
            &self.instances(),
            &self.media(),
            &self.trash(),
        ] {
            std::fs::create_dir_all(path).map_err(|source| AppPathsError::CreateDirectory {
                path: path.to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppPathsError {
    #[error("the operating system did not provide a local data directory")]
    BaseDirectoryUnavailable,
    #[error("failed to create managed directory {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::AppPaths;
    use slate_domain::InstanceId;

    #[test]
    fn creates_only_the_declared_base_layout() -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let app_data = temporary.path().join("app-data");
        let storage = temporary.path().join("storage");
        let paths = AppPaths::from_roots(app_data, storage);

        paths.ensure_base_directories()?;

        assert!(
            paths
                .state_database()
                .parent()
                .is_some_and(std::path::Path::exists)
        );
        assert!(paths.logs().is_dir());
        assert!(paths.artifacts().is_dir());
        assert!(paths.instances().is_dir());
        assert!(!paths.instance(InstanceId::new()).exists());
        Ok(())
    }

    #[test]
    fn an_instance_override_does_not_move_shared_artifacts() {
        let id = InstanceId::new();
        let paths = AppPaths::from_roots("app-data".into(), "storage".into());
        let moved = paths.with_instance_root(id, "other/instances/custom".into());

        assert_eq!(
            moved.instance(id),
            std::path::Path::new("other/instances/custom")
        );
        assert_eq!(moved.artifacts(), std::path::Path::new("storage/artifacts"));
        assert_eq!(
            paths.instance(id),
            std::path::Path::new("storage/instances").join(id.to_string())
        );
    }
}
