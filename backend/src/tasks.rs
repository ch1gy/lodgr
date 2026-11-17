use std::path::Path;

use chrono::{Duration, Months, NaiveDate, Utc};
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

pub fn advance_recur_date(date_str: &str, interval: &str) -> Option<String> {
    let d = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
    let months = match interval {
        "quarterly" => 3,
        "yearly" => 12,
        _ => 1, // monthly is the default
    };
    let next = d.checked_add_months(Months::new(months))?;
    Some(next.format("%Y-%m-%d").to_string())
}

/// Due date for a recurring invoice instance issued on `today`: the template's
/// own issued→due gap, so "Net 30", "Due on receipt" and custom gaps all carry
/// over. Falls back to 14 days when either template date fails to parse, the
/// gap is negative, or the result would be out of range.
pub fn recurring_due_date(
    today: NaiveDate,
    template_issued: &str,
    template_due: &str,
) -> NaiveDate {
    NaiveDate::parse_from_str(template_due, "%Y-%m-%d")
        .ok()
        .zip(NaiveDate::parse_from_str(template_issued, "%Y-%m-%d").ok())
        .map(|(due, issued)| due.signed_duration_since(issued))
        .filter(|gap| *gap >= Duration::zero())
        .and_then(|gap| today.checked_add_signed(gap))
        .unwrap_or_else(|| today + Duration::days(14))
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

                    // Belt-and-braces: list_due_for_recurrence already excludes NULL-client
                    // templates, but guard here too so a direct DB edit can't cause a panic.
                    let Some(ref client_id) = t.client_id else {
                        tracing::warn!(
                            "recurring invoice: template {} has no client_id — skipping",
                            t.id
                        );
                        continue;
                    };

                    let new_id = Uuid::new_v4().to_string();
                    let today_nd = Utc::now().date_naive();
                    let today = today_nd.format("%Y-%m-%d").to_string();
                    let due_date = recurring_due_date(today_nd, &t.issued_date, &t.due_date)
                        .format("%Y-%m-%d")
                        .to_string();

                    let items_json = t.items.clone();
                    let notes_json = t.notes.clone();

                    // Run next_seq + create + date advance in a single transaction so a
                    // mid-flight crash cannot produce duplicate numbers or stuck templates.
                    let tx_result: crate::error::AppResult<()> = async {
                        let mut tx = crate::db::begin_write(&pool).await?;
                        let seq = db::invoices::next_seq(&mut *tx).await?;
                        let number = format!("INV-{seq:04}");
                        db::invoices::create(
                            &mut *tx,
                            db::invoices::NewInvoice {
                                id: &new_id,
                                client_id,
                                number: &number,
                                currency: &t.currency,
                                terms: &t.terms,
                                issued_date: &today,
                                due_date: &due_date,
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
                                tax_rate: t.tax_rate,
                                recurring: false, // instances are not themselves recurring
                                recur_interval: None,
                                next_recur_date: None,
                                sub_client_id: t.sub_client_id.as_deref(),
                            },
                        )
                        .await?;
                        db::invoices::update_next_recur_date(&mut *tx, &t.id, &next_date).await?;
                        tx.commit().await?;
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

/// Housekeeping jobs run this long after the server starts, then on their
/// interval. Waiting a little lets startup finish first, and running at all
/// means a server that is only up for an hour or two still cleans up.
pub const STARTUP_DELAY: tokio::time::Duration = tokio::time::Duration::from_secs(60);

/// Log files older than this are deleted (the privacy notice promises 30 days).
pub const LOG_RETENTION: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 3600);

/// Runs about a minute after start, then every 24 h: drops expired sessions and
/// revocation rows, folds the write-ahead log into the database file, and
/// deletes logs past their retention.
pub async fn daily_maintenance(pool: SqlitePool) {
    tokio::time::sleep(STARTUP_DELAY).await;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86_400));
    loop {
        interval.tick().await;
        if let Ok(n) = db::sessions::delete_expired_and_revoked(&pool).await {
            tracing::info!("session cleanup: removed {n} rows");
        }
        if let Ok(n) = db::jwt_revocations::delete_expired(&pool).await {
            if n > 0 {
                tracing::info!("jti cleanup: removed {n} expired rows");
            }
        }
        if let Err(e) = db::checkpoint_wal(&pool).await {
            tracing::warn!("WAL checkpoint failed: {e}");
        }
        match clean_old_logs(Path::new("logs"), LOG_RETENTION).await {
            Ok(n) if n > 0 => tracing::info!("log cleanup: removed {n} old log files"),
            Err(e) => tracing::warn!("log cleanup failed: {e}"),
            _ => {}
        }
    }
}

