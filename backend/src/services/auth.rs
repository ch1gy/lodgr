use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use chrono::Utc;
use jsonwebtoken::{encode, Header};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{collections::HashSet, net::IpAddr, sync::OnceLock};
use uuid::Uuid;

use crate::{
    config::Config,
    crypto::{self, EncryptionKey},
    db,
    email::SmtpMailer,
    error::{AppError, AppResult},
    models::{Claims, User},
    services::magic,
};

/// Failed-attempt count at which an account is permanently locked.
/// Must stay in sync with the `_ =>` arm of `compute_locked_until`.
pub const PERMANENT_LOCKOUT_THRESHOLD: i64 = 9;

/// Maximum number of concurrent refresh-token sessions per user.
const MAX_SESSIONS_PER_USER: i64 = 10;

// ── 100 most-common passwords — rejected regardless of length ─────────────────
// Stored as a HashSet (O(1) lookup) initialised once via OnceLock.
static COMMON_PASSWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn common_passwords() -> &'static HashSet<&'static str> {
    COMMON_PASSWORDS.get_or_init(|| {
        [
            "password",
            "password1",
            "password123",
            "password12",
            "password!",
            "12345678",
            "123456789",
            "1234567890",
            "123456789a",
            "1234567891",
            "qwerty123",
            "qwertyuiop",
            "qwerty1234",
            "qwerty12",
            "qwerty!1",
            "iloveyou",
            "iloveyou1",
            "iloveyou12",
            "iloveyou!",
            "loveyou12",
            "admin123",
            "admin1234",
            "admin12345",
            "admin2024",
            "adminadmin",
            "letmein1",
            "letmein12",
            "letmein123",
            "letmein!1",
            "letme1n1",
            "monkey123",
            "monkey1234",
            "monkeys12",
            "dragon123",
            "dragon1234",
            "shadow123",
            "shadow1234",
            "shadows12",
            "master123",
            "master1234",
            "superman1",
            "superman12",
            "batman123",
            "batman1234",
            "batmanman",
            "trustno1",
            "trustno12",
            "sunshine1",
            "sunshine12",
            "sunshines",
            "princess1",
            "princess12",
            "princess!",
            "michael1",
            "michael12",
            "football1",
            "football12",
            "baseball1",
            "baseball12",
            "soccer123",
            "welcome1",
            "welcome12",
            "welcome!1",
            "hello1234",
            "hello12345",
            "pass1234",
            "pass12345",
            "passw0rd1",
            "p@ssword1",
            "p@ss1234",
            "changeme",
            "changeme1",
            "changeme!",
            "whatever1",
            "whatever!",
            "nothing12",
            "freedom12",
            "freedom1!",
            "starwars1",
            "starwars12",
            "starwar12",
            "1q2w3e4r",
            "q1w2e3r4",
            "1qaz2wsx",
            "zxcvbn12",
            "zaq1zaq1",
            "qazwsx12",
            "abc12345",
            "abc123456",
            "test1234",
            "testing1",
            "root1234",
            "toor1234",
            "computer1",
            "internet1",
            "windows10",
            "mustang1",
            "charlie1",
            "jessica1",
            "1234abcd",
            "abcd1234",
        ]
        .into_iter()
        .collect()
    })
}

#[derive(Debug)]
pub struct LoginOutput {
    pub access_token: String,
    /// Raw token value — caller sets as the httpOnly cookie.
    pub refresh_token: String,
    pub refresh_ttl_secs: i64,
}

/// Pre-computed argon2id hash used to normalise login response time regardless
/// of whether the queried email exists. Computed once on first use.
static DUMMY_HASH: OnceLock<Option<String>> = OnceLock::new();

/// Call this at startup to pre-warm the dummy hash. Logs a warning and continues
/// if Argon2 fails — timing normalization is degraded but the server still starts.
pub fn dummy_hash_warmup() {
    if dummy_hash().is_none() {
        tracing::warn!(
            "argon2 dummy hash init failed — \
             timing normalization degraded for unknown-email login attempts"
        );
    }
}

fn dummy_hash() -> Option<&'static str> {
    DUMMY_HASH
        .get_or_init(|| {
            let salt = SaltString::generate(&mut OsRng);
            let params = Params::new(65_536, 3, 4, None).ok()?;
            Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
                .hash_password(b"__dummy_that_never_matches_anything_real__", &salt)
                .ok()
                .map(|h| h.to_string())
        })
        .as_deref()
}

