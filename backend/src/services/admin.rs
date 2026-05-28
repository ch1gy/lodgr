use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Config,
    crypto::EncryptionKey,
    db,
    email::SmtpMailer,
    error::{AppError, AppResult},
    models::User,
    services::{
        auth::{hash_password, validate_password_strength},
        export::{export_client, ExportOutput},
        magic::{create_magic_link, MagicLinkOutput},
    },
};

pub async fn create_client(
    pool: &SqlitePool,
    name: String,
    email: String,
    password: String,
) -> AppResult<User> {
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::BadRequest("name must be 1–100 characters".into()));
    }
    validate_email(&email)?;
    validate_password_strength(&password)?;

    let id = Uuid::new_v4().to_string();
    let hash = hash_password(&password)?;

    db::users::create(pool, &id, &name, &email, &hash, "client")
        .await
        .map_err(|e| match e {
            AppError::Internal(ref msg) if msg.contains("UNIQUE") => {
                AppError::Conflict(format!("email '{email}' is already registered"))
            }
            other => other,
        })?;

    db::users::find_by_id(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("user vanished after insert".into()))
}

pub async fn list_clients(pool: &SqlitePool) -> AppResult<Vec<User>> {
    db::users::find_all_clients(pool).await
}

pub async fn delete_client_sessions(pool: &SqlitePool, client_id: &str) -> AppResult<u64> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }
    db::sessions::delete_all_for_user(pool, client_id).await
}

pub async fn soft_delete_client(pool: &SqlitePool, client_id: &str) -> AppResult<()> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }
    if user.deleted_at.is_some() {
        return Err(AppError::BadRequest("user is already deleted".into()));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE tickets SET deleted_at = ?
         WHERE client_id = ? AND deleted_at IS NULL",
    )
    .bind(&now)
    .bind(client_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE users SET deleted_at = ? WHERE id = ?")
        .bind(&now)
        .bind(client_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(client_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn restore_client(pool: &SqlitePool, client_id: &str) -> AppResult<()> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let deleted_at = user
        .deleted_at
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("user is not deleted".into()))?;

    let deleted_ts = chrono::DateTime::parse_from_rfc3339(deleted_at)
        .map_err(|_| AppError::Internal("invalid deleted_at".into()))?
        .with_timezone(&Utc);

    if Utc::now() - deleted_ts > chrono::Duration::days(30) {
        return Err(AppError::BadRequest(
            "recovery window (30 days) has expired; user must be permanently deleted".into(),
        ));
    }

    let mut tx = pool.begin().await?;

    // Only restore tickets that were deleted together with this user (same deleted_at
    // timestamp), not tickets that were individually deleted beforehand.
    sqlx::query(
        "UPDATE tickets SET deleted_at = NULL
         WHERE client_id = ? AND deleted_at = ?",
    )
    .bind(client_id)
    .bind(deleted_at)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE users SET deleted_at = NULL WHERE id = ?")
        .bind(client_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn hard_delete_client(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    client_id: &str,
    confirm: &str,
) -> AppResult<()> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    let expected = format!("permanently delete {}", user.email);
    if confirm != expected {
        return Err(AppError::BadRequest(format!(
            "confirmation string must be exactly: \"{expected}\""
        )));
    }

    if !db::exports::exists_for_client(pool, client_id).await? {
        return Err(AppError::BadRequest(
            "an export must be created before hard deletion (POST /admin/clients/:id/export)".into(),
        ));
    }

    // Collect ticket IDs before deletion so we can clean up upload directories.
    let tickets = db::tickets::list_all_for_client(pool, client_id).await.unwrap_or_default();

    // Final export — must succeed before any data is deleted.
    export_client(pool, enc_key, client_id).await?;

    // Single transaction: cascade all child rows then delete the user.
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM magic_links WHERE user_id = ?")
        .bind(client_id).execute(&mut *tx).await?;

    sqlx::query(
        "DELETE FROM notifications WHERE ticket_id IN
         (SELECT id FROM tickets WHERE client_id = ?)",
    )
    .bind(client_id).execute(&mut *tx).await?;

    sqlx::query(
        "DELETE FROM thread_entries WHERE ticket_id IN
         (SELECT id FROM tickets WHERE client_id = ?)",
    )
    .bind(client_id).execute(&mut *tx).await?;

    sqlx::query(
        "DELETE FROM internal_notes WHERE ticket_id IN
         (SELECT id FROM tickets WHERE client_id = ?)",
    )
    .bind(client_id).execute(&mut *tx).await?;

    sqlx::query("DELETE FROM tickets WHERE client_id = ?")
        .bind(client_id).execute(&mut *tx).await?;

    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(client_id).execute(&mut *tx).await?;

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(client_id).execute(&mut *tx).await?;

    tx.commit().await?;

    // Remove per-ticket upload directories after the DB rows are gone.
    for ticket in &tickets {
        let dir = format!("uploads/{}", ticket.id);
        if let Err(e) = tokio::fs::remove_dir_all(&dir).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    dir = %dir,
                    err = %e,
                    "failed to remove upload dir during client hard delete"
                );
            }
        }
    }

    Ok(())
}

