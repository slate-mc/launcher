use super::*;

impl Database {
    pub async fn complete_instance_install(
        &self,
        job_id: JobId,
        revision_id: RevisionId,
        manifest_digest: &str,
        client_version: &str,
        runtime: InstalledRuntime,
        message: &str,
    ) -> Result<(), StorageError> {
        self.complete_instance_install_inner(
            CompletedInstall {
                job_id,
                revision_id,
                manifest_digest: manifest_digest.to_owned(),
                client_version: client_version.to_owned(),
                runtime,
                message: message.to_owned(),
            },
            None,
            None,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn complete_instance_mod_install(
        &self,
        completion: CompletedInstall,
        installed_mods: Vec<NewInstanceMod>,
        dependency_sets: Vec<NewInstanceModDependencySet>,
    ) -> Result<(), StorageError> {
        self.complete_instance_install_inner(
            completion,
            Some(installed_mods),
            None,
            Some(dependency_sets),
            None,
            None,
        )
        .await
    }

    pub async fn complete_instance_mod_update(
        &self,
        completion: CompletedInstall,
        installed_mods: Vec<NewInstanceMod>,
        replaced_file_paths: Vec<String>,
        dependency_sets: Vec<NewInstanceModDependencySet>,
    ) -> Result<(), StorageError> {
        self.complete_instance_install_inner(
            completion,
            Some(installed_mods),
            Some(replaced_file_paths),
            Some(dependency_sets),
            None,
            None,
        )
        .await
    }

    pub async fn complete_instance_modpack_update(
        &self,
        completion: CompletedInstall,
        update: CompletedModpackUpdate,
    ) -> Result<(), StorageError> {
        self.complete_instance_install_inner(completion, None, None, None, Some(update), None)
            .await
    }

    pub async fn complete_instance_provider_content_install(
        &self,
        completion: CompletedInstall,
        installed_content: Vec<NewInstanceProviderContent>,
        replaced_file_paths: Vec<String>,
    ) -> Result<(), StorageError> {
        self.complete_instance_install_inner(
            completion,
            None,
            None,
            None,
            None,
            Some((installed_content, replaced_file_paths)),
        )
        .await
    }

    pub async fn complete_instance_mixed_content_install(
        &self,
        completion: CompletedInstall,
        installed_mods: Vec<NewInstanceMod>,
        replaced_mod_paths: Vec<String>,
        dependency_sets: Vec<NewInstanceModDependencySet>,
        installed_content: Vec<NewInstanceProviderContent>,
        replaced_content_paths: Vec<String>,
    ) -> Result<(), StorageError> {
        self.complete_instance_install_inner(
            completion,
            Some(installed_mods),
            Some(replaced_mod_paths),
            Some(dependency_sets),
            None,
            Some((installed_content, replaced_content_paths)),
        )
        .await
    }

    async fn complete_instance_install_inner(
        &self,
        completion: CompletedInstall,
        installed_mods: Option<Vec<NewInstanceMod>>,
        replaced_mod_paths: Option<Vec<String>>,
        dependency_sets: Option<Vec<NewInstanceModDependencySet>>,
        modpack_update: Option<CompletedModpackUpdate>,
        provider_content: Option<(Vec<NewInstanceProviderContent>, Vec<String>)>,
    ) -> Result<(), StorageError> {
        let now = now_rfc3339()?;
        let runtime_id = Uuid::new_v4().to_string();
        let mut transaction = self.pool.begin().await?;
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM jobs j \
         INNER JOIN instances i ON i.id = j.entity_id \
         WHERE j.id = ? AND j.state IN ('queued', 'running', 'paused') AND i.trashed_at IS NULL)",
        )
        .bind(completion.job_id.to_string())
        .fetch_one(&mut *transaction)
        .await?;
        if !active {
            transaction.rollback().await?;
            return Err(StorageError::InstallNoLongerActive);
        }
        sqlx::query(
            "INSERT INTO runtime_installations \
         (id, vendor, release_name, java_version, major, os, arch, executable_ref, \
          source_digest, managed, verified_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?) \
         ON CONFLICT(vendor, release_name, os, arch) DO UPDATE SET \
          java_version = excluded.java_version, executable_ref = excluded.executable_ref, \
          source_digest = excluded.source_digest, verified_at = excluded.verified_at",
        )
        .bind(&runtime_id)
        .bind(&completion.runtime.vendor)
        .bind(&completion.runtime.release_name)
        .bind(&completion.runtime.java_version)
        .bind(i64::from(completion.runtime.major))
        .bind(&completion.runtime.os)
        .bind(&completion.runtime.arch)
        .bind(&completion.runtime.executable_ref)
        .bind(&completion.runtime.source_digest)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        let stored_runtime_id: String = sqlx::query_scalar(
            "SELECT id FROM runtime_installations \
         WHERE vendor = ? AND release_name = ? AND os = ? AND arch = ?",
        )
        .bind(&completion.runtime.vendor)
        .bind(&completion.runtime.release_name)
        .bind(&completion.runtime.os)
        .bind(&completion.runtime.arch)
        .fetch_one(&mut *transaction)
        .await?;

        let instance_id: String =
            sqlx::query_scalar("SELECT instance_id FROM instance_revisions WHERE id = ?")
                .bind(completion.revision_id.to_string())
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or(StorageError::RevisionNotFound)?;
        if let Some(replaced_mod_paths) = replaced_mod_paths {
            for file_path in replaced_mod_paths {
                let replaced_mods = sqlx::query(
                    "SELECT provider, project_id, version_id, display_name, file_path \
                 FROM instance_mods WHERE instance_id = ? AND file_path = ?",
                )
                .bind(&instance_id)
                .bind(file_path.trim())
                .fetch_all(&mut *transaction)
                .await?;
                for replaced_mod in replaced_mods {
                    let provider: String = replaced_mod.try_get("provider")?;
                    let project_id: String = replaced_mod.try_get("project_id")?;
                    sqlx::query(
                        "INSERT INTO instance_mod_history \
                     (id, instance_id, provider, project_id, version_id, display_name, \
                      file_path, changed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(Uuid::new_v4().to_string())
                    .bind(&instance_id)
                    .bind(&provider)
                    .bind(&project_id)
                    .bind(replaced_mod.try_get::<String, _>("version_id")?)
                    .bind(replaced_mod.try_get::<String, _>("display_name")?)
                    .bind(replaced_mod.try_get::<String, _>("file_path")?)
                    .bind(&now)
                    .execute(&mut *transaction)
                    .await?;
                    sqlx::query(
                        "DELETE FROM instance_mod_history WHERE id IN (\
                     SELECT id FROM instance_mod_history WHERE instance_id = ? \
                     AND provider = ? AND project_id = ? ORDER BY changed_at DESC, id DESC \
                     LIMIT -1 OFFSET 20)",
                    )
                    .bind(&instance_id)
                    .bind(provider)
                    .bind(project_id)
                    .execute(&mut *transaction)
                    .await?;
                }
                sqlx::query(
                    "DELETE FROM instance_mod_dependencies WHERE instance_id = ? AND (\
                 EXISTS (SELECT 1 FROM instance_mods m WHERE m.instance_id = ? \
                 AND m.file_path = ? AND m.provider = root_provider \
                 AND m.project_id = root_project_id) OR \
                 EXISTS (SELECT 1 FROM instance_mods m WHERE m.instance_id = ? \
                 AND m.file_path = ? AND m.provider = dependency_provider \
                 AND m.project_id = dependency_project_id))",
                )
                .bind(&instance_id)
                .bind(&instance_id)
                .bind(file_path.trim())
                .bind(&instance_id)
                .bind(file_path.trim())
                .execute(&mut *transaction)
                .await?;
                sqlx::query("DELETE FROM instance_mods WHERE instance_id = ? AND file_path = ?")
                    .bind(&instance_id)
                    .bind(file_path.trim())
                    .execute(&mut *transaction)
                    .await?;
            }
        }
        if let Some(installed_mods) = installed_mods {
            for installed_mod in installed_mods {
                sqlx::query(
                    "INSERT INTO instance_mods \
             (instance_id, provider, project_id, version_id, display_name, file_path, \
              hashes_json, enabled, pinned, installed_at) \
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
              ON CONFLICT(instance_id, provider, project_id) DO UPDATE SET \
               version_id = excluded.version_id, display_name = excluded.display_name, \
               file_path = excluded.file_path, hashes_json = excluded.hashes_json, \
               enabled = excluded.enabled, pinned = excluded.pinned, \
               installed_at = excluded.installed_at",
                )
                .bind(&instance_id)
                .bind(installed_mod.provider.as_str())
                .bind(installed_mod.project_id.trim())
                .bind(installed_mod.version_id.trim())
                .bind(installed_mod.display_name.trim())
                .bind(installed_mod.file_path.trim())
                .bind(serde_json::to_string(&installed_mod.hashes)?)
                .bind(i64::from(installed_mod.enabled))
                .bind(i64::from(installed_mod.pinned))
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
            }
        }
        if let Some(dependency_sets) = dependency_sets {
            for dependency_set in dependency_sets {
                sqlx::query(
                    "DELETE FROM instance_mod_dependencies WHERE instance_id = ? \
                 AND root_provider = ? AND root_project_id = ?",
                )
                .bind(&instance_id)
                .bind(dependency_set.root_provider.as_str())
                .bind(dependency_set.root_project_id.trim())
                .execute(&mut *transaction)
                .await?;
                for dependency in dependency_set.dependencies {
                    sqlx::query(
                        "INSERT OR IGNORE INTO instance_mod_dependencies \
                     (instance_id, root_provider, root_project_id, dependency_provider, \
                      dependency_project_id) VALUES (?, ?, ?, ?, ?)",
                    )
                    .bind(&instance_id)
                    .bind(dependency_set.root_provider.as_str())
                    .bind(dependency_set.root_project_id.trim())
                    .bind(dependency.provider.as_str())
                    .bind(dependency.project_id.trim())
                    .execute(&mut *transaction)
                    .await?;
                }
            }
        }
        if let Some(update) = modpack_update {
            let updated_source = sqlx::query(
                "UPDATE instance_modpacks SET version_id = ?, updated_at = ? WHERE instance_id = ?",
            )
            .bind(update.version_id.trim())
            .bind(&now)
            .bind(&instance_id)
            .execute(&mut *transaction)
            .await?;
            if updated_source.rows_affected() != 1 {
                transaction.rollback().await?;
                return Err(StorageError::InvalidStoredValue {
                    field: "instance_modpacks.instance_id",
                    value: instance_id,
                });
            }
            sqlx::query(
                "UPDATE instance_configuration SET loader_version = ? WHERE instance_id = ?",
            )
            .bind(update.loader_version.as_deref())
            .bind(&instance_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query("UPDATE instance_revisions SET loader_version = ? WHERE id = ?")
                .bind(update.loader_version.as_deref())
                .bind(completion.revision_id.to_string())
                .execute(&mut *transaction)
                .await?;
        }
        if let Some((installed_content, replaced_paths)) = provider_content {
            insert_provider_content(
                &mut transaction,
                &instance_id,
                installed_content,
                replaced_paths,
            )
            .await?;
        }
        sqlx::query(
            "UPDATE instance_revisions SET manifest_digest = ?, client_version = ?, \
         runtime_id = ?, installed_at = ?, status = 'installed' WHERE id = ?",
        )
        .bind(&completion.manifest_digest)
        .bind(&completion.client_version)
        .bind(stored_runtime_id)
        .bind(&now)
        .bind(completion.revision_id.to_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("UPDATE instances SET active_revision_id = ?, updated_at = ? WHERE id = ?")
            .bind(completion.revision_id.to_string())
            .bind(&now)
            .bind(&instance_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "UPDATE instance_configuration SET setup_state = 'ready' WHERE instance_id = ?",
        )
        .bind(&instance_id)
        .execute(&mut *transaction)
        .await?;
        update_job_transaction(
            &mut transaction,
            completion.job_id,
            JobState::Succeeded,
            "complete",
            &completion.message,
            &now,
        )
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}