pub async fn login(
    pool: &SqlitePool,
    config: &Config,
    enc_key: &EncryptionKey,
    mailer: Option<&SmtpMailer>,
    email: &str,
    password: &str,
    peer_ip: IpAddr,
) -> AppResult<LoginOutput> {
    let email_hash = crypto::hash_email(email, &config.email_hash_salt)?;
    let user_opt = db::users::find_by_email_hash(pool, &email_hash).await?;

    // Check lockout before running argon2. A locked account's existence is
    // already known (it wouldn't be locked otherwise), so returning early is
    // safe and avoids burning CPU on accounts being hammered.
    if let Some(ref user) = user_opt {
        if let Some(ref locked_until_str) = user.locked_until {
            let locked_until = chrono::DateTime::parse_from_rfc3339(locked_until_str)
                .map_err(|_| AppError::Internal("invalid locked_until".into()))?
                .with_timezone(&Utc);

            if locked_until > Utc::now() {
                let retry_after_secs = if user.failed_attempts >= PERMANENT_LOCKOUT_THRESHOLD {
                    None // permanent
                } else {
                    Some((locked_until - Utc::now()).num_seconds().max(0) as u64)
                };
                tracing::warn!(
                    email = %email,
                    ip = %peer_ip,
                    failed_attempts = user.failed_attempts,
                    permanent = retry_after_secs.is_none(),
                    "login rejected — account locked"
                );
                return Err(AppError::Locked { retry_after_secs });
            }
        }
    }

    // Always run argon2 to make response time identical whether the user exists
    // or not, preventing timing-based email enumeration.
    let stored_hash: String = user_opt
        .as_ref()
        .map(|u| u.password_hash.clone())
        .unwrap_or_else(|| dummy_hash().unwrap_or("").to_owned());

    let password_ok = verify_password(password, &stored_hash).unwrap_or(false);

    match (user_opt, password_ok) {
        (Some(u), true) if u.deleted_at.is_none() => {
            db::users::reset_lockout(pool, &u.id).await?;
            let output = issue_tokens(pool, config, &u.id, &u.role).await?;
            tracing::info!(user_id = %u.id, role = %u.role, ip = %peer_ip, "successful login");
            if let Err(e) = db::auth_events::create(pool, &u.id, "login_ok").await {
                tracing::warn!(user_id = %u.id, %e, "failed to write login_ok auth event");
            }
            if let Some(m) = mailer {
                spawn_login_alert(m, enc_key, &u, peer_ip);
            }
            Ok(output)
        }
        (Some(u), true) => {
            // Correct password but account is soft-deleted — don't increment lockout.
            tracing::warn!(
                email = %email,
                ip = %peer_ip,
                user_id = %u.id,
                "login attempt for soft-deleted account"
            );
            Err(AppError::Unauthorized)
        }
        (Some(u), false) => {
            // Wrong password — increment counter and set lockout window.
            let new_attempts = u.failed_attempts + 1;
            let locked_until = compute_locked_until(new_attempts);
            db::users::increment_failed_attempts(
                pool,
                &u.id,
                new_attempts,
                locked_until.as_deref(),
            )
            .await?;
            tracing::warn!(
                email = %email,
                ip = %peer_ip,
                failed_attempts = new_attempts,
                locked = locked_until.is_some(),
                "failed login attempt"
            );

            // On first permanent lockout, take automated action.
            if new_attempts >= PERMANENT_LOCKOUT_THRESHOLD {
                if u.role == "client" {
                    maybe_auto_lockout_ticket(pool, &u.id, new_attempts).await;
                } else if u.role == "desk" {
                    if let Some(m) = mailer {
                        spawn_desk_recovery_link(
                            pool,
                            config,
                            m,
                            enc_key,
                            DeskUser {
                                id: &u.id,
                                email_nonce: &u.email_nonce,
                                email_ct: &u.email,
                                name: &u.name,
                            },
                        );
                    } else {
                        tracing::error!(
                            user_id = %u.id,
                            "SECURITY: desk account permanently locked and no SMTP configured — \
                             manual DB recovery required: \
                             UPDATE users SET failed_attempts=0, locked_until=NULL WHERE id='{}'",
                            u.id
                        );
                    }
                }
            }

            Err(AppError::Unauthorized)
        }
        (None, _) => {
            tracing::warn!(email = %email, ip = %peer_ip, "failed login — unknown email");
            Err(AppError::Unauthorized)
        }
    }
}

