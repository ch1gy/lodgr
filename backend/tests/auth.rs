mod common;

use backend::{
    db,
    error::AppError,
    models::Claims,
    services::auth::{self, validate_password_strength},
};
use std::net::IpAddr;

const PEER_IP: &str = "127.0.0.1";

fn peer_ip() -> IpAddr {
    PEER_IP.parse().unwrap()
}

// ── Login ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn login_succeeds_with_correct_credentials() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (_, email, password) = common::create_test_client(&pool).await;

    let result = auth::login(&pool, &config, None,&email, &password, peer_ip()).await;

    let output = result.expect("login should succeed with correct credentials");
    assert!(
        !output.access_token.is_empty(),
        "access_token must be non-empty"
    );
    assert!(
        !output.refresh_token.is_empty(),
        "refresh_token must be non-empty"
    );
}

#[tokio::test]
async fn login_fails_with_wrong_password() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (_, email, _) = common::create_test_client(&pool).await;

    let result = auth::login(&pool, &config, None,&email, "WrongPassword999!", peer_ip()).await;

    assert!(
        matches!(result, Err(AppError::Unauthorized)),
        "expected Unauthorized, got: {:?}",
        result,
    );
}

#[tokio::test]
async fn login_fails_with_nonexistent_email() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();

    let result = auth::login(
        &pool,
        &config,
        None,
        "nobody@nowhere.example",
        "SomePassword1!",
        peer_ip(),
    )
    .await;

    assert!(
        matches!(result, Err(AppError::Unauthorized)),
        "expected Unauthorized for unknown email, got: {:?}",
        result,
    );
}

#[tokio::test]
async fn login_fails_when_account_is_soft_deleted() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, email, password) = common::create_test_client(&pool).await;

    // Soft-delete the account.
    sqlx::query("UPDATE users SET deleted_at = ? WHERE id = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();

    let result = auth::login(&pool, &config, None,&email, &password, peer_ip()).await;

    assert!(
        matches!(result, Err(AppError::Unauthorized)),
        "expected Unauthorized for soft-deleted account, got: {:?}",
        result,
    );
}

#[tokio::test]
async fn login_fails_when_account_is_permanently_locked() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, email, password) = common::create_test_client(&pool).await;

    // Trigger permanent lockout: PERMANENT_LOCKOUT_THRESHOLD is 9.
    // The lockout check gates every login once locked_until is set, so we
    // cannot reach failed_attempts=9 through login() calls alone (the account
    // becomes hour-locked at 8).  Directly set failed_attempts to 8 and
    // locked_until to NULL, then make one wrong-password attempt — that
    // increments to 9 and writes the permanent far-future sentinel.
    sqlx::query("UPDATE users SET failed_attempts = 8, locked_until = NULL WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();

    // 9th bad attempt — sets permanent lock, returns Unauthorized.
    let _ = auth::login(&pool, &config, None,&email, "BadPass1234!", peer_ip()).await;

    // 10th attempt (correct password) — account is now permanently locked.
    let result = auth::login(&pool, &config, None,&email, &password, peer_ip()).await;

    assert!(
        matches!(
            result,
            Err(AppError::Locked {
                retry_after_secs: None
            })
        ),
        "expected permanent Locked, got: {:?}",
        result,
    );
}

#[tokio::test]
async fn lockout_escalates_after_five_failures() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (_, email, _) = common::create_test_client(&pool).await;

    // Attempts 1-4: Unauthorized (no lockout yet).
    for _ in 0..4 {
        let r = auth::login(&pool, &config, None,&email, "BadPass1234!", peer_ip()).await;
        assert!(
            matches!(r, Err(AppError::Unauthorized)),
            "expected Unauthorized before lockout threshold, got: {:?}",
            r,
        );
    }

    // 5th failure: increments counter to 5 and writes locked_until, but
    // login() still returns Unauthorized for the attempt that triggered the lock.
    let fifth = auth::login(&pool, &config, None,&email, "BadPass1234!", peer_ip()).await;
    assert!(
        matches!(fifth, Err(AppError::Unauthorized)),
        "expected Unauthorized on the 5th attempt (sets lock), got: {:?}",
        fifth,
    );

    // 6th attempt: the account is now locked — must return Locked.
    let result = auth::login(&pool, &config, None,&email, "BadPass1234!", peer_ip()).await;

    assert!(
        matches!(
            result,
            Err(AppError::Locked {
                retry_after_secs: Some(_)
            })
        ),
        "expected temporary Locked after 5+ failures, got: {:?}",
        result,
    );
}

#[tokio::test]
async fn successful_login_resets_lockout_counter() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, email, password) = common::create_test_client(&pool).await;

    // 4 wrong-password attempts — no lockout yet.
    for _ in 0..4 {
        let _ = auth::login(&pool, &config, None,&email, "BadPass1234!", peer_ip()).await;
    }

    // Successful login should reset the counter.
    let result = auth::login(&pool, &config, None,&email, &password, peer_ip()).await;
    assert!(
        result.is_ok(),
        "expected successful login, got: {:?}",
        result
    );

    // Verify the DB record has failed_attempts = 0.
    let user = db::users::find_by_id(&pool, &id)
        .await
        .unwrap()
        .expect("user must exist");

    assert_eq!(
        user.failed_attempts, 0,
        "failed_attempts should be reset to 0 after a successful login"
    );
}

