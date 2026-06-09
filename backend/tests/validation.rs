mod common;

use backend::{
    error::AppError,
    models::Claims,
    services::{
        auth::validate_password_strength,
        tickets::{create, CreateTicketInput},
    },
};
use chrono::Utc;

// ── Password strength ─────────────────────────────────────────────────────────

#[test]
fn password_too_short_rejected() {
    assert!(validate_password_strength("short").is_err());
}

#[test]
fn password_minimum_length_accepted() {
    assert!(validate_password_strength("abcdefgh").is_ok());
}

#[test]
fn password_whitespace_only_rejected() {
    assert!(validate_password_strength("        ").is_err());
}

#[test]
fn common_password_rejected() {
    assert!(validate_password_strength("password").is_err());
    assert!(validate_password_strength("changeme").is_err());
    assert!(validate_password_strength("12345678").is_err());
}

#[test]
fn strong_password_accepted() {
    assert!(validate_password_strength("C0rrect_H0rse_Battery_Staple").is_ok());
}

// ── Email format (tested through create_client) ───────────────────────────────

#[tokio::test]
async fn email_missing_at_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();
    let r = backend::services::admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "Test".into(),
        "notanemail".into(),
        "ValidPass123!".into(),
        None,
    )
    .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn email_missing_domain_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();
    let r = backend::services::admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "Test".into(),
        "user@".into(),
        "ValidPass123!".into(),
        None,
    )
    .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn email_leading_dot_in_domain_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();
    let r = backend::services::admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "Test".into(),
        "user@.example.com".into(),
        "ValidPass123!".into(),
        None,
    )
    .await;
    assert!(r.is_err());
}

// ── Ticket field validation ───────────────────────────────────────────────────

fn desk_claims(user_id: &str) -> Claims {
    Claims {
        sub: user_id.to_owned(),
        role: "desk".to_owned(),
        exp: (Utc::now() + chrono::Duration::hours(1)).timestamp(),
        session_type: "full".to_owned(),
        ticket_scope: None,
        jti: None,
    }
}

#[tokio::test]
async fn ticket_date_format_yyyy_mm_dd_accepted() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let r = create(
        &pool,
        None,
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "Test",
            priority: "medium",
            category: None,
            due_date: Some("2025-12-31"),
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await;
    assert!(r.is_ok());
}

#[tokio::test]
async fn ticket_date_format_dd_mm_yyyy_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let r = create(
        &pool,
        None,
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "Test",
            priority: "medium",
            category: None,
            due_date: Some("31/12/2025"),
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn recurring_interval_days_zero_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let r = create(
        &pool,
        None,
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "Test",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: true,
            recurring_interval_days: Some(0),
            client_id: Some(&client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn recurring_interval_days_366_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let r = create(
        &pool,
        None,
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "Test",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: true,
            recurring_interval_days: Some(366),
            client_id: Some(&client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn recurring_interval_days_1_accepted() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let r = create(
        &pool,
        None,
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "Test",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: true,
            recurring_interval_days: Some(1),
            client_id: Some(&client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await;
    assert!(r.is_ok());
}

#[tokio::test]
async fn recurring_interval_days_365_accepted() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let r = create(
        &pool,
        None,
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "Test",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: true,
            recurring_interval_days: Some(365),
            client_id: Some(&client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await;
    assert!(r.is_ok());
}

#[tokio::test]
async fn category_over_100_chars_rejected() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let long_cat = "x".repeat(101);
    let r = create(
        &pool,
        None,
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "Test",
            priority: "medium",
            category: Some(&long_cat),
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await;
    assert!(matches!(r, Err(AppError::BadRequest(_))));
}
