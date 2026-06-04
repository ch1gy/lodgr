use chrono::Utc;
use jsonwebtoken::{encode, Header};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Config,
    db,
    email::SmtpMailer,
    error::{AppError, AppResult},
    models::Claims,
};

pub struct MagicLinkOutput {
    /// Full URL ready to be sent via any channel. Always included regardless
    /// of whether SMTP is configured — desk can copy it to WhatsApp, etc.
    pub url: String,
}

pub async fn create_magic_link(
    pool: &SqlitePool,
    config: &Config,
    mailer: Option<&SmtpMailer>,
    target_user_id: &str,
    scope: &str,
    ticket_id: Option<&str>,
) -> AppResult<MagicLinkOutput> {
    let user = db::users::find_by_id(pool, target_user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if user.role != "client" {
        return Err(AppError::BadRequest(
            "magic links can only be created for client users".into(),
        ));
    }
    if user.deleted_at.is_some() {
        return Err(AppError::BadRequest("user has been deleted".into()));
    }

    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let raw_token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let token_hash: String = Sha256::digest(raw_token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let expires_at =
        (Utc::now() + chrono::Duration::seconds(config.magic_link_ttl_secs)).to_rfc3339();

    // Invalidate any prior unconsumed links for this user+scope before issuing a new one.
    // Caps outstanding links to 1 per user/scope/ticket — generating a new link
    // revokes all previous unexchanged ones.
    db::magic_links::delete_unused_for_user_scope(pool, target_user_id, scope, ticket_id).await?;

    let id = Uuid::new_v4().to_string();
    db::magic_links::create(
        pool,
        db::magic_links::NewMagicLink {
            id: &id,
            token_hash: &token_hash,
            user_id: target_user_id,
            scope,
            ticket_id,
            expires_at: &expires_at,
        },
    )
    .await?;

    tracing::info!(
        target_client_id = %target_user_id,
        scope = %scope,
        ticket_id = ?ticket_id,
        "magic link created"
    );

    let url = format!("{}/auth/magic?token={}", config.base_url, raw_token);

    // Fire-and-forget: email failure is non-fatal and already logged inside send_magic_link.
    if let Some(m) = mailer {
        let m = m.clone();
        let email = user.email.clone();
        let name = user.name.clone();
        let link = url.clone();
        tokio::spawn(async move {
            m.send_magic_link(&email, &name, &link).await;
        });
    }

    Ok(MagicLinkOutput { url })
}

pub async fn exchange_magic_link(
    pool: &SqlitePool,
    config: &Config,
    raw_token: &str,
) -> AppResult<String> {
    let token_hash: String = Sha256::digest(raw_token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let link = db::magic_links::find_by_token_hash(pool, &token_hash)
        .await?
        .ok_or_else(|| {
            tracing::warn!("magic link exchange failed — token not found");
            AppError::Unauthorized
        })?;

    if link.used_at.is_some() {
        tracing::warn!(
            link_id = %link.id,
            user_id = %link.user_id,
            "magic link exchange failed — already used"
        );
        return Err(AppError::BadRequest(
            "magic link has already been used".into(),
        ));
    }

    let expires_at = chrono::DateTime::parse_from_rfc3339(&link.expires_at)
        .map_err(|_| AppError::Internal("invalid expires_at".into()))?
        .with_timezone(&Utc);

    if expires_at < Utc::now() {
        tracing::warn!(
            link_id = %link.id,
            user_id = %link.user_id,
            "magic link exchange failed — expired"
        );
        return Err(AppError::Unauthorized);
    }

    db::magic_links::mark_used(pool, &link.id).await?;
    // Reset any lockout on the account — exchanging a valid link is a successful
    // authentication regardless of prior failed password attempts.
    if let Err(e) = db::users::reset_lockout(pool, &link.user_id).await {
        tracing::warn!(user_id = %link.user_id, %e, "failed to reset lockout on magic link exchange");
    }

    let user = db::users::find_by_id(pool, &link.user_id)
        .await?
        .ok_or_else(|| AppError::Internal("magic link references missing user".into()))?;

    if user.deleted_at.is_some() {
        tracing::warn!(
            link_id = %link.id,
            user_id = %link.user_id,
            "magic link exchange failed — user deleted"
        );
        return Err(AppError::Unauthorized);
    }

    let (session_type, ticket_scope): (String, Option<String>) = if link.scope == "ticket" {
        ("scoped".into(), link.ticket_id.clone())
    } else {
        // "full" scope — client gets a normal full session, identical to a
        // password login except it comes from a magic link.
        ("full".into(), None)
    };

    // Compute exp and expires_at together so they refer to the same instant.
    let exp_dt = Utc::now() + chrono::Duration::seconds(config.scoped_token_ttl_secs);
    let exp = exp_dt.timestamp();
    let expires_at = exp_dt.to_rfc3339();

    // Every magic-link JWT carries a jti so it can be individually revoked.
    // Password-login access tokens are stateless and never carry jti.
    let jti = Uuid::new_v4().to_string();

    let claims = Claims {
        sub: user.id.clone(),
        role: user.role.clone(),
        exp,
        session_type: session_type.clone(),
        ticket_scope: ticket_scope.clone(),
        jti: Some(jti.clone()),
    };

    let jwt = encode(&Header::default(), &claims, &config.encoding_key())
        .map_err(|e| AppError::Internal(format!("jwt encode: {e}")))?;

    // Insert the active jti row BEFORE returning the token.
    // If this insert fails the client never receives the token, so there is
    // no window in which a token without a DB record can be presented.
    db::jwt_revocations::create(pool, &jti, &user.id, &expires_at).await?;

    if let Err(e) = db::auth_events::create(pool, &user.id, "magic_ok").await {
        tracing::warn!(user_id = %user.id, %e, "failed to write magic_ok auth event");
    }

    tracing::info!(
        link_id = %link.id,
        user_id = %user.id,
        jti = %jti,
        session_type = %session_type,
        ticket_scope = ?ticket_scope,
        "magic link exchanged successfully"
    );

    Ok(jwt)
}

/// Auto-send a recovery magic link when the desk account is permanently locked.
/// Non-fatal — all errors are logged. Rate-limited to one email per 5 minutes.
/// Only fires if SMTP is configured; otherwise the caller logs a manual-recovery
/// instruction for the operator.
pub async fn send_desk_recovery_link(
    pool: &SqlitePool,
    config: &Config,
    mailer: &SmtpMailer,
    user_id: &str,
    user_email: &str,
    user_name: &str,
) {
    // Rate-limit: skip if an active link was already sent in the last 5 minutes.
    match db::magic_links::has_recent_active_for_user(pool, user_id).await {
        Ok(true) => {
            tracing::info!(user_id = %user_id, "desk recovery link already sent recently, skipping");
            return;
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, %e, "failed to check recent desk recovery links");
            return;
        }
        Ok(false) => {}
    }

    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let raw_token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let token_hash: String = Sha256::digest(raw_token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let expires_at =
        (Utc::now() + chrono::Duration::seconds(config.magic_link_ttl_secs)).to_rfc3339();

    if let Err(e) = db::magic_links::delete_unused_for_user_scope(pool, user_id, "full", None).await
    {
        tracing::warn!(user_id = %user_id, %e, "failed to clear old desk recovery links");
    }

    let id = Uuid::new_v4().to_string();
    if let Err(e) = db::magic_links::create(
        pool,
        db::magic_links::NewMagicLink {
            id: &id,
            token_hash: &token_hash,
            user_id,
            scope: "full",
            ticket_id: None,
            expires_at: &expires_at,
        },
    )
    .await
    {
        tracing::warn!(user_id = %user_id, %e, "failed to create desk recovery magic link");
        return;
    }

    let url = format!("{}/auth/magic?token={}", config.base_url, raw_token);
    tracing::warn!(user_id = %user_id, "desk account permanently locked — recovery link sent");

    let m = mailer.clone();
    let email = user_email.to_owned();
    let name = user_name.to_owned();
    tokio::spawn(async move {
        m.send_magic_link(&email, &name, &url).await;
    });
}