/// Auto-opens a security_log ticket when a client account is permanently locked.
/// Guards against duplicates by checking for a recent ticket first.
async fn maybe_auto_lockout_ticket(pool: &SqlitePool, user_id: &str, attempts: i64) {
    match db::tickets::has_recent_security_lockout_ticket(pool, user_id).await {
        Ok(false) => {
            let ticket_id = Uuid::new_v4().to_string();
            let desc = format!(
                "Client account {} was permanently locked after {} consecutive \
                 failed login attempts. Generate a magic link to restore access.",
                user_id, attempts
            );
            if let Err(e) = db::tickets::create(
                pool,
                db::tickets::NewTicket {
                    id: &ticket_id,
                    title: "Account locked — repeated failed login attempts",
                    description: &desc,
                    created_by: user_id,
                    client_id: user_id,
                    priority: "urgent",
                    category: None,
                    due_date: None,
                    estimated_completion: None,
                    ticket_type: "security_log",
                    recurring: false,
                    recurring_interval_days: None,
                    sub_client_id: None,
                },
            )
            .await
            {
                tracing::warn!(user_id = %user_id, %e, "failed to auto-create lockout ticket");
            } else {
                tracing::warn!(user_id = %user_id, ticket_id = %ticket_id, "auto-created security_log ticket on permanent lockout");
            }
        }
        Ok(true) => {}
        Err(e) => {
            tracing::warn!(user_id = %user_id, %e, "failed to check for existing lockout ticket")
        }
    }
}

struct DeskUser<'a> {
    id: &'a str,
    email_nonce: &'a str,
    email_ct: &'a str,
    name: &'a str,
}

/// Decrypts the desk user's email and spawns a recovery magic-link email.
/// Logs an error and skips the dispatch if decryption fails.
fn spawn_desk_recovery_link(
    pool: &SqlitePool,
    config: &Config,
    mailer: &SmtpMailer,
    enc_key: &EncryptionKey,
    desk: DeskUser<'_>,
) {
    match crypto::decrypt(enc_key, desk.email_nonce, desk.email_ct) {
        Ok(uemail) => {
            let pool2 = pool.clone();
            let config2 = config.clone();
            let m2 = mailer.clone();
            let uid = desk.id.to_owned();
            let uname = desk.name.to_owned();
            tokio::spawn(async move {
                magic::send_desk_recovery_link(&pool2, &config2, &m2, &uid, &uemail, &uname).await;
            });
        }
        Err(err) => tracing::error!(
            user_id = %desk.id,
            %err,
            "failed to decrypt desk email — skipping recovery link dispatch"
        ),
    }
}

/// Decrypts the user's email and spawns a best-effort login-notification
/// email. Logs an error and skips the dispatch if decryption fails.
fn spawn_login_alert(mailer: &SmtpMailer, enc_key: &EncryptionKey, user: &User, peer_ip: IpAddr) {
    match crypto::decrypt(enc_key, &user.email_nonce, &user.email) {
        Ok(uemail) => {
            let m2 = mailer.clone();
            let uname = user.name.clone();
            let role = user.role.clone();
            let now = Utc::now();
            let date = now.format("%d %b %Y").to_string();
            let time = now.format("%H:%M UTC").to_string();
            let ip = peer_ip.to_string();
            tokio::spawn(async move {
                m2.send_login_alert(&uemail, &uname, &role, &ip, &date, &time)
                    .await;
            });
        }
        Err(err) => tracing::error!(
            user_id = %user.id,
            %err,
            "failed to decrypt user email — skipping login alert dispatch"
        ),
    }
}

/// Compute the RFC-3339 timestamp the account should be locked until, based on
/// how many consecutive failures have accumulated. Returns `None` if no lockout
/// applies yet (fewer than 5 failures).
fn compute_locked_until(failed_attempts: i64) -> Option<String> {
    let now = Utc::now();
    let until = match failed_attempts {
        n if n < 5 => return None,
        5 => now + chrono::Duration::minutes(1),
        6 => now + chrono::Duration::minutes(5),
        7 => now + chrono::Duration::minutes(15),
        8 => now + chrono::Duration::hours(1),
        // 9+ → permanent: use a sentinel far-future date.
        _ => chrono::DateTime::parse_from_rfc3339("9999-01-01T00:00:00+00:00")
            .unwrap()
            .with_timezone(&Utc),
    };
    Some(until.to_rfc3339())
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

    if user.deleted_at.is_some() {
        db::sessions::delete_all_for_user(pool, &user.id).await?;
        return Err(AppError::Unauthorized);
    }

    let (new_raw, new_hash) = generate_refresh_token();
    let new_expires_at =
        (Utc::now() + chrono::Duration::seconds(config.refresh_token_ttl_secs)).to_rfc3339();

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
        MAX_SESSIONS_PER_USER,
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
        if let Err(e) = db::auth_events::create(pool, &session.user_id, "logout").await {
            tracing::warn!(user_id = %session.user_id, %e, "failed to write logout auth event");
        }
    }
    Ok(())
}

