use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Config,
    crypto::{self, EncryptionKey},
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

// ── Encryption helpers ────────────────────────────────────────────────────────

/// Returns a User with all PII fields decrypted to plaintext.
/// Callers must always decrypt before passing users to DTOs or business logic.
pub fn decrypt_user(user: User, key: &EncryptionKey) -> AppResult<User> {
    let email = if user.email.is_empty() {
        String::new()
    } else {
        crypto::decrypt(key, &user.email_nonce, &user.email)?
    };

    let address_line1 = decrypt_opt(key, user.address_line1.as_deref(), user.address_line1_nonce.as_deref())?;
    let address_line2 = decrypt_opt(key, user.address_line2.as_deref(), user.address_line2_nonce.as_deref())?;
    let pin_number = decrypt_opt(key, user.pin_number.as_deref(), user.pin_number_nonce.as_deref())?;
    let contact_person = decrypt_opt(key, user.contact_person.as_deref(), user.contact_person_nonce.as_deref())?;
    let phone = decrypt_opt(key, user.phone.as_deref(), user.phone_nonce.as_deref())?;

    Ok(User {
        email,
        address_line1,
        address_line2,
        pin_number,
        contact_person,
        phone,
        ..user
    })
}

fn decrypt_opt(key: &EncryptionKey, ct: Option<&str>, nonce: Option<&str>) -> AppResult<Option<String>> {
    match (ct, nonce) {
        (Some(c), Some(n)) if !c.is_empty() => Ok(Some(crypto::decrypt(key, n, c)?)),
        _ => Ok(None),
    }
}

/// Encrypts an optional plaintext field. Returns `(ciphertext, nonce)` or `(None, None)`.
fn encrypt_opt(key: &EncryptionKey, value: Option<&str>) -> AppResult<(Option<String>, Option<String>)> {
    match value {
        Some(v) if !v.is_empty() => {
            let (nonce, ct) = crypto::encrypt(key, v)?;
            Ok((Some(ct), Some(nonce)))
        }
        _ => Ok((None, None)),
    }
}

/// If `new_val` is `Some`, re-encrypt it; otherwise pass the existing
/// ciphertext/nonce through unchanged. Used by `update_client_profile` to
/// avoid re-encrypting fields the caller didn't touch.
fn keep_or_reencrypt(
    key: &EncryptionKey,
    new_val: &Option<String>,
    existing_ct: &Option<String>,
    existing_nonce: &Option<String>,
) -> AppResult<(Option<String>, Option<String>)> {
    if new_val.is_some() {
        encrypt_opt(key, new_val.as_deref())
    } else {
        Ok((existing_ct.clone(), existing_nonce.clone()))
    }
}

// ── Public structs ────────────────────────────────────────────────────────────

pub struct NewClientProfile {
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub pin_number: Option<String>,
    pub contact_person: Option<String>,
    pub phone: Option<String>,
}

pub async fn create_client(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    email_hash_salt: &str,
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

    let (email_nonce, email_ct) = crypto::encrypt(enc_key, &email)?;
    let email_hash = crypto::hash_email(&email, email_hash_salt)?;

    db::users::create(pool, &id, &name, &email_ct, &email_nonce, &email_hash, &hash, "client")
        .await
        .map_err(|e| match e {
            AppError::Internal(ref msg) if msg.contains("UNIQUE") => {
                AppError::Conflict(format!("email '{email}' is already registered"))
            }
            other => other,
        })?;

    if let Some(p) = profile {
        let has_any = p.address_line1.is_some()
            || p.address_line2.is_some()
            || p.pin_number.is_some()
            || p.contact_person.is_some()
            || p.phone.is_some();
        if has_any {
            let (addr1_ct, addr1_nonce) = encrypt_opt(enc_key, p.address_line1.as_deref())?;
            let (addr2_ct, addr2_nonce) = encrypt_opt(enc_key, p.address_line2.as_deref())?;
            let (pin_ct, pin_nonce) = encrypt_opt(enc_key, p.pin_number.as_deref())?;
            let (cp_ct, cp_nonce) = encrypt_opt(enc_key, p.contact_person.as_deref())?;
            let (phone_ct, phone_nonce) = encrypt_opt(enc_key, p.phone.as_deref())?;

            db::users::update_profile(
                pool,
                &id,
                db::users::UpdateProfile {
                    name: &name,
                    email: &email_ct,
                    email_nonce: &email_nonce,
                    email_hash: &email_hash,
                    address_line1: addr1_ct.as_deref(),
                    address_line1_nonce: addr1_nonce.as_deref(),
                    address_line2: addr2_ct.as_deref(),
                    address_line2_nonce: addr2_nonce.as_deref(),
                    pin_number: pin_ct.as_deref(),
                    pin_number_nonce: pin_nonce.as_deref(),
                    contact_person: cp_ct.as_deref(),
                    contact_person_nonce: cp_nonce.as_deref(),
                    phone: phone_ct.as_deref(),
                    phone_nonce: phone_nonce.as_deref(),
                },
            )
            .await?;
        }
    }

    let raw = db::users::find_by_id(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("user vanished after insert".into()))?;
    decrypt_user(raw, enc_key)
}

