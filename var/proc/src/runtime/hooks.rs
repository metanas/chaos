use super::*;
use chaos_ipc::hooks::{HookDefinition, HookRegistration};

macro_rules! hooks_backend {
    ($runtime:ty) => {
        impl $runtime {
            async fn import_hooks(&self, hooks: &[(String, HookDefinition)]) -> anyhow::Result<()> {
                let mut tx = self.pool().begin().await?;
                for (id, definition) in hooks {
                    let revision: i64 = sqlx::query(
                        "UPDATE hook_revision SET revision = revision + 1 WHERE id = 1 RETURNING revision",
                    ).fetch_one(&mut *tx).await?.try_get("revision")?;
                    sqlx::query("INSERT INTO hooks (id, revision, definition_json, enabled) VALUES ($1,$2,$3,0)")
                        .bind(id).bind(revision).bind(serde_json::to_string(definition)?)
                        .execute(&mut *tx).await?;
                }
                sqlx::query("INSERT INTO configuration_events (id, event, subject, created_at) VALUES ($1,'hooks_imported',$2,$3)")
                    .bind(Uuid::now_v7().to_string()).bind(hooks.len().to_string()).bind(settings::now()?)
                    .execute(&mut *tx).await?;
                tx.commit().await?;
                Ok(())
            }

            async fn list_hooks(&self, installation: &str) -> anyhow::Result<Vec<HookRegistration>> {
                let rows = sqlx::query(
                    "SELECT h.id, h.revision, h.definition_json, h.enabled, \
                     EXISTS(SELECT 1 FROM remembered_approvals a WHERE a.kind = 'hook' \
                     AND a.subject = h.id AND a.identity = CAST(h.revision AS TEXT) \
                     AND a.installation_id = $1 AND a.state = 'active') AS approved \
                     FROM hooks h ORDER BY h.id",
                )
                .bind(installation)
                .fetch_all(self.pool())
                .await?;
                rows.into_iter().map(|row| Ok(HookRegistration {
                    id: row.try_get("id")?,
                    revision: row.try_get("revision")?,
                    definition: serde_json::from_str(&row.try_get::<String, _>("definition_json")?)?,
                    enabled: row.try_get::<i64, _>("enabled")? != 0,
                    approved: row.try_get("approved")?,
                })).collect()
            }

            /// Caller must obtain operator authorization before entering this transaction.
            /// expected_revision=0 creates; None definition deletes. All writes are CAS.
            async fn commit_hook(
                &self,
                id: &str,
                expected_revision: i64,
                definition: Option<&HookDefinition>,
                enabled: bool,
                installation: &str,
            ) -> anyhow::Result<i64> {
                anyhow::ensure!(!id.is_empty() && expected_revision >= 0, "invalid hook identity");
                anyhow::ensure!(definition.is_some() || expected_revision > 0, "cannot delete absent hook");
                let mut tx = self.pool().begin().await?;
                // Serialize writes and never reuse revisions, including after delete/recreate.
                let revision: i64 = sqlx::query(
                    "UPDATE hook_revision SET revision = revision + 1 WHERE id = 1 RETURNING revision",
                ).fetch_one(&mut *tx).await?.try_get("revision")?;
                let result = match definition {
                    Some(definition) if expected_revision == 0 => sqlx::query(
                        "INSERT INTO hooks (id, revision, definition_json, enabled) VALUES ($1,$2,$3,$4) ON CONFLICT (id) DO NOTHING",
                    ).bind(id).bind(revision).bind(serde_json::to_string(definition)?)
                        .bind(i64::from(enabled)).execute(&mut *tx).await?,
                    Some(definition) => sqlx::query(
                        "UPDATE hooks SET revision = $1, definition_json = $2, enabled = $3 WHERE id = $4 AND revision = $5",
                    ).bind(revision).bind(serde_json::to_string(definition)?).bind(i64::from(enabled))
                        .bind(id).bind(expected_revision).execute(&mut *tx).await?,
                    None => sqlx::query("DELETE FROM hooks WHERE id = $1 AND revision = $2")
                        .bind(id).bind(expected_revision).execute(&mut *tx).await?,
                };
                anyhow::ensure!(result.rows_affected() == 1, "hook revision conflict; reload and request fresh approval");
                // Any edit invalidates every installation's old approval.
                sqlx::query("UPDATE remembered_approvals SET state = 'revoked' WHERE kind = 'hook' AND subject = $1")
                    .bind(id).execute(&mut *tx).await?;
                let timestamp = settings::now()?;
                if let Some(definition) = definition.filter(|_| enabled) {
                    sqlx::query(
                        "INSERT INTO remembered_approvals (id, installation_id, scope, kind, subject, identity, state, payload_json, created_at, updated_at) \
                         VALUES ($1,$2,$3,'hook',$4,$5,'active','{}',$6,$6)",
                    ).bind(Uuid::now_v7().to_string()).bind(installation)
                        .bind(definition.project.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "global".into()))
                        .bind(id).bind(revision.to_string()).bind(timestamp).execute(&mut *tx).await?;
                }
                sqlx::query("UPDATE approval_revision SET revision = revision + 1 WHERE id = 1")
                    .execute(&mut *tx).await?;
                sqlx::query("INSERT INTO configuration_events (id, event, subject, created_at) VALUES ($1,$2,$3,$4)")
                    .bind(Uuid::now_v7().to_string())
                    .bind(if definition.is_some() { "hook_updated" } else { "hook_deleted" })
                    .bind(id).bind(timestamp).execute(&mut *tx).await?;
                tx.commit().await?;
                Ok(revision)
            }
        }
    };
}

