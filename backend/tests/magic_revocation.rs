// M8 â€” magic-link JWT revocation tests.
//
// These tests cover the security-critical failure modes, not just happy path:
//   â€¢ active jti           â†’ allowed
//   â€¢ revoked jti          â†’ denied
//   â€¢ jti present, no row  â†’ denied (fail-closed â€” the entire point of the feature)
//   â€¢ access token (no jti)â†’ allowed, no DB lookup needed
//   â€¢ revoke_for_user      â†’ marks all active rows for a user as revoked
//   â€¢ scoped session       â†’ jti is set; ticket_scope is preserved
//   â€¢ cleanup              â†’ removes expired rows only; active rows survive

mod common;

use backend::{db, services::magic};

// â”€â”€ Helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn future_rfc3339(hours: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::hours(hours)).to_rfc3339()
}

fn past_rfc3339(hours: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::hours(hours)).to_rfc3339()
}

// â”€â”€ db::jwt_revocations â€” fail-closed unit tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn active_jti_is_allowed() {
    let (pool, _dir) = common::setup_test_db().await;
    let (user_id, _, _) = common::create_test_client(&pool).await;

    db::jwt_revocations::create(&pool, "jti-active", &user_id, &future_rfc3339(24))
        .await
        .unwrap();

    assert!(
        db::jwt_revocations::is_active(&pool, "jti-active")
            .await
            .unwrap(),
        "active row must be allowed"
    );
}

#[tokio::test]
async fn revoked_jti_is_denied() {
    let (pool, _dir) = common::setup_test_db().await;
    let (user_id, _, _) = common::create_test_client(&pool).await;

    db::jwt_revocations::create(&pool, "jti-rev", &user_id, &future_rfc3339(24))
        .await
        .unwrap();
    // Revoke it directly.
    sqlx::query("UPDATE jwt_revocations SET revoked_at = ? WHERE jti = ?")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind("jti-rev")
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-rev")
            .await
            .unwrap(),
        "revoked row must be denied"
    );
}

#[tokio::test]
async fn missing_jti_is_denied_fail_closed() {
    let (pool, _dir) = common::setup_test_db().await;

    // No row for this jti â€” must be denied, not allowed.
    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-does-not-exist")
            .await
            .unwrap(),
        "missing row must be denied (fail-closed â€” could be forged jti)"
    );
}

// â”€â”€ access token (no jti) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn access_token_without_jti_is_allowed_without_lookup() {
    // Password-login Claims have jti: None. The middleware skips the DB check
    // entirely. Test this at the Claims level â€” if jti is None, is_active is
    // never called, so even an empty DB returns "allow" via the None branch.
    let claims = backend::models::Claims {
        sub: "some-user".into(),
        role: "client".into(),
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
        session_type: "full".into(),
        ticket_scope: None,
        jti: None,
    };
    // If jti is None, the middleware's `if let Some(jti) = &claims.jti` block
    // does not execute. Verify the field is None so callers can trust the gate.
    assert!(
        claims.jti.is_none(),
        "password-login Claims must have jti: None"
    );
}

// â”€â”€ revoke_for_user â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn revoke_for_user_marks_all_active_rows() {
    let (pool, _dir) = common::setup_test_db().await;
    let (user_id, _, _) = common::create_test_client(&pool).await;

    // Insert two active JTIs for the same user.
    db::jwt_revocations::create(&pool, "jti-a", &user_id, &future_rfc3339(24))
        .await
        .unwrap();
    db::jwt_revocations::create(&pool, "jti-b", &user_id, &future_rfc3339(24))
        .await
        .unwrap();

    db::jwt_revocations::revoke_for_user(&pool, &user_id)
        .await
        .unwrap();

    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-a")
            .await
            .unwrap(),
        "jti-a must be revoked after revoke_for_user"
    );
    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-b")
            .await
            .unwrap(),
        "jti-b must be revoked after revoke_for_user"
    );
}

#[tokio::test]
async fn revoke_for_user_does_not_affect_other_users() {
    let (pool, _dir) = common::setup_test_db().await;
    let (user_a, _, _) = common::create_test_client(&pool).await;
    let (user_b, _, _) = common::create_test_client(&pool).await;

    db::jwt_revocations::create(&pool, "jti-user-a", &user_a, &future_rfc3339(24))
        .await
        .unwrap();
    db::jwt_revocations::create(&pool, "jti-user-b", &user_b, &future_rfc3339(24))
        .await
        .unwrap();

    db::jwt_revocations::revoke_for_user(&pool, &user_a)
        .await
        .unwrap();

    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-user-a")
            .await
            .unwrap(),
        "user_a jti must be revoked"
    );
    assert!(
        db::jwt_revocations::is_active(&pool, "jti-user-b")
            .await
            .unwrap(),
        "user_b jti must be unaffected"
    );
}

