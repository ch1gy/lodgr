use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::User};

pub async fn find_by_email(pool: &SqlitePool, email: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, name, email, password_hash, role, created_at FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, name, email, password_hash, role, created_at FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    email: &str,
    password_hash: &str,
    role: &str,
) -> AppResult<()> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, role, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(email)
    .bind(password_hash)
    .bind(role)
    .bind(&created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_password_hash(
    pool: &SqlitePool,
    user_id: &str,
    new_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(new_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