/// Runs about a minute after start, then hourly: removes stale export files.
pub async fn export_cleanup() {
    tokio::time::sleep(STARTUP_DELAY).await;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;
        match clean_old_exports().await {
            Ok(n) if n > 0 => tracing::info!("export cleanup: removed {n} stale export files"),
            Err(e) => tracing::warn!("export cleanup failed: {e}"),
            _ => {}
        }
    }
}

/// Deletes `lodgr*.log*` files in `dir` last modified more than `max_age` ago.
/// Returns how many were removed. A missing directory is not an error.
pub async fn clean_old_logs(dir: &Path, max_age: std::time::Duration) -> std::io::Result<u32> {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(std::time::UNIX_EPOCH);

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };

    let mut removed = 0u32;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("lodgr") || !name.contains(".log") {
            continue;
        }
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if modified < cutoff && tokio::fs::remove_file(entry.path()).await.is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Runs about a minute after start, then every 24 h. See `purge_expired_users`.
pub async fn hard_delete_expired_users(pool: SqlitePool, enc_key: EncryptionKey, auto_purge: bool) {
    tokio::time::sleep(STARTUP_DELAY).await;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86_400));
    loop {
        interval.tick().await;
        purge_expired_users(&pool, &enc_key, auto_purge).await;
    }
}

/// One pass over clients archived more than 30 days ago.
///
/// Permanently deleting a client (tickets, messages, notes, sessions, drafts)
/// is irreversible, so it only happens when `auto_purge` is true
/// (`AUTO_PURGE_ARCHIVED_CLIENTS=true`). Otherwise this only reports, by id,
/// how many clients are past the window, and the desk deletes them by hand.
/// When enabled it mirrors the manual path: a pre-deletion export is ensured
/// (generated if none exists) before the cascade delete.
/// Returns how many clients were deleted.
pub async fn purge_expired_users(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    auto_purge: bool,
) -> usize {
    let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
    let users = match db::users::find_expired_soft_deleted(pool, &cutoff).await {
        Ok(users) => users,
        Err(e) => {
            tracing::warn!("expired user query failed: {e}");
            return 0;
        }
    };
    if users.is_empty() {
        return 0;
    }

    if !auto_purge {
        let ids: Vec<&str> = users.iter().map(|u| u.id.as_str()).collect();
        tracing::warn!(
            count = users.len(),
            user_ids = ?ids,
            "archived clients are past the 30-day recovery window and were NOT deleted; set AUTO_PURGE_ARCHIVED_CLIENTS=true to purge them automatically, or delete them from the Clients page"
        );
        return 0;
    }

    let mut purged = 0;
    for u in users {
        // Ensure a pre-deletion export record exists. If not, generate
        // one now — mirrors the guard in services::admin::hard_delete_client.
        let has_export = match db::exports::exists_for_client(pool, &u.id).await {
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
            match export_client(pool, enc_key, &u.id).await {
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
                .fetch_all(pool)
                .await
                .unwrap_or_default();

        match cascade_hard_delete_user(pool, &u.id).await {
            Ok(()) => {
                tracing::info!(user_id = %u.id, "auto hard-delete: user deleted");
                purged += 1;
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
    purged
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
    let mut tx = crate::db::begin_write(pool).await?;
    crate::services::admin::cascade_delete_user_data(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(())
}