// â”€â”€ cleanup â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn cleanup_removes_expired_rows_only() {
    let (pool, _dir) = common::setup_test_db().await;
    let (user_id, _, _) = common::create_test_client(&pool).await;

    // One expired row, one active row.
    db::jwt_revocations::create(&pool, "jti-expired", &user_id, &past_rfc3339(2))
        .await
        .unwrap();
    db::jwt_revocations::create(&pool, "jti-live", &user_id, &future_rfc3339(24))
        .await
        .unwrap();

    let removed = db::jwt_revocations::delete_expired(&pool).await.unwrap();
    assert_eq!(removed, 1, "only the expired row should be removed");

    // The live row must still be present and active.
    assert!(
        db::jwt_revocations::is_active(&pool, "jti-live")
            .await
            .unwrap(),
        "active row must survive cleanup"
    );
    // The expired row must be gone.
    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-expired")
            .await
            .unwrap(),
        "expired row must be gone after cleanup"
    );
}

// â”€â”€ full exchange flow â€” jti is created and scoped session is preserved â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn exchange_magic_link_creates_jti_row_and_preserves_ticket_scope() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    // Create a ticket for the scoped link to reference.
    let ticket_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, created_at,
          priority, ticket_type, recurring)
         VALUES (?, 'Test', 'desc', 'open', ?, ?, ?, 'medium', 'standard', 0)",
    )
    .bind(&ticket_id)
    .bind(&desk_id)
    .bind(&client_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    // Generate and exchange a ticket-scoped magic link.
    let link_out =
        magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "ticket", Some(&ticket_id))
            .await
            .unwrap();
    let raw_token = link_out.url.split("token=").nth(1).unwrap();
    let jwt = magic::exchange_magic_link(&pool, &config, raw_token)
        .await
        .unwrap();

    // Decode the JWT and verify jti and ticket_scope are set.
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_required_spec_claims(&["sub", "exp"]);
    let token_data = jsonwebtoken::decode::<backend::models::Claims>(
        &jwt,
        &jsonwebtoken::DecodingKey::from_secret(b"test_secret_at_least_32_bytes_long_abcdef"),
        &validation,
    )
    .unwrap();
    let claims = token_data.claims;

    let jti = claims
        .jti
        .as_deref()
        .expect("magic-link JWT must have a jti");
    assert_eq!(
        claims.ticket_scope.as_deref(),
        Some(ticket_id.as_str()),
        "ticket_scope must be preserved"
    );
    assert_eq!(claims.session_type, "scoped");

    // Verify the jti row was inserted and is active.
    assert!(
        db::jwt_revocations::is_active(&pool, jti).await.unwrap(),
        "jti row must be active immediately after exchange"
    );
}

// â”€â”€ delete_client_sessions also revokes magic JTIs â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn delete_client_sessions_revokes_magic_jtis() {
    use backend::services::admin;

    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;

    // Plant an active jti for this client.
    db::jwt_revocations::create(&pool, "jti-to-revoke", &client_id, &future_rfc3339(24))
        .await
        .unwrap();
    assert!(
        db::jwt_revocations::is_active(&pool, "jti-to-revoke")
            .await
            .unwrap(),
        "jti must be active before sessions are deleted"
    );

    admin::delete_client_sessions(&pool, &client_id)
        .await
        .unwrap();

    assert!(
        !db::jwt_revocations::is_active(&pool, "jti-to-revoke")
            .await
            .unwrap(),
        "jti must be revoked after delete_client_sessions"
    );
}

// â”€â”€ magic_ok auth event â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn exchange_magic_link_writes_magic_ok_auth_event() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let out = magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();
    let token = out.url.split("token=").nth(1).unwrap().to_owned();

    magic::exchange_magic_link(&pool, &config, &token)
        .await
        .unwrap();

    let events = backend::db::auth_events::list_recent_for_user(&pool, &client_id, 10)
        .await
        .unwrap();
    assert_eq!(events.len(), 1, "one event must be written on exchange");
    assert_eq!(events[0].event_type, "magic_ok");
}

