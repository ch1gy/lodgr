use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::ClientExport};

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    client_id: &str,
    file_path: &str,
) -> AppResult<ClientExport> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO client_exports (id, client_id, file_path, created_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(client_id)
    .bind(file_path)
    .bind(&created_at)
    .execute(pool)
    .await?;

    Ok(ClientExport {
        id: id.to_owned(),
        client_id: client_id.to_owned(),
        file_path: file_path.to_owned(),
        created_at,
    })
}

pub async fn exists_for_client(pool: &SqlitePool, client_id: &str) -> AppResult<bool> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM client_exports WHERE client_id = ?",
    )
    .bind(client_id)
    .fetch_one(pool)
    .await?;
    Ok(count.0 > 0)
}

pub async fn find_latest_for_client(
    pool: &SqlitePool,
    client_id: &str,
) -> AppResult<Option<ClientExport>> {
    Ok(sqlx::query_as::<_, ClientExport>(
        "SELECT id, client_id, file_path, created_at
         FROM client_exports WHERE client_id = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(client_id)
    .fetch_optional(pool)
    .await?)
}
