use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::User};

const USER_COLS: &str =
    "id, name, email, password_hash, role, created_at, deleted_at,
     failed_attempts, locked_until";

pub async fn find_by_email(pool: &SqlitePool, email: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        &format!("SELECT {USER_COLS} FROM users WHERE email = ?"),
    )
    .bind(email)
    .fetch_optional(pool)
    .await?)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        &format!("SELECT {USER_COLS} FROM users WHERE id = ?"),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn find_all_clients(pool: &SqlitePool) -> AppResult<Vec<User>> {
    Ok(sqlx::query_as::<_, User>(
        &format!("SELECT {USER_COLS} FROM users WHERE role = 'client' ORDER BY created_at DESC"),
    )
    .fetch_all(pool)
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

/// Increment the failed login counter and set the appropriate lockout window.
pub async fn increment_failed_attempts(
    pool: &SqlitePool,
    user_id: &str,
    new_attempts: i64,
    locked_until: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE users SET failed_attempts = ?, locked_until = ? WHERE id = ?",
    )
    .bind(new_attempts)
    .bind(locked_until)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Reset lockout state after a successful login.
pub async fn reset_lockout(pool: &SqlitePool, user_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE id = ?",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE users SET deleted_at = ? WHERE id = ?")
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn restore(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE users SET deleted_at = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn hard_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Returns users whose `deleted_at` is older than the given cutoff (RFC-3339).
pub async fn find_expired_soft_deleted(
    pool: &SqlitePool,
    cutoff: &str,
) -> AppResult<Vec<User>> {
    Ok(sqlx::query_as::<_, User>(
        &format!("SELECT {USER_COLS} FROM users WHERE deleted_at IS NOT NULL AND deleted_at < ?"),
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await?)
}
