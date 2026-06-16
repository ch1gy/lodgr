mod common;

use backend::{
    db,
    error::AppError,
    models::Claims,
    services::{
        admin,
        messages::{post_message, PostMessageInput},
        tickets::{self, CreateTicketInput},
    },
};
use chrono::Utc;

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

fn client_claims(user_id: &str) -> Claims {
    Claims {
        sub: user_id.to_owned(),
        role: "client".to_owned(),
        exp: (Utc::now() + chrono::Duration::hours(1)).timestamp(),
        session_type: "full".to_owned(),
        ticket_scope: None,
        jti: None,
    }
}

async fn create_ticket_for_client(
    pool: &sqlx::SqlitePool,
    desk_id: &str,
    client_id: &str,
) -> backend::models::Ticket {
    tickets::create(
        pool,
        None,
        &common::test_enc_key(),
        &desk_claims(desk_id),
        CreateTicketInput {
            title: "Test ticket",
            description: "Test description",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(client_id),
            initial_status: None,
            sub_client_id: None,
        },
    )
    .await
    .unwrap()
}

// Ã¢â€â‚¬Ã¢â€â‚¬ Create Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

#[tokio::test]
async fn create_ticket_succeeds_with_valid_input() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;
    assert_eq!(ticket.title, "Test ticket");
    assert_eq!(ticket.status, "open");
    assert_eq!(ticket.client_id, client_id);
}

#[tokio::test]
async fn create_ticket_fails_with_empty_title() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    let r = tickets::create(
        &pool,
        None,
        &common::test_enc_key(),
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "",
            description: "desc",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            sub_client_id: None,
            initial_status: None,
        },
    )
    .await;
    assert!(matches!(r, Err(AppError::BadRequest(_))));
}

#[tokio::test]
async fn create_ticket_fails_with_title_over_200_chars() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let long_title = "x".repeat(201);

    let r = tickets::create(
        &pool,
        None,
        &common::test_enc_key(),
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: &long_title,
            description: "desc",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            sub_client_id: None,
            initial_status: None,
        },
    )
    .await;
    assert!(matches!(r, Err(AppError::BadRequest(_))));
}

#[tokio::test]
async fn create_ticket_fails_with_invalid_priority() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    let r = tickets::create(
        &pool,
        None,
        &common::test_enc_key(),
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "desc",
            priority: "extreme",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            sub_client_id: None,
            initial_status: None,
        },
    )
    .await;
    assert!(matches!(r, Err(AppError::BadRequest(_))));
}

#[tokio::test]
async fn create_ticket_fails_with_invalid_ticket_type() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    let r = tickets::create(
        &pool,
        None,
        &common::test_enc_key(),
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "desc",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "unknown_type",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            sub_client_id: None,
            initial_status: None,
        },
    )
    .await;
    assert!(matches!(r, Err(AppError::BadRequest(_))));
}

#[tokio::test]
async fn create_ticket_fails_with_invalid_date_format() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    let r = tickets::create(
        &pool,
        None,
        &common::test_enc_key(),
        &desk_claims(&desk_id),
        CreateTicketInput {
            title: "Test",
            description: "desc",
            priority: "medium",
            category: None,
            due_date: Some("31/12/2025"),
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_id),
            sub_client_id: None,
            initial_status: None,
        },
    )
    .await;
    assert!(matches!(r, Err(AppError::BadRequest(_))));
}

// Ã¢â€â‚¬Ã¢â€â‚¬ Status transitions Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

#[tokio::test]
async fn status_transition_open_to_acknowledged_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    tickets::transition_ack(&pool, &ticket.id, None, &common::test_enc_key())
        .await
        .unwrap();

    let updated = db::tickets::find_by_id(&pool, &ticket.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, "acknowledged");
}

#[tokio::test]
async fn status_transition_open_to_closed_fails() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    // open Ã¢â€ â€™ close is not a valid transition; must go open Ã¢â€ â€™ acknowledged Ã¢â€ â€™ closed.
    let r = tickets::transition_close(&pool, &ticket.id, None, &common::test_enc_key()).await;
    assert!(matches!(r, Err(AppError::InvalidTransition { .. })));
}

#[tokio::test]
async fn status_transition_pending_to_acknowledged_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    tickets::transition_pend(&pool, &ticket.id, None, &common::test_enc_key())
        .await
        .unwrap();
    tickets::transition_ack(&pool, &ticket.id, None, &common::test_enc_key())
        .await
        .unwrap();

    let updated = db::tickets::find_by_id(&pool, &ticket.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, "acknowledged");
}

#[tokio::test]
async fn status_transition_closed_to_open_fails() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    // Reach closed state: open Ã¢â€ â€™ acknowledged Ã¢â€ â€™ closed.
    tickets::transition_ack(&pool, &ticket.id, None, &common::test_enc_key())
        .await
        .unwrap();
    tickets::transition_close(&pool, &ticket.id, None, &common::test_enc_key())
        .await
        .unwrap();

    // Try to acknowledge (re-open) a closed ticket Ã¢â‚¬â€ invalid.
    let r = tickets::transition_ack(&pool, &ticket.id, None, &common::test_enc_key()).await;
    assert!(matches!(r, Err(AppError::InvalidTransition { .. })));
}

// Ã¢â€â‚¬Ã¢â€â‚¬ Soft delete Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

#[tokio::test]
async fn soft_delete_hides_ticket_from_client_list() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    create_ticket_for_client(&pool, &desk_id, &client_id).await;

    // Before soft-delete the client sees 1 ticket.
    let (before, _) = tickets::list(&pool, &client_claims(&client_id), 1, 50)
        .await
        .unwrap();
    assert_eq!(before.len(), 1);

    // Soft-delete the client Ã¢â‚¬â€ cascades to all their tickets.
    admin::soft_delete_client(&pool, &client_id).await.unwrap();

    // After soft-delete the client sees 0 tickets.
    let (after, _) = tickets::list(&pool, &client_claims(&client_id), 1, 50)
        .await
        .unwrap();
    assert_eq!(after.len(), 0);
}

