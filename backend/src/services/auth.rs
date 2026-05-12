use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use jsonwebtoken::{encode, Header};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::{
    config::Config,
    db,
    error::{AppError, AppResult},
    models::Claims,
};

pub struct LoginOutput {
    pub access_token: String,
    /// Raw token value — caller sets as the httpOnly cookie.
    pub refresh_token: String,
    pub refresh_ttl_secs: i64,
}

/// Pre-computed argon2id hash used to normalise login response time regardless
/// of whether the queried email exists. Computed once on first use.
static DUMMY_HASH: OnceLock<String> = OnceLock::new();

/// Call this at startup to pre-warm the dummy hash so the first real login
/// request doesn't pay the argon2 cost for the first time.
pub fn dummy_hash_warmup() -> &'static str {
    dummy_hash()
}

fn dummy_hash() -> &'static str {
    DUMMY_HASH.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(b"__dummy_that_never_matches_anything_real__", &salt)
            .expect("argon2 dummy hash failed")
            .to_string()
    })
}

pub async fn login(
    pool: &SqlitePool,
    config: &Config,
    email: &str,
    password: &str,
) -> AppResult<LoginOutput> {
    let user_opt = db::users::find_by_email(pool, email).await?;

    // Always run argon2 to make response time identical whether the user exists
    // or not, preventing timing-based email enumeration.
    // Clone the hash into an owned String so we can consume user_opt in the match below.
    let stored_hash: String = user_opt
        .as_ref()
        .map(|u| u.password_hash.clone())
        .unwrap_or_else(|| dummy_hash().to_owned());

    let password_ok = verify_password(password, &stored_hash).unwrap_or(false);

    let user = match (user_opt, password_ok) {
        (Some(u), true) => u,
        _ => return Err(AppError::Unauthorized),
    };

    // Transparent bcrypt → argon2id migration on next successful login.
    if user.password_hash.starts_with("$2") {
        if let Ok(new_hash) = hash_password(password) {
            let _ = db::users::update_password_hash(pool, &user.id, &new_hash).await;
        }
    }

    issue_tokens(pool, config, &user.id, &user.role).await
}

pub async fn refresh(
    pool: &SqlitePool,
    config: &Config,
    raw_token: &str,
) -> AppResult<LoginOutput> {
    let token_hash = hash_token(raw_token);

    let session = db::sessions::find_by_token_hash(pool, &token_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    // Replaying a rotated token means someone has a stale copy — treat as theft.
    if session.revoked_at.is_some() {
        db::sessions::delete_all_for_user(pool, &session.user_id).await?;
        tracing::warn!(
            user_id = %session.user_id,
            "refresh token reuse detected — all sessions revoked"
        );
        return Err(AppError::Unauthorized);
    }

    let expires_at = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .map_err(|_| AppError::Internal("invalid session expires_at".into()))?
        .with_timezone(&Utc);

    if expires_at < Utc::now() {
        db::sessions::delete(pool, &session.id).await?;
        return Err(AppError::Unauthorized);
    }

    let user = db::users::find_by_id(pool, &session.user_id)
        .await?
        .ok_or_else(|| AppError::Internal("session references missing user".into()))?;

    let (new_raw, new_hash) = generate_refresh_token();
    let new_expires_at = (Utc::now()
        + chrono::Duration::seconds(config.refresh_token_ttl_secs))
    .to_rfc3339();

    db::sessions::revoke(pool, &session.id, Some(&new_hash)).await?;

    let new_session_id = Uuid::new_v4().to_string();
    db::sessions::create_capped(
        pool,
        db::sessions::NewSession {
            id: &new_session_id,
            user_id: &user.id,
            token_hash: &new_hash,
            expires_at: &new_expires_at,
        },
        10,
    )
    .await?;

    let access_token = generate_access_token(config, &user.id, &user.role)?;

    Ok(LoginOutput {
        access_token,
        refresh_token: new_raw,
        refresh_ttl_secs: config.refresh_token_ttl_secs,
    })
}

pub async fn logout(pool: &SqlitePool, raw_token: &str) -> AppResult<()> {
    let token_hash = hash_token(raw_token);
    if let Some(session) = db::sessions::find_by_token_hash(pool, &token_hash).await? {
        db::sessions::delete(pool, &session.id).await?;
    }
    Ok(())
}

pub async fn check_default_password_warning(pool: &SqlitePool) {
    if let Ok(Some(user)) = db::users::find_by_email(pool, "desk@local").await {
        if verify_password("changeme", &user.password_hash).unwrap_or(false) {
            tracing::warn!(
                "SECURITY WARNING: desk@local is still using the default password 'changeme'. \
                 Change it immediately before exposing this service."
            );
            eprintln!(
                "\n╔══════════════════════════════════════════════════════════╗\
                 \n║  WARNING: desk@local still uses the default password     ║\
                 \n║  Change it immediately in any non-development deployment ║\
                 \n╚══════════════════════════════════════════════════════════╝\n"
            );
        }
    }
}

// ── internals ────────────────────────────────────────────────────────────────

async fn issue_tokens(
    pool: &SqlitePool,
    config: &Config,
    user_id: &str,
    role: &str,
) -> AppResult<LoginOutput> {
    let (raw_token, token_hash) = generate_refresh_token();
    let expires_at = (Utc::now() + chrono::Duration::seconds(config.refresh_token_ttl_secs))
        .to_rfc3339();

    let session_id = Uuid::new_v4().to_string();
    db::sessions::create_capped(
        pool,
        db::sessions::NewSession {
            id: &session_id,
            user_id,
            token_hash: &token_hash,
            expires_at: &expires_at,
        },
        10,
    )
    .await?;

    let access_token = generate_access_token(config, user_id, role)?;

    Ok(LoginOutput {
        access_token,
        refresh_token: raw_token,
        refresh_ttl_secs: config.refresh_token_ttl_secs,
    })
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("argon2 hash failed: {e}")))
}

/// Supports legacy bcrypt hashes (prefix $2a/$2b/$2y) for transparent migration.
pub fn verify_password(password: &str, stored_hash: &str) -> AppResult<bool> {
    if stored_hash.starts_with("$2") {
        bcrypt::verify(password, stored_hash)
            .map_err(|e| AppError::Internal(format!("bcrypt verify: {e}")))
    } else {
        let parsed = PasswordHash::new(stored_hash)
            .map_err(|e| AppError::Internal(format!("parse hash: {e}")))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }
}

fn generate_access_token(config: &Config, user_id: &str, role: &str) -> AppResult<String> {
    let exp = (Utc::now() + chrono::Duration::seconds(config.access_token_ttl_secs)).timestamp();
    let claims = Claims {
        sub: user_id.to_owned(),
        role: role.to_owned(),
        exp,
    };
    encode(&Header::default(), &claims, &config.encoding_key())
        .map_err(|e| AppError::Internal(format!("jwt encode: {e}")))
}

fn generate_refresh_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let raw: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let hash = hash_token(&raw);
    (raw, hash)
}

pub fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
