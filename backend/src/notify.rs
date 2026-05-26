use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db;

/// Insert a notification row and emit a structured log line.
/// The DB insert is non-fatal: a notification failure must not abort the
/// operation that triggered it (e.g. ticket creation, message send).
pub async fn notify(pool: &SqlitePool, recipient_id: &str, ticket_id: &str, message: &str) {
    tracing::info!(
        user_id = %recipient_id,
        ticket_id = %ticket_id,
        msg = %message,
        "notification dispatched"
    );
    let id = Uuid::new_v4().to_string();
    if let Err(e) = db::notifications::insert(pool, &id, ticket_id, recipient_id, message).await {
        tracing::warn!(
            %e,
            ticket_id = %ticket_id,
            recipient_id = %recipient_id,
            "notification DB insert failed (non-fatal)"
        );
    }
}
