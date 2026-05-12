use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db;

pub async fn notify(pool: &SqlitePool, recipient_id: &str, ticket_id: &str, message: &str) {
    println!("[NOTIFY] → user={recipient_id} ticket={ticket_id} msg={message}");

    let id = Uuid::new_v4().to_string();
    let _ = db::notifications::insert(pool, &id, ticket_id, recipient_id, message).await;
}
