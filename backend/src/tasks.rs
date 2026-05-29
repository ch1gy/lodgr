use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{crypto::EncryptionKey, db, services::export::export_client};

/// Runs every hour. For each recurring ticket template whose interval has
/// elapsed, creates a new non-recurring open ticket and updates the template.
pub async fn recurring_tickets(pool: SqlitePool) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;
        match db::tickets::list_due_for_recurrence(&pool).await {
            Ok(templates) => {
                for t in templates {
                    let new_id = Uuid::new_v4().to_string();
                    let result = db::tickets::create(
                        &pool,
                        db::tickets::NewTicket {
                            id: &new_id,
                            title: &format!("[Auto] {}", t.title),
                            description: &t.description,
                            created_by: &t.created_by,
                            client_id: &t.client_id,
                            priority: &t.priority,
                            category: t.category.as_deref(),
                            due_date: None,
                            estimated_completion: None,
                            ticket_type: &t.ticket_type,
                            recurring: false, // instances are not themselves recurring
                            recurring_interval_days: None,
                        },
                    )
                    .await;

                    match result {
                        Ok(_) => {
                            let now = Utc::now().to_rfc3339();
                            if let Err(e) =
                                db::tickets::update_last_recurred(&pool, &t.id, &now).await
                            {
                                tracing::warn!("update last_recurred_at failed for {}: {e}", t.id);
                            }
                            tracing::info!(
                                "recurring: created ticket {new_id} from template {}",
                                t.id
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "recurring: failed to create from template {}: {e}",
                                t.id
                            );
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("recurring tickets query failed: {e}"),
        }
    }
}

/// Runs every 24 h. Hard-deletes users soft-deleted more than 30 days ago.
/// Mirrors the manual path: ensures a pre-deletion export exists, generating
/// one if none has been taken, before cascading the delete.
pub async fn hard_delete_expired_users(pool: SqlitePool, enc_key: EncryptionKey) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86_400));
    interval.tick().await; // skip first tick — already ran at startup
    loop {
        interval.tick().await;
        let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        match db::users::find_expired_soft_deleted(&pool, &cutoff).await {
            Ok(users) => {
                for u in users {
                    // Ensure a pre-deletion export record exists. If not, generate
                    // one now — mirrors the guard in services::admin::hard_delete_client.
                    let has_export = match db::exports::exists_for_client(&pool, &u.id).await {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(
                                user_id = %u.id,
                                err = %e,
                                "auto hard-delete: export check failed — skipping"
                            );
                            continue;
                        }
                    };
                    if !has_export {
                        tracing::info!(
                            user_id = %u.id,
                            "auto hard-delete: no prior export found — generating now"
                        );
                        match export_client(&pool, &enc_key, &u.id).await {
                            Ok(_) => tracing::info!(
                                user_id = %u.id,
                                "auto hard-delete: export generated"
                            ),
                            Err(e) => {
                                tracing::warn!(
                                    user_id = %u.id,
                                    err = %e,
                                    "auto hard-delete: export failed — skipping delete"
                                );
                                continue;
                            }
                        }
                    }

                    // Collect ticket IDs before deletion for filesystem cleanup.
                    let ticket_ids: Vec<String> =
                        sqlx::query_scalar("SELECT id FROM tickets WHERE client_id = ?")
                            .bind(&u.id)
                            .fetch_all(&pool)
                            .await
                            .unwrap_or_default();

                    match cascade_hard_delete_user(&pool, &u.id).await {
                        Ok(()) => {
                            tracing::info!(user_id = %u.id, "auto hard-delete: user deleted");
                            for ticket_id in &ticket_ids {
                                let dir = format!("uploads/{ticket_id}");
                                if let Err(e) = tokio::fs::remove_dir_all(&dir).await {
                                    if e.kind() != std::io::ErrorKind::NotFound {
                                        tracing::warn!(
                                            dir = %dir,
                                            err = %e,
                                            "failed to remove upload dir during user hard delete"
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => tracing::warn!(user_id = %u.id, err = %e, "hard delete failed"),
                    }
                }
            }
            Err(e) => tracing::warn!("expired user query failed: {e}"),
        }
    }
}

/// Scans `exports/` and removes any file older than 24 hours.
/// Returns the number of files removed.
pub async fn clean_old_exports() -> std::io::Result<u32> {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(24 * 3600))
        .unwrap_or(std::time::UNIX_EPOCH);

    let mut removed = 0u32;
    let mut client_dirs = match tokio::fs::read_dir("exports").await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };

    while let Ok(Some(client_dir)) = client_dirs.next_entry().await {
        let Ok(mut files) = tokio::fs::read_dir(client_dir.path()).await else {
            continue;
        };
        while let Ok(Some(file)) = files.next_entry().await {
            let Ok(meta) = file.metadata().await else {
                continue;
            };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            if modified < cutoff && tokio::fs::remove_file(file.path()).await.is_ok() {
                removed += 1;
            }
        }
    }

    Ok(removed)
}

async fn cascade_hard_delete_user(pool: &SqlitePool, user_id: &str) -> crate::error::AppResult<()> {
    let mut tx = pool.begin().await?;
    crate::services::admin::cascade_delete_user_data(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(())
}
