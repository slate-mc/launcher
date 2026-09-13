use directories::BaseDirs;
use slate_domain::{InstanceId, PRODUCT_NAME};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    app_data: PathBuf,
    storage_root: PathBuf,
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
        self.instances().join(instance_id.to_string())
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
}