// ── Refresh-token rotation ────────────────────────────────────────────────────

#[tokio::test]
async fn refresh_token_rotation_works() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (_, email, password) = common::create_test_client(&pool).await;

    let first = auth::login(&pool, &config, None,&email, &password, peer_ip())
        .await
        .expect("login should succeed");

    // Rotate: exchange the refresh token for a new pair.
    let second = auth::refresh(&pool, &config, &first.refresh_token)
        .await
        .expect("first refresh should succeed");

    assert!(!second.access_token.is_empty());
    assert!(!second.refresh_token.is_empty());
    // Refresh tokens are random — they must always differ.
    assert_ne!(first.refresh_token, second.refresh_token);
    // Access tokens include a second-precision exp claim; they may be equal
    // when issued in the same second, so we only assert non-empty above.

    // The old refresh token is now revoked — using it again must fail.
    let replay = auth::refresh(&pool, &config, &first.refresh_token).await;
    assert!(
        matches!(replay, Err(AppError::Unauthorized)),
        "expected Unauthorized when reusing old refresh token, got: {:?}",
        replay,
    );
}

#[tokio::test]
async fn refresh_token_replay_detected() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (_, email, password) = common::create_test_client(&pool).await;

    let first = auth::login(&pool, &config, None,&email, &password, peer_ip())
        .await
        .expect("login should succeed");

    // Rotate once so `first.refresh_token` becomes stale (revoked).
    let _second = auth::refresh(&pool, &config, &first.refresh_token)
        .await
        .expect("first refresh should succeed");

    // Replay the old (now revoked) token — service detects theft and wipes
    // all sessions for the user.
    let replay = auth::refresh(&pool, &config, &first.refresh_token).await;
    assert!(
        matches!(replay, Err(AppError::Unauthorized)),
        "expected Unauthorized on revoked-token replay, got: {:?}",
        replay,
    );

    // All sessions for the user have been wiped — the fresh token is also gone.
    let result_after_wipe = auth::refresh(&pool, &config, &_second.refresh_token).await;
    assert!(
        matches!(result_after_wipe, Err(AppError::Unauthorized)),
        "expected Unauthorized after sessions wiped by replay detection, got: {:?}",
        result_after_wipe,
    );
}

#[tokio::test]
async fn refresh_fails_with_expired_token() {
    let (pool, _dir) = common::setup_test_db().await;

    // Issue a session that expires in the past (TTL = -1 second).
    let mut expired_config = common::test_config();
    expired_config.refresh_token_ttl_secs = -1;

    let (_, email, password) = common::create_test_client(&pool).await;
    let output = auth::login(&pool, &expired_config, None, &email, &password, peer_ip())
        .await
        .expect("login should succeed even with negative TTL");

    // The session's expires_at is already in the past — refresh must reject it.
    let result = auth::refresh(&pool, &expired_config, &output.refresh_token).await;
    assert!(
        matches!(result, Err(AppError::Unauthorized)),
        "expected Unauthorized for expired refresh token, got: {:?}",
        result,
    );
}

#[tokio::test]
async fn refresh_fails_with_revoked_token() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, email, password) = common::create_test_client(&pool).await;

    let output = auth::login(&pool, &config, None,&email, &password, peer_ip())
        .await
        .expect("login should succeed");

    // Delete all sessions for the user — simulates revocation without knowing
    // the internal session ID.
    db::sessions::delete_all_for_user(&pool, &id)
        .await
        .expect("delete_all_for_user should succeed");

    let result = auth::refresh(&pool, &config, &output.refresh_token).await;
    assert!(
        matches!(result, Err(AppError::Unauthorized)),
        "expected Unauthorized after session revocation, got: {:?}",
        result,
    );
}

// ── Change password ───────────────────────────────────────────────────────────

#[tokio::test]
async fn change_password_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, email, old_password) = common::create_test_desk(&pool).await;
    let new_password = "NewSecurePass99!";

    let result = auth::change_password(&pool, &config, &id, &old_password, new_password).await;
    assert!(
        result.is_ok(),
        "change_password should succeed, got: {:?}",
        result
    );

    // Old password must now be rejected.
    let old_attempt = auth::login(&pool, &config, None,&email, &old_password, peer_ip()).await;
    assert!(
        matches!(old_attempt, Err(AppError::Unauthorized)),
        "old password should be rejected after change, got: {:?}",
        old_attempt,
    );

    // New password must be accepted.
    let new_attempt = auth::login(&pool, &config, None,&email, new_password, peer_ip()).await;
    assert!(
        new_attempt.is_ok(),
        "new password should be accepted after change, got: {:?}",
        new_attempt,
    );
}

