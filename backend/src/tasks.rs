use chrono::{Months, NaiveDate, Utc};
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
                            sub_client_id: t.sub_client_id.as_deref(),
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

fn advance_recur_date(date_str: &str, interval: &str) -> Option<String> {
    let d = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
    let months = match interval {
        "quarterly" => 3,
        "yearly" => 12,
        _ => 1, // monthly is the default
    };
    let next = d.checked_add_months(Months::new(months))?;
    Some(next.format("%Y-%m-%d").to_string())
}

/// Runs every hour. Creates draft invoices from recurring templates whose
/// next_recur_date has arrived. Desk must review and confirm before sending.
pub async fn recurring_invoices(pool: SqlitePool) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;
        match db::invoices::list_due_for_recurrence(&pool).await {
            Ok(templates) => {
                for t in templates {
                    // Guard: skip templates missing recur_interval — would loop forever otherwise.
                    let Some(ref iv) = t.recur_interval else {
                        tracing::warn!(
                            "recurring invoice: template {} has no recur_interval — skipping",
                            t.id
                        );
                        continue;
                    };
                    let Some(ref cur_date) = t.next_recur_date else {
                        tracing::warn!(
                            "recurring invoice: template {} has no next_recur_date — skipping",
                            t.id
                        );
                        continue;
                    };
                    let Some(next_date) = advance_recur_date(cur_date, iv) else {
                        tracing::warn!(
                            "recurring invoice: could not advance date for template {} — skipping",
                            t.id
                        );
                        continue;
                    };

                    let new_id = Uuid::new_v4().to_string();
                    // Include a short UUID segment so the number is unique per auto-generation,
                    // preventing UNIQUE constraint collisions on restart or template reuse.
                    let suffix = &new_id[..8];
                    let number = format!("{}-auto-{}", t.number, suffix);
                    let today = Utc::now().format("%Y-%m-%d").to_string();
                    let items_json = t.items.clone();
                    let notes_json = t.notes.clone();

                    // Run create + date advance in a single transaction so a mid-flight
                    // crash does not leave the template stuck at the old date.
                    let tx_result: crate::error::AppResult<()> = async {
                        db::invoices::create(
                            &pool,
                            db::invoices::NewInvoice {
                                id: &new_id,
                                client_id: &t.client_id,
                                number: &number,
                                currency: &t.currency,
                                terms: &t.terms,
                                issued_date: &today,
                                due_date: &t.due_date,
                                project_type: &t.project_type,
                                project_location: &t.project_location,
                                billed_to_name: &t.billed_to_name,
                                billed_to_role: &t.billed_to_role,
                                billed_to_addr1: &t.billed_to_addr1,
                                billed_to_addr2: &t.billed_to_addr2,
                                billed_to_pin: &t.billed_to_pin,
                                billed_to_email: &t.billed_to_email,
                                billed_to_phone: &t.billed_to_phone,
                                items_json: &items_json,
                                notes_json: &notes_json,
                                editor_note: &t.editor_note,
                                kra_number: None, // must be filled before sending
                                recurring: false, // instances are not themselves recurring
                                recur_interval: None,
                                next_recur_date: None,
                            },
                        )
                        .await?;
                        db::invoices::update_next_recur_date(&pool, &t.id, &next_date).await?;
                        Ok(())
                    }
                    .await;

                    match tx_result {
                        Ok(()) => tracing::info!(
                            "recurring invoice: created draft {new_id} from template {}, next={next_date}",
                            t.id
                        ),
                        Err(e) => tracing::warn!(
                            "recurring invoice: failed for template {}: {e}",
                            t.id
                        ),
                    }
                }
            }
            Err(e) => tracing::warn!("recurring invoices query failed: {e}"),
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
