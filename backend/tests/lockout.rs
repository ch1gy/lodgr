// Lockout-specific integration tests.
//
// Covers the three new behaviours added alongside the auto-ticket feature:
//   1. Permanent client lockout auto-creates a security_log ticket (urgent).
//   2. A second lockout within 24 h does NOT create a duplicate ticket.
//   3. Exchanging a magic link resets failed_attempts and locked_until.
//   4. Desk account lockout does NOT create a ticket (client-only behaviour).

mod common;

use backend::{
    db,
    services::{auth, magic},
};
use std::net::IpAddr;

fn peer_ip() -> IpAddr {
    "127.0.0.1".parse().unwrap()
}

// ── 1. Auto-ticket on first permanent client lockout ──────────────────────────

#[tokio::test]
async fn permanent_lockout_auto_creates_urgent_security_log_ticket() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, email, _) = common::create_test_client(&pool).await;

    // Advance to 8 failures so the next wrong attempt hits the permanent tier.
    sqlx::query("UPDATE users SET failed_attempts = 8, locked_until = NULL WHERE id = ?")
        .bind(&client_id)
        .execute(&pool)
        .await
        .unwrap();

    // 9th wrong attempt → permanent lock → auto-ticket.
    let _ = auth::login(
        &pool,
        &config,
        &common::test_enc_key(),
        None,
        &email,
        "BadPass1234!",
        peer_ip(),
    )
    .await;

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tickets
         WHERE client_id = ? AND ticket_type = 'security_log' AND deleted_at IS NULL",
    )
    .bind(&client_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        count, 1,
        "one security_log ticket must be auto-created on permanent lockout"
    );

    let (priority,): (String,) = sqlx::query_as(
        "SELECT priority FROM tickets WHERE client_id = ? AND ticket_type = 'security_log'",
    )
    .bind(&client_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        priority, "urgent",
        "auto-created lockout ticket must have urgent priority"
    );
}

// ── 2. Deduplication — no second ticket within 24 h ──────────────────────────

#[tokio::test]
async fn lockout_ticket_not_duplicated_within_24h() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, email, _) = common::create_test_client(&pool).await;

    // First lockout — ticket is created.
    sqlx::query("UPDATE users SET failed_attempts = 8, locked_until = NULL WHERE id = ?")
        .bind(&client_id)
        .execute(&pool)
        .await
        .unwrap();
    let _ = auth::login(
        &pool,
        &config,
        &common::test_enc_key(),
        None,
        &email,
        "BadPass1234!",
        peer_ip(),
    )
    .await;

    // Simulate admin recovery (unlock without full reset — same scenario the desk
    // would do via the unlock endpoint, which clears the lockout but the guard
    // window is still < 24 h).
    sqlx::query("UPDATE users SET failed_attempts = 8, locked_until = NULL WHERE id = ?")
        .bind(&client_id)
        .execute(&pool)
        .await
        .unwrap();

    // Second lockout within 24 h — must NOT create a duplicate.
    let _ = auth::login(
        &pool,
        &config,
        &common::test_enc_key(),
        None,
        &email,
        "BadPass1234!",
        peer_ip(),
    )
    .await;

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tickets
         WHERE client_id = ? AND ticket_type = 'security_log' AND deleted_at IS NULL",
    )
    .bind(&client_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        count, 1,
        "deduplication guard must prevent a second ticket within 24 h"
    );
}

// ── 3. Magic link exchange clears the lockout counter ────────────────────────

#[tokio::test]
async fn magic_link_exchange_resets_lockout() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    // Put the account in a temporary locked state (7 failures, 15-min window).
    let locked_until = (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339();
    sqlx::query("UPDATE users SET failed_attempts = 7, locked_until = ? WHERE id = ?")
        .bind(&locked_until)
        .bind(&client_id)
        .execute(&pool)
        .await
        .unwrap();

    // Exchange a magic link — the exchange itself must reset the lockout.
    let out = magic::create_magic_link(&pool, &config, None, &client_id, "full", None)
        .await
        .unwrap();
    let token = out.url.split("token=").nth(1).unwrap();
    magic::exchange_magic_link(&pool, &config, token)
        .await
        .unwrap();

    let user = db::users::find_by_id(&pool, &client_id)
        .await
        .unwrap()
        .expect("user must still exist");

    assert_eq!(
        user.failed_attempts, 0,
        "failed_attempts must be reset to 0 after magic link exchange"
    );
    assert!(
        user.locked_until.is_none(),
        "locked_until must be cleared after magic link exchange"
    );
}

// ── 4. Desk lockout does NOT create a ticket ─────────────────────────────────

#[tokio::test]
async fn desk_permanent_lockout_does_not_create_ticket() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (desk_id, desk_email, _) = common::create_test_desk(&pool).await;

    sqlx::query("UPDATE users SET failed_attempts = 8, locked_until = NULL WHERE id = ?")
        .bind(&desk_id)
        .execute(&pool)
        .await
        .unwrap();

    // 9th wrong attempt — desk account hits permanent lockout.
    let _ = auth::login(
        &pool,
        &config,
        &common::test_enc_key(),
        None,
        &desk_email,
        "BadPass1234!",
        peer_ip(),
    )
    .await;

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tickets WHERE ticket_type = 'security_log'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        count, 0,
        "desk lockout must not auto-create a security_log ticket"
    );
}
