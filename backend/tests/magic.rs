mod common;

use backend::{error::AppError, models::Claims, services::magic};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

fn extract_token(url: &str) -> String {
    url.split("token=").nth(1).unwrap().to_owned()
}

fn decode_jwt(jwt: &str) -> Claims {
    let mut v = Validation::new(Algorithm::HS256);
    v.set_required_spec_claims(&["sub", "exp"]);
    decode::<Claims>(
        jwt,
        &DecodingKey::from_secret(b"test_secret_at_least_32_bytes_long_abcdef"),
        &v,
    )
    .unwrap()
    .claims
}

async fn insert_test_ticket(
    pool: &sqlx::SqlitePool,
    ticket_id: &str,
    desk_id: &str,
    client_id: &str,
) {
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, created_at,
          priority, ticket_type, recurring)
         VALUES (?, 'Test Ticket', 'desc', 'open', ?, ?, ?, 'medium', 'standard', 0)",
    )
    .bind(ticket_id)
    .bind(desk_id)
    .bind(client_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();
}

// â”€â”€ Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn magic_link_generation_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let out = magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();

    assert!(out.url.contains("token="), "URL must contain token param");
}

#[tokio::test]
async fn magic_link_exchange_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let out = magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();
    let token = extract_token(&out.url);

    let jwt = magic::exchange_magic_link(&pool, &config, &token)
        .await
        .unwrap();

    assert!(!jwt.is_empty());
    let claims = decode_jwt(&jwt);
    assert_eq!(claims.sub, client_id);
}

#[tokio::test]
async fn magic_link_is_single_use() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let out = magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();
    let token = extract_token(&out.url);

    magic::exchange_magic_link(&pool, &config, &token)
        .await
        .unwrap();

    let second = magic::exchange_magic_link(&pool, &config, &token).await;
    assert!(
        second.is_err(),
        "second exchange of the same token must fail"
    );
}

#[tokio::test]
async fn expired_magic_link_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let mut config = common::test_config();
    config.magic_link_ttl_secs = -3600; // expires_at is 1 hour in the past
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let out = magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();
    let token = extract_token(&out.url);

    let result = magic::exchange_magic_link(&pool, &config, &token).await;
    assert!(
        matches!(result, Err(AppError::Unauthorized)),
        "expected Unauthorized for expired link, got: {:?}",
        result
    );
}

#[tokio::test]
async fn new_link_creation_revokes_prior_unconsumed_link() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    // Create first link.
    let first = magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();
    let first_token = extract_token(&first.url);

    // Create second link â€” must invalidate the first.
    magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();

    let result = magic::exchange_magic_link(&pool, &config, &first_token).await;
    assert!(
        result.is_err(),
        "first (revoked) token must be rejected after a second link was issued"
    );
}

#[tokio::test]
async fn full_scope_link_creates_full_session() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let out = magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "full", None)
        .await
        .unwrap();
    let token = extract_token(&out.url);
    let jwt = magic::exchange_magic_link(&pool, &config, &token)
        .await
        .unwrap();

    let claims = decode_jwt(&jwt);
    assert_eq!(claims.session_type, "full");
    assert!(claims.ticket_scope.is_none());
}

#[tokio::test]
async fn ticket_scoped_link_creates_scoped_session() {
    let (pool, _dir) = common::setup_test_db().await;
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket_id = uuid::Uuid::new_v4().to_string();
    insert_test_ticket(&pool, &ticket_id, &desk_id, &client_id).await;

    let out =
        magic::create_magic_link(&pool, &config, &common::test_enc_key(), None, &client_id, "ticket", Some(&ticket_id))
            .await
            .unwrap();
    let token = extract_token(&out.url);
    let jwt = magic::exchange_magic_link(&pool, &config, &token)
        .await
        .unwrap();

    let claims = decode_jwt(&jwt);
    assert_eq!(claims.session_type, "scoped");
    assert_eq!(claims.ticket_scope.as_deref(), Some(ticket_id.as_str()));
}