hooks_backend!(StateRuntime);
hooks_backend!(PostgresRuntime);

impl RuntimeDbHandle {
    chaos_dispatch::backend_dispatch! {
        pub async fn import_hooks(&self, hooks: &[(String, HookDefinition)]) -> anyhow::Result<()>;
        pub async fn list_hooks(&self, installation: &str) -> anyhow::Result<Vec<HookRegistration>>;
        pub async fn commit_hook(&self, id: &str, expected_revision: i64, definition: Option<&HookDefinition>, enabled: bool, installation: &str) -> anyhow::Result<i64>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hooks_cas_approval_and_delete_recreate() -> anyhow::Result<()> {
        let home = tempfile::tempdir()?;
        let db =
            RuntimeDbHandle::Sqlite(StateRuntime::init(home.path().into(), "test".into()).await?);
        let definition: HookDefinition = serde_json::from_value(serde_json::json!({
            "event": "before_turn", "command": "printf hello"
        }))?;
        let first = db
            .commit_hook("hello", 0, Some(&definition), true, "one")
            .await?;
        assert!(db.list_hooks("one").await?[0].approved);
        assert!(!db.list_hooks("two").await?[0].approved);
        assert!(
            db.commit_hook("hello", 0, Some(&definition), true, "one")
                .await
                .is_err()
        );
        db.revoke_approvals("one", None).await?;
        assert!(!db.list_hooks("one").await?[0].approved);
        let second = db
            .commit_hook("hello", first, Some(&definition), true, "two")
            .await?;
        assert!(!db.list_hooks("one").await?[0].approved);
        assert!(db.list_hooks("two").await?[0].approved);
        assert!(
            db.commit_hook("hello", first, None, false, "one")
                .await
                .is_err()
        );
        db.commit_hook("hello", second, None, false, "two").await?;
        let third = db
            .commit_hook("hello", 0, Some(&definition), false, "one")
            .await?;
        assert!(third > second);
        assert!(!db.list_hooks("two").await?[0].approved);
        assert!(
            db.import_hooks(&[
                ("fresh".into(), definition.clone()),
                ("hello".into(), definition)
            ])
            .await
            .is_err()
        );
        assert_eq!(
            db.list_hooks("one").await?.len(),
            1,
            "import must roll back as a unit"
        );
        Ok(())
    }
}