#[tokio::test]
async fn change_password_fails_with_wrong_current_password() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, _, _) = common::create_test_desk(&pool).await;

    let result =
        auth::change_password(&pool, &config, &id, "NotTheRealPassword!", "NewPass999!").await;

    assert!(
        matches!(result, Err(AppError::Unauthorized)),
        "expected Unauthorized for wrong current password, got: {:?}",
        result,
    );
}

#[tokio::test]
async fn change_password_rejects_common_password() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, _, current_password) = common::create_test_desk(&pool).await;

    // "password" is in the common-password blocklist.
    let result = auth::change_password(&pool, &config, &id, &current_password, "password").await;

    assert!(
        matches!(result, Err(AppError::BadRequest(_))),
        "expected BadRequest for common new password, got: {:?}",
        result,
    );
}

// ── Client password self-service ─────────────────────────────────────────────

#[tokio::test]
async fn client_can_change_their_own_password() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    // Clients use create_test_client, not create_test_desk.
    let (id, email, old_password) = common::create_test_client(&pool).await;
    let new_password = "ClientNewPass99!";

    let result = auth::change_password(&pool, &config, &id, &old_password, new_password).await;
    assert!(
        result.is_ok(),
        "client password change must succeed, got: {:?}",
        result
    );

    // Old password must now be rejected.
    let old_attempt = auth::login(&pool, &config, None,&email, &old_password, peer_ip()).await;
    assert!(
        matches!(old_attempt, Err(AppError::Unauthorized)),
        "old password must be rejected after client changes it"
    );

    // New password must be accepted.
    assert!(
        auth::login(&pool, &config, None,&email, new_password, peer_ip())
            .await
            .is_ok(),
        "new password must work after client changes it"
    );
}

// ── Scoped session cannot change password ────────────────────────────────────

#[test]
fn scoped_session_cannot_change_password() {
    // The FullSessionUser extractor calls require_full_session(), which must
    // return Forbidden for scoped magic-link tokens regardless of role.
    let scoped = Claims {
        sub: "some-client".into(),
        role: "client".into(),
        exp: 9_999_999_999,
        session_type: "scoped".into(),
        ticket_scope: Some("some-ticket".into()),
        jti: Some("some-jti".into()),
    };
    assert!(
        matches!(scoped.require_full_session(), Err(AppError::Forbidden)),
        "scoped session must be rejected by require_full_session"
    );
}

// ── Password change revokes outstanding magic-link JTIs ──────────────────────

#[tokio::test]
async fn change_password_revokes_outstanding_magic_jtis() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, _, current_password) = common::create_test_client(&pool).await;

    // Plant an active JTI simulating an outstanding magic-link session.
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    db::jwt_revocations::create(&pool, "jti-before-pw-change", &id, &expires_at)
        .await
        .unwrap();
    assert!(
        db::jwt_revocations::is_active(&pool, "jti-before-pw-change")
            .await
            .unwrap(),
        "JTI must be active before password change"
    );

    auth::change_password(&pool, &config, &id, &current_password, "BrandNewPass99!")
        .await
        .unwrap();

    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-before-pw-change")
            .await
            .unwrap(),
        "outstanding JTI must be revoked after password change"
    );
}

// ── auth_events — login and logout events are recorded ───────────────────────

#[tokio::test]
async fn successful_login_writes_login_ok_event() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, email, password) = common::create_test_client(&pool).await;

    auth::login(&pool, &config, None,&email, &password, peer_ip())
        .await
        .unwrap();

    let events = backend::db::auth_events::list_recent_for_user(&pool, &id, 10)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "login_ok");
}

#[tokio::test]
async fn logout_writes_logout_event() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (id, email, password) = common::create_test_client(&pool).await;

    let output = auth::login(&pool, &config, None,&email, &password, peer_ip())
        .await
        .unwrap();
    auth::logout(&pool, &output.refresh_token).await.unwrap();

    let events = backend::db::auth_events::list_recent_for_user(&pool, &id, 10)
        .await
        .unwrap();
    // login_ok + logout, newest first
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "logout");
    assert_eq!(events[1].event_type, "login_ok");
}

#[tokio::test]
async fn auth_event_failure_does_not_block_login() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (_, email, password) = common::create_test_client(&pool).await;

    // Simulate a DB failure by dropping the auth_events table.
    // The login must still succeed — event writes are non-fatal.
    sqlx::query("DROP TABLE auth_events")
        .execute(&pool)
        .await
        .unwrap();

    let result = auth::login(&pool, &config, None,&email, &password, peer_ip()).await;
    assert!(
        result.is_ok(),
        "login must succeed even when auth_events write fails, got: {:?}",
        result
    );
}

// ── validate_password_strength (unit, no DB needed) ──────────────────────────
// These are already covered in validation.rs; kept here as a quick smoke-check
// alongside the auth service tests.

#[test]
fn password_strength_rejects_short_password() {
    assert!(matches!(
        validate_password_strength("short"),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn password_strength_accepts_strong_password() {
    assert!(validate_password_strength("Tr0ub4dor&3").is_ok());
}
