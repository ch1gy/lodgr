use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db;

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
                            tracing::warn!("recurring: failed to create from template {}: {e}", t.id);
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("recurring tickets query failed: {e}"),
        }
    }
}

/// Runs every 24 h. Hard-deletes users soft-deleted more than 30 days ago.
pub async fn hard_delete_expired_users(pool: SqlitePool) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86_400));
    interval.tick().await; // skip first tick — already ran at startup
    loop {
        interval.tick().await;
        let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        match db::users::find_expired_soft_deleted(&pool, &cutoff).await {
            Ok(users) => {
                for u in users {
                    let _ = db::sessions::delete_all_for_user(&pool, &u.id).await;
                    match db::users::hard_delete(&pool, &u.id).await {
                        Ok(_) => tracing::info!("hard deleted expired user {}", u.id),
                        Err(e) => tracing::warn!("hard delete of {} failed: {e}", u.id),
                    }
                }
            }
            Err(e) => tracing::warn!("expired user query failed: {e}"),
        }
    }
}
