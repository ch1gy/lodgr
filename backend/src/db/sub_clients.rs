use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::SubClient};

pub async fn list_for_client(pool: &SqlitePool, client_id: &str) -> AppResult<Vec<SubClient>> {
    Ok(
        sqlx::query_as(
            "SELECT id, client_id, name, created_at
             FROM sub_clients WHERE client_id = ? ORDER BY name ASC",
        )
        .bind(client_id)
        .fetch_all(pool)
        .await?,
    )
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<SubClient>> {
    Ok(
        sqlx::query_as(
            "SELECT id, client_id, name, created_at FROM sub_clients WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?,
    )
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    client_id: &str,
    name: &str,
) -> AppResult<SubClient> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sub_clients (id, client_id, name, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(client_id)
    .bind(name)
    .bind(&created_at)
    .execute(pool)
    .await?;
    Ok(SubClient {
        id: id.to_owned(),
        client_id: client_id.to_owned(),
        name: name.to_owned(),
        created_at,
    })
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sub_clients WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