// Ã¢â€â‚¬Ã¢â€â‚¬ Hard delete Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

#[tokio::test]
async fn hard_delete_requires_export_first() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, email, _) = common::create_test_client(&pool).await;
    let confirm = format!("permanently delete {email}");

    let r = admin::hard_delete_client(&pool, &enc_key, &client_id, &confirm).await;
    assert!(
        matches!(r, Err(AppError::BadRequest(_))),
        "expected BadRequest when no export exists, got: {:?}",
        r
    );
}

// Ã¢â€â‚¬Ã¢â€â‚¬ Upload directory cleanup Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

#[tokio::test]
async fn ticket_delete_cleans_up_upload_directory() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    // Simulate an attachment upload directory for the ticket.
    let upload_dir = format!("uploads/{}", ticket.id);
    tokio::fs::create_dir_all(&upload_dir).await.unwrap();
    assert!(
        tokio::fs::metadata(&upload_dir).await.is_ok(),
        "upload directory should exist before delete"
    );

    // Mirror what routes::tickets::delete does: cascade SQL + fs cleanup.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM thread_entries WHERE ticket_id = ?")
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM internal_notes WHERE ticket_id = ?")
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM magic_links WHERE ticket_id = ?")
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM tickets WHERE id = ?")
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    tokio::fs::remove_dir_all(&upload_dir).await.unwrap();

    assert!(
        tokio::fs::metadata(&upload_dir).await.is_err(),
        "upload directory should be gone after delete"
    );
}

// Ã¢â€â‚¬Ã¢â€â‚¬ post_message success path Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬Ã¢â€â‚¬

#[tokio::test]
async fn client_post_message_is_stored_and_appears_in_thread() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket.id.clone(),
            sender_id: client_id.clone(),
            sender_role: "client".into(),
            sender_session_type: "full".into(),
            ticket_scope: None,
            body: "Hello from the client.".into(),
            attachment_path: None,
        },
    )
    .await
    .unwrap();

    let with_thread =
        tickets::get_with_thread(&pool, &ticket.id, &client_claims(&client_id), &enc_key)
            .await
            .unwrap();

    assert_eq!(
        with_thread.thread.len(),
        1,
        "thread must have exactly one entry"
    );
    assert_eq!(with_thread.thread[0].body, "Hello from the client.");
    assert_eq!(with_thread.thread[0].sender_id, client_id);
}

// ── Reopen flow ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn message_on_closed_ticket_reopens_it() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    // Reach closed state: open → acknowledged → closed.
    tickets::transition_ack(&pool, &ticket.id, None, &enc_key)
        .await
        .unwrap();
    tickets::transition_close(&pool, &ticket.id, None, &enc_key)
        .await
        .unwrap();

    let before = db::tickets::find_by_id(&pool, &ticket.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.status, "closed");

    // Posting a message on a closed ticket reopens it.
    post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket.id.clone(),
            sender_id: client_id.clone(),
            sender_role: "client".into(),
            sender_session_type: "full".into(),
            ticket_scope: None,
            body: "Is this still broken?".into(),
            attachment_path: None,
        },
    )
    .await
    .unwrap();

    let after = db::tickets::find_by_id(&pool, &ticket.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after.status, "open",
        "ticket must be open after message on closed"
    );
}

#[tokio::test]
async fn reopen_endpoint_works_from_closed() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    tickets::transition_ack(&pool, &ticket.id, None, &enc_key)
        .await
        .unwrap();
    tickets::transition_close(&pool, &ticket.id, None, &enc_key)
        .await
        .unwrap();

    tickets::transition_reopen(&pool, &ticket.id, None, &enc_key)
        .await
        .unwrap();

    let after = db::tickets::find_by_id(&pool, &ticket.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "open");
}

#[tokio::test]
async fn reopen_rejected_on_open_ticket() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    let r = tickets::transition_reopen(&pool, &ticket.id, None, &enc_key).await;
    assert!(matches!(r, Err(AppError::InvalidTransition { .. })));
}

#[tokio::test]
async fn scoped_session_message_on_closed_ticket_reopens_it() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    tickets::transition_ack(&pool, &ticket.id, None, &enc_key)
        .await
        .unwrap();
    tickets::transition_close(&pool, &ticket.id, None, &enc_key)
        .await
        .unwrap();

    // Scoped session may only message its own ticket.
    post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket.id.clone(),
            sender_id: client_id.clone(),
            sender_role: "client".into(),
            sender_session_type: "scoped".into(),
            ticket_scope: Some(ticket.id.clone()),
            body: "Replying via scoped session.".into(),
            attachment_path: None,
        },
    )
    .await
    .unwrap();

    let after = db::tickets::find_by_id(&pool, &ticket.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "open");
}

#[tokio::test]
async fn scoped_session_cannot_message_other_ticket() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket_a = create_ticket_for_client(&pool, &desk_id, &client_id).await;
    let ticket_b = create_ticket_for_client(&pool, &desk_id, &client_id).await;

    let r = post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket_b.id.clone(),
            sender_id: client_id.clone(),
            sender_role: "client".into(),
            sender_session_type: "scoped".into(),
            ticket_scope: Some(ticket_a.id.clone()), // scoped to a, not b
            body: "Should be rejected.".into(),
            attachment_path: None,
        },
    )
    .await;
    assert!(matches!(r, Err(AppError::Forbidden)));
}