pub async fn list_clients(pool: &SqlitePool, enc_key: &EncryptionKey) -> AppResult<Vec<User>> {
    let raw_users = db::users::find_all_clients(pool).await?;
    raw_users.into_iter().map(|u| decrypt_user(u, enc_key)).collect()
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
    let raw = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if raw.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    let user = decrypt_user(raw, enc_key)?;
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
    let tickets = db::tickets::list_all_for_client(pool, client_id).await?;

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
    pub phone: Option<String>,
}

pub async fn update_client_profile(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    email_hash_salt: &str,
    client_id: &str,
    input: UpdateClientProfileInput,
) -> AppResult<User> {
    let raw = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if raw.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    let name = match input.name {
        Some(ref n) => {
            if n.is_empty() || n.len() > 100 {
                return Err(AppError::BadRequest("name must be 1–100 characters".into()));
            }
            n.clone()
        }
        None => raw.name.clone(),
    };

    // For email: if new value provided, encrypt + hash; otherwise reuse existing ciphertext.
    let (email_ct, email_nonce, email_hash) = if let Some(ref new_email) = input.email {
        validate_email(new_email)?;
        let (nonce, ct) = crypto::encrypt(enc_key, new_email)?;
        let hash = crypto::hash_email(new_email, email_hash_salt)?;
        (ct, nonce, hash)
    } else {
        (raw.email.clone(), raw.email_nonce.clone(), raw.email_hash.clone())
    };

    // For each optional field: if new value provided, re-encrypt it; else keep existing ciphertext.
    let (addr1_ct, addr1_nonce)  = keep_or_reencrypt(enc_key, &input.address_line1, &raw.address_line1, &raw.address_line1_nonce)?;
    let (addr2_ct, addr2_nonce)  = keep_or_reencrypt(enc_key, &input.address_line2, &raw.address_line2, &raw.address_line2_nonce)?;
    let (pin_ct, pin_nonce)      = keep_or_reencrypt(enc_key, &input.pin_number, &raw.pin_number, &raw.pin_number_nonce)?;
    let (cp_ct, cp_nonce)        = keep_or_reencrypt(enc_key, &input.contact_person, &raw.contact_person, &raw.contact_person_nonce)?;
    let (phone_ct, phone_nonce)  = keep_or_reencrypt(enc_key, &input.phone, &raw.phone, &raw.phone_nonce)?;

    db::users::update_profile(
        pool,
        client_id,
        db::users::UpdateProfile {
            name: &name,
            email: &email_ct,
            email_nonce: &email_nonce,
            email_hash: &email_hash,
            address_line1: addr1_ct.as_deref(),
            address_line1_nonce: addr1_nonce.as_deref(),
            address_line2: addr2_ct.as_deref(),
            address_line2_nonce: addr2_nonce.as_deref(),
            pin_number: pin_ct.as_deref(),
            pin_number_nonce: pin_nonce.as_deref(),
            contact_person: cp_ct.as_deref(),
            contact_person_nonce: cp_nonce.as_deref(),
            phone: phone_ct.as_deref(),
            phone_nonce: phone_nonce.as_deref(),
        },
    )
    .await
    .map_err(|e| match e {
        AppError::Internal(ref msg) if msg.contains("UNIQUE") => {
            let email_plain = input.email.as_deref().unwrap_or("");
            AppError::Conflict(format!("email '{email_plain}' is already registered"))
        }
        other => other,
    })?;

    let updated_raw = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or_else(|| AppError::Internal("user vanished after update".into()))?;
    decrypt_user(updated_raw, enc_key)
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