/// Change password for any fully-authenticated user (desk or client).
/// Verifies current password, validates and hashes the new one, revokes all
/// existing sessions and outstanding magic-link JTIs, and issues fresh tokens.
pub async fn change_password(
    pool: &SqlitePool,
    config: &Config,
    user_id: &str,
    current_password: &str,
    new_password: &str,
) -> AppResult<LoginOutput> {
    let user = db::users::find_by_id(pool, user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify_password(current_password, &user.password_hash)? {
        tracing::warn!(user_id = %user_id, "password change rejected — wrong current password");
        return Err(AppError::Unauthorized);
    }

    validate_password_strength(new_password)?;

    let new_hash = hash_password(new_password)?;
    db::users::update_password_hash(pool, user_id, &new_hash).await?;

    // Invalidate all existing sessions and any outstanding magic-link JTIs.
    // issue_tokens creates one fresh session for the caller.
    db::sessions::delete_all_for_user(pool, user_id).await?;
    db::jwt_revocations::revoke_for_user(pool, user_id).await?;

    tracing::info!(user_id = %user_id, "password changed — all sessions and magic-link JTIs revoked");

    issue_tokens(pool, config, user_id, &user.role).await
}

pub async fn check_default_password_warning(
    pool: &SqlitePool,
    desk_email: &str,
    email_hash_salt: &str,
) {
    if let Ok(hash) = crypto::hash_email(desk_email, email_hash_salt) {
        if let Ok(Some(user)) = db::users::find_by_email_hash(pool, &hash).await {
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
}

// ── internals ────────────────────────────────────────────────────────────────

async fn issue_tokens(
    pool: &SqlitePool,
    config: &Config,
    user_id: &str,
    role: &str,
) -> AppResult<LoginOutput> {
    let (raw_token, token_hash) = generate_refresh_token();
    let expires_at =
        (Utc::now() + chrono::Duration::seconds(config.refresh_token_ttl_secs)).to_rfc3339();

    let session_id = Uuid::new_v4().to_string();
    db::sessions::create_capped(
        pool,
        db::sessions::NewSession {
            id: &session_id,
            user_id,
            token_hash: &token_hash,
            expires_at: &expires_at,
        },
        MAX_SESSIONS_PER_USER,
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
    let params = Params::new(65_536, 3, 4, None)
        .map_err(|e| AppError::Internal(format!("argon2 params: {e}")))?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("argon2 hash: {e}")))
}

/// Validates password strength. Called on account creation and password change.
/// Rules: at least 8 chars, at most 128, not whitespace-only, not in common list.
pub fn validate_password_strength(password: &str) -> AppResult<()> {
    if password.chars().count() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    if password.chars().count() > 128 {
        return Err(AppError::BadRequest(
            "password must be at most 128 characters".into(),
        ));
    }
    if password.chars().all(|c| c.is_whitespace()) {
        return Err(AppError::BadRequest(
            "password cannot consist entirely of whitespace".into(),
        ));
    }
    if common_passwords().contains(password.to_lowercase().as_str()) {
        return Err(AppError::BadRequest(
            "password is too common — choose a more unique password".into(),
        ));
    }
    Ok(())
}

pub fn verify_password(password: &str, stored_hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|e| AppError::Internal(format!("parse hash: {e}")))?;
    // Params are read from the PHC string embedded in stored_hash, so the
    // explicit Argon2id algorithm and version are the only values that matter here.
    Ok(
        Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
    )
}

fn generate_access_token(config: &Config, user_id: &str, role: &str) -> AppResult<String> {
    let exp = (Utc::now() + chrono::Duration::seconds(config.access_token_ttl_secs)).timestamp();
    let claims = Claims {
        sub: user_id.to_owned(),
        role: role.to_owned(),
        exp,
        session_type: "full".into(),
        ticket_scope: None,
        jti: None, // password-login tokens are stateless — no revocation check
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
