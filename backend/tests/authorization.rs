// Negative-authorization tests â€” every check here is an attempt by the *wrong*
// principal to access data. The correct result is always rejection (404 or
// Forbidden), never a successful read or a privilege escalation.
//
// Role set is currently DB-constrained to ('desk', 'client') via the CHECK
// constraint in migrations/001_init.sql, so the "unknown role" cases below
// cannot be triggered with a validly-issued JWT today. They are regression
// guards against future role additions â€” if a new role is ever added, these
// tests will fail immediately, forcing an explicit authorization decision
// rather than silent fail-open.

mod common;

use backend::{
    error::AppError,
    models::Claims,
    services::{
        messages::{post_message, PostMessageInput},
        tickets::{self, CreateTicketInput},
    },
};
use chrono::Utc;

// â”€â”€ Claim builders â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn claims_for(user_id: &str, role: &str) -> Claims {
    Claims {
        sub: user_id.to_owned(),
        role: role.to_owned(),
        exp: (Utc::now() + chrono::Duration::hours(1)).timestamp(),
        session_type: "full".to_owned(),
        ticket_scope: None,
        jti: None,
    }
}

fn client(id: &str) -> Claims {
    claims_for(id, "client")
}
fn desk(id: &str) -> Claims {
    claims_for(id, "desk")
}
/// Simulates a hypothetical future role. The DB currently rejects any role
/// outside ('desk', 'client'), so this cannot appear in a real JWT today.
fn unknown_role(id: &str) -> Claims {
    claims_for(id, "superadmin")
}

// â”€â”€ Shared ticket-creation helper â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

async fn make_ticket(
    pool: &sqlx::SqlitePool,
    desk_id: &str,
    client_id: &str,
) -> backend::models::Ticket {
    tickets::create(
        pool,
        None,
        &common::test_enc_key(),
        &desk(desk_id),
        CreateTicketInput {
            title: "Auth test ticket",
            description: "desc",
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

// â”€â”€ get_with_thread â€” IDOR â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn client_cannot_read_another_clients_ticket() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let (owner_id, _, _) = common::create_test_client(&pool).await;
    let (attacker_id, _, _) = common::create_test_client(&pool).await;

    let ticket = make_ticket(&pool, &desk_id, &owner_id).await;

    let result = tickets::get_with_thread(&pool, &ticket.id, &client(&attacker_id), &enc_key).await;

    assert!(
        matches!(result, Err(AppError::NotFound)),
        "client accessing another client's ticket must return NotFound, got: {:?}",
        result
    );
}

#[tokio::test]
async fn future_role_addition_falls_through_to_ownership_check_on_read() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let (owner_id, _, _) = common::create_test_client(&pool).await;
    let ticket = make_ticket(&pool, &desk_id, &owner_id).await;

    // An unrecognised role must be treated the same as a client (ownership-checked),
    // not as a desk (full access).
    let result =
        tickets::get_with_thread(&pool, &ticket.id, &unknown_role(&owner_id), &enc_key).await;

    // Ownership matches (same sub) so this should succeed â€” the important thing
    // is that *different* sub would be denied.  Assert here that the unknown role
    // does NOT silently get desk-level access to a ticket it doesn't own.
    let (attacker_id, _, _) = common::create_test_client(&pool).await;
    let denied =
        tickets::get_with_thread(&pool, &ticket.id, &unknown_role(&attacker_id), &enc_key).await;
    assert!(
        matches!(denied, Err(AppError::NotFound)),
        "unknown role accessing another user's ticket must be denied, got: {:?}",
        denied
    );
    // Suppress unused binding
    let _ = result;
}

#[tokio::test]
async fn desk_can_read_any_ticket() {
    // Sanity check that the inversion didn't break desk access.
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let ticket = make_ticket(&pool, &desk_id, &client_id).await;

    let result = tickets::get_with_thread(&pool, &ticket.id, &desk(&desk_id), &enc_key).await;
    assert!(
        result.is_ok(),
        "desk must be able to read any ticket, got: {:?}",
        result
    );
}

// â”€â”€ ticket create â€” client_id override attempt â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn client_filing_ticket_with_another_clients_id_is_filed_as_self() {
    let (pool, _dir) = common::setup_test_db().await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let (client_a, _, _) = common::create_test_client(&pool).await;
    let (client_b, _, _) = common::create_test_client(&pool).await;

    // client_a tries to file a ticket that names client_b as the owner.
    let ticket = tickets::create(
        &pool,
        None,
        &common::test_enc_key(),
        &client(&client_a),
        CreateTicketInput {
            title: "Impersonation attempt",
            description: "desc",
            priority: "medium",
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard",
            recurring: false,
            recurring_interval_days: None,
            client_id: Some(&client_b), // attempted override
            sub_client_id: None,
            initial_status: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        ticket.client_id, client_a,
        "ticket must be filed as the calling client (client_a), not the supplied client_id"
    );
    let _ = desk_id; // suppress unused
}

// â”€â”€ post_message â€” IDOR â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn client_cannot_post_message_to_another_clients_ticket() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let (owner_id, _, _) = common::create_test_client(&pool).await;
    let (attacker_id, _, _) = common::create_test_client(&pool).await;

    let ticket = make_ticket(&pool, &desk_id, &owner_id).await;

    let result = post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket.id.clone(),
            sender_id: attacker_id.clone(),
            sender_role: "client".to_owned(),
            sender_session_type: "full".to_owned(),
            ticket_scope: None,
            body: "I shouldn't be here".to_owned(),
            attachment_path: None,
        },
    )
    .await;

    assert!(
        matches!(result, Err(AppError::Forbidden)),
        "client posting to another client's ticket must return Forbidden, got: {:?}",
        result
    );
}

#[tokio::test]
async fn future_role_addition_falls_through_to_ownership_check_on_post_message() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let (owner_id, _, _) = common::create_test_client(&pool).await;
    let (attacker_id, _, _) = common::create_test_client(&pool).await;

    let ticket = make_ticket(&pool, &desk_id, &owner_id).await;

    let result = post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket.id.clone(),
            sender_id: attacker_id.clone(),
            sender_role: "superadmin".to_owned(), // unknown role
            sender_session_type: "full".to_owned(),
            ticket_scope: None,
            body: "Unknown role should not get through".to_owned(),
            attachment_path: None,
        },
    )
    .await;

    assert!(
        matches!(result, Err(AppError::Forbidden)),
        "unknown role posting to another client's ticket must return Forbidden, got: {:?}",
        result
    );
}

// â”€â”€ desk still has full access â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[tokio::test]
async fn desk_can_post_message_to_any_ticket() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let ticket = make_ticket(&pool, &desk_id, &client_id).await;

    let result = post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket.id.clone(),
            sender_id: desk_id.clone(),
            sender_role: "desk".to_owned(),
            sender_session_type: "full".to_owned(),
            ticket_scope: None,
            body: "Desk message".to_owned(),
            attachment_path: None,
        },
    )
    .await;

    assert!(
        result.is_ok(),
        "desk must be able to post to any ticket, got: {:?}",
        result
    );
}


