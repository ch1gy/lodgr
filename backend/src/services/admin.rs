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

pub struct NewClientProfile {
    pub address_line1:  Option<String>,
    pub address_line2:  Option<String>,
    pub pin_number:     Option<String>,
    pub contact_person: Option<String>,
}

pub async fn create_client(
    pool: &SqlitePool,
    name: String,
    email: String,
    password: String,
    profile: Option<NewClientProfile>,
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

    if let Some(p) = profile {
        let has_any = p.address_line1.is_some() || p.address_line2.is_some()
            || p.pin_number.is_some() || p.contact_person.is_some();
        if has_any {
            db::users::update_profile(
                pool, &id, &name, &email,
                p.address_line1.as_deref(),
                p.address_line2.as_deref(),
                p.pin_number.as_deref(),
                p.contact_person.as_deref(),
            )
            .await?;
        }
    }

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
    // Revoke any outstanding magic-link JTIs so those tokens are immediately
    // invalid even before their exp. Must run alongside the refresh-token wipe.
    db::jwt_revocations::revoke_for_user(pool, client_id).await?;
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
            "an export must be created before hard deletion (POST /admin/clients/:id/export)"
                .into(),
        ));
    }

    // Collect ticket IDs before deletion so we can clean up upload directories.
    let tickets = db::tickets::list_all_for_client(pool, client_id)
        .await
        .unwrap_or_default();

    // Final export — must succeed before any data is deleted.
    export_client(pool, enc_key, client_id).await?;

    // Single transaction: cascade all child rows then delete the user.
    let mut tx = pool.begin().await?;
    cascade_delete_user_data(&mut tx, client_id).await?;
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

pub struct UpdateClientProfileInput {
    pub name: Option<String>,
    pub email: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub pin_number: Option<String>,
    pub contact_person: Option<String>,
}

pub async fn update_client_profile(
    pool: &SqlitePool,
    client_id: &str,
    input: UpdateClientProfileInput,
) -> AppResult<User> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    let name = match input.name {
        Some(ref n) => {
            if n.is_empty() || n.len() > 100 {
                return Err(AppError::BadRequest("name must be 1–100 characters".into()));
            }
            n.clone()
        }
        None => user.name.clone(),
    };

    let email = match input.email {
        Some(ref e) => {
            validate_email(e)?;
            e.clone()
        }
        None => user.email.clone(),
    };

    let addr1 = input.address_line1.as_deref().or(user.address_line1.as_deref());
    let addr2 = input.address_line2.as_deref().or(user.address_line2.as_deref());
    let pin   = input.pin_number.as_deref().or(user.pin_number.as_deref());
    let cp    = input.contact_person.as_deref().or(user.contact_person.as_deref());

    db::users::update_profile(pool, client_id, &name, &email, addr1, addr2, pin, cp)
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

/// Run the full child-row cascade for a user inside an *existing* transaction.
///
/// Deletes in FK-safe order:
///   magic_links → thread_entries → internal_notes → tickets
///   → sessions → jwt_revocations → client_exports → users
///
/// Callers own the transaction and must call `tx.commit()` after this returns.
/// Called from both `hard_delete_client` (desk-initiated) and
/// `cascade_hard_delete_user` (background 30-day expiry task).
///
/// # TODO
/// `client_exports.client_id` has a FK to `users.id` that should be
/// `ON DELETE CASCADE` at the schema level, which would make this DELETE
/// unnecessary and remove an entire class of "forgot one call site" bug.
/// SQLite requires a full table-recreation migration to alter FKs — see
/// PLANNED.md for the tracked item.
pub(crate) async fn cascade_delete_user_data(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user_id: &str,
) -> AppResult<()> {
    sqlx::query("DELETE FROM magic_links WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "DELETE FROM thread_entries WHERE ticket_id IN
         (SELECT id FROM tickets WHERE client_id = ?)",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "DELETE FROM internal_notes WHERE ticket_id IN
         (SELECT id FROM tickets WHERE client_id = ?)",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM tickets WHERE client_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    // jwt_revocations and auth_events both FK to users — delete before the user row.
    sqlx::query("DELETE FROM jwt_revocations WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM auth_events WHERE user_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM client_exports WHERE client_id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

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
    if domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(AppError::BadRequest(
            "email domain is invalid (must contain at least one dot, no leading/trailing dots)"
                .into(),
        ));
    }
    Ok(())
}
