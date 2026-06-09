use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::User};

const USER_COLS: &str = "id, name, email, email_nonce, email_hash, password_hash, role, \
    created_at, deleted_at, failed_attempts, locked_until, \
    address_line1, address_line1_nonce, address_line2, address_line2_nonce, \
    pin_number, pin_number_nonce, contact_person, contact_person_nonce, \
    phone, phone_nonce";

pub async fn find_by_email_hash(pool: &SqlitePool, hash: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(&format!(
        "SELECT {USER_COLS} FROM users WHERE email_hash = ?"
    ))
    .bind(hash)
    .fetch_optional(pool)
    .await?)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<User>> {
    Ok(
        sqlx::query_as::<_, User>(&format!("SELECT {USER_COLS} FROM users WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn find_all_clients(pool: &SqlitePool) -> AppResult<Vec<User>> {
    Ok(sqlx::query_as::<_, User>(&format!(
        "SELECT {USER_COLS} FROM users WHERE role = 'client' ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await?)
}

pub struct NewUser<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub email: &'a str,
    pub email_nonce: &'a str,
    pub email_hash: &'a str,
    pub password_hash: &'a str,
    pub role: &'a str,
}

pub async fn create(pool: &SqlitePool, u: NewUser<'_>) -> AppResult<()> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, name, email, email_nonce, email_hash, password_hash, role, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(u.id)
    .bind(u.name)
    .bind(u.email)
    .bind(u.email_nonce)
    .bind(u.email_hash)
    .bind(u.password_hash)
    .bind(u.role)
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
    sqlx::query("UPDATE users SET failed_attempts = ?, locked_until = ? WHERE id = ?")
        .bind(new_attempts)
        .bind(locked_until)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Reset lockout state after a successful login.
pub async fn reset_lockout(pool: &SqlitePool, user_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn find_desk_user(pool: &SqlitePool) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(&format!(
        "SELECT {USER_COLS} FROM users WHERE role = 'desk' LIMIT 1"
    ))
    .fetch_optional(pool)
    .await?)
}

pub struct UpdateProfile<'a> {
    pub name: &'a str,
    pub email: &'a str,
    pub email_nonce: &'a str,
    pub email_hash: &'a str,
    pub address_line1: Option<&'a str>,
    pub address_line1_nonce: Option<&'a str>,
    pub address_line2: Option<&'a str>,
    pub address_line2_nonce: Option<&'a str>,
    pub pin_number: Option<&'a str>,
    pub pin_number_nonce: Option<&'a str>,
    pub contact_person: Option<&'a str>,
    pub contact_person_nonce: Option<&'a str>,
    pub phone: Option<&'a str>,
    pub phone_nonce: Option<&'a str>,
}

pub async fn update_profile(
    pool: &SqlitePool,
    user_id: &str,
    p: UpdateProfile<'_>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE users SET
            name = ?,
            email = ?, email_nonce = ?, email_hash = ?,
            address_line1 = ?, address_line1_nonce = ?,
            address_line2 = ?, address_line2_nonce = ?,
            pin_number = ?, pin_number_nonce = ?,
            contact_person = ?, contact_person_nonce = ?,
            phone = ?, phone_nonce = ?
         WHERE id = ?",
    )
    .bind(p.name)
    .bind(p.email)
    .bind(p.email_nonce)
    .bind(p.email_hash)
    .bind(p.address_line1)
    .bind(p.address_line1_nonce)
    .bind(p.address_line2)
    .bind(p.address_line2_nonce)
    .bind(p.pin_number)
    .bind(p.pin_number_nonce)
    .bind(p.contact_person)
    .bind(p.contact_person_nonce)
    .bind(p.phone)
    .bind(p.phone_nonce)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns users whose `deleted_at` is older than the given cutoff (RFC-3339).
pub async fn find_expired_soft_deleted(pool: &SqlitePool, cutoff: &str) -> AppResult<Vec<User>> {
    Ok(sqlx::query_as::<_, User>(&format!(
        "SELECT {USER_COLS} FROM users WHERE deleted_at IS NOT NULL AND deleted_at < ?"
    ))
    .bind(cutoff)
    .fetch_all(pool)
    .await?)
}