pub async fn do_export(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    client_id: &str,
) -> AppResult<ExportOutput> {
    export_client(pool, enc_key, client_id).await
}

pub async fn update_client_profile(
    pool: &SqlitePool,
    client_id: &str,
    new_name: Option<String>,
    new_email: Option<String>,
) -> AppResult<User> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    // Nothing to update — return current state without a DB round-trip.
    if new_name.is_none() && new_email.is_none() {
        return Ok(user);
    }

    let name = match new_name {
        Some(ref n) => {
            if n.is_empty() || n.len() > 100 {
                return Err(AppError::BadRequest("name must be 1–100 characters".into()));
            }
            n.clone()
        }
        None => user.name.clone(),
    };

    let email = match new_email {
        Some(ref e) => {
            validate_email(e)?;
            e.clone()
        }
        None => user.email.clone(),
    };

    db::users::update_profile(pool, client_id, &name, &email)
        .await
        .map_err(|e| match e {
            AppError::Internal(ref msg) if msg.contains("UNIQUE") => {
                AppError::Conflict(format!("email '{email}' is already registered"))
            }
            other => other,
        })?;

    db::users::find_by_id(pool, client_id)
        .await?
        .ok_or_else(|| AppError::Internal("user vanished after update".into()))
}

pub async fn unlock_client(pool: &SqlitePool, client_id: &str) -> AppResult<()> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }
    db::users::reset_lockout(pool, client_id).await
}

pub async fn generate_magic_link(
    pool: &SqlitePool,
    config: &Config,
    mailer: Option<&SmtpMailer>,
    target_user_id: &str,
    scope: &str,
    ticket_id: Option<&str>,
) -> AppResult<MagicLinkOutput> {
    create_magic_link(pool, config, mailer, target_user_id, scope, ticket_id).await
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Basic RFC-5321 email validation without external crates.
/// Checks for a single '@', non-empty local part, and a valid domain.
fn validate_email(email: &str) -> AppResult<()> {
    if email != email.trim() {
        return Err(AppError::BadRequest(
            "email must not have leading or trailing whitespace".into(),
        ));
    }
    if email.is_empty() {
        return Err(AppError::BadRequest("email must not be empty".into()));
    }
    if email.len() > 254 {
        return Err(AppError::BadRequest(
            "email must be at most 254 characters".into(),
        ));
    }
    let at_count = email.chars().filter(|c| *c == '@').count();
    if at_count != 1 {
        return Err(AppError::BadRequest(
            "email must contain exactly one '@' character".into(),
        ));
    }
    let (local, domain) = email.split_once('@').unwrap();
    if local.is_empty() {
        return Err(AppError::BadRequest(
            "email local part (before '@') must not be empty".into(),
        ));
    }
    if domain.is_empty() || !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return Err(AppError::BadRequest(
            "email domain is invalid (must contain at least one dot, no leading/trailing dots)".into(),
        ));
    }
    Ok(())
}
