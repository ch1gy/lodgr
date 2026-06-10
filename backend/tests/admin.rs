mod common;

use backend::{
    db,
    error::AppError,
    services::{
        admin::{self, UpdateClientProfileInput},
        messages::{post_message, PostMessageInput},
        notes,
    },
};

// ── Create client ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_client_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();

    let user = admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "Alice Test".into(),
        "alice@example.com".into(),
        "SecurePass123!".into(),
        None,
    )
    .await
    .unwrap();

    assert_eq!(user.name, "Alice Test");
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.role, "client");
}

#[tokio::test]
async fn create_client_fails_with_duplicate_email() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();

    admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "First".into(),
        "dup@example.com".into(),
        "Pass111111!".into(),
        None,
    )
    .await
    .unwrap();

    let r = admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "Second".into(),
        "dup@example.com".into(),
        "Pass222222!".into(),
        None,
    )
    .await;
    assert!(
        matches!(r, Err(AppError::Conflict(_))),
        "expected Conflict for duplicate email, got: {:?}",
        r
    );
}

#[tokio::test]
async fn create_client_fails_with_invalid_email() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();

    let r = admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "Test".into(),
        "notanemail".into(),
        "ValidPass123!".into(),
        None,
    )
    .await;
    assert!(
        matches!(r, Err(AppError::BadRequest(_))),
        "expected BadRequest for invalid email, got: {:?}",
        r
    );
}

// ── Soft delete & restore ─────────────────────────────────────────────────────

#[tokio::test]
async fn soft_delete_client_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;

    admin::soft_delete_client(&pool, &client_id).await.unwrap();

    let user = db::users::find_by_id(&pool, &client_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        user.deleted_at.is_some(),
        "deleted_at must be set after soft delete"
    );
}

#[tokio::test]
async fn restore_client_within_30_days_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_id, _, _) = common::create_test_client(&pool).await;

    admin::soft_delete_client(&pool, &client_id).await.unwrap();
    admin::restore_client(&pool, &client_id).await.unwrap();

    let user = db::users::find_by_id(&pool, &client_id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        user.deleted_at.is_none(),
        "deleted_at must be None after restore"
    );
}

// ── Hard delete ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn hard_delete_requires_export_fails_without_one() {
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

#[tokio::test]
async fn hard_delete_succeeds_after_export_created() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, email, _) = common::create_test_client(&pool).await;
    let confirm = format!("permanently delete {email}");

    // Create a pre-deletion export.
    admin::do_export(&pool, &enc_key, &client_id).await.unwrap();

    // Hard delete must now succeed.
    admin::hard_delete_client(&pool, &enc_key, &client_id, &confirm)
        .await
        .unwrap();

    // User must be gone from the DB.
    let gone = db::users::find_by_id(&pool, &client_id).await.unwrap();
    assert!(gone.is_none(), "user must not exist after hard delete");

    // Clean up export files created on disk.
    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();
}

// ── Profile update ────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_client_profile_name_change_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let updated = admin::update_client_profile(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        &client_id,
        UpdateClientProfileInput {
            name: Some("New Name".into()),
            email: None,
            address_line1: None,
            address_line2: None,
            pin_number: None,
            contact_person: None,
            phone: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.name, "New Name");
}

#[tokio::test]
async fn update_client_profile_email_to_duplicate_fails() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();
    let (client_a, _, _) = common::create_test_client(&pool).await;
    let (client_b, email_b, _) = common::create_test_client(&pool).await;

    let r = admin::update_client_profile(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        &client_a,
        UpdateClientProfileInput {
            name: None,
            email: Some(email_b),
            address_line1: None,
            address_line2: None,
            pin_number: None,
            contact_person: None,
            phone: None,
        },
    )
    .await;
    assert!(
        matches!(r, Err(AppError::Conflict(_))),
        "expected Conflict when updating to an email already in use, got: {:?}",
        r
    );

    let _ = client_b; // suppress unused warning
}

// ── T2 — partial update must preserve all untouched PII fields ────────────────

#[tokio::test]
async fn update_profile_touching_only_name_preserves_pii_fields() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();

    let client = admin::create_client(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        "Alice".into(),
        "alice-pii@test.example".into(),
        "SecurePass123!".into(),
        Some(admin::NewClientProfile {
            address_line1: Some("123 Main St".into()),
            address_line2: Some("Apt 4".into()),
            pin_number: Some("SECRET-PIN".into()),
            contact_person: Some("Bob".into()),
            phone: Some("+1-555-0100".into()),
        }),
    )
    .await
    .unwrap();

    let updated = admin::update_client_profile(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        &client.id,
        UpdateClientProfileInput {
            name: Some("Alice Updated".into()),
            email: None,
            address_line1: None,
            address_line2: None,
            pin_number: None,
            contact_person: None,
            phone: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.name, "Alice Updated");
    assert_eq!(
        updated.address_line1.as_deref(),
        Some("123 Main St"),
        "address_line1 must be preserved"
    );
    assert_eq!(
        updated.address_line2.as_deref(),
        Some("Apt 4"),
        "address_line2 must be preserved"
    );
    assert_eq!(
        updated.pin_number.as_deref(),
        Some("SECRET-PIN"),
        "pin_number must be preserved"
    );
    assert_eq!(
        updated.contact_person.as_deref(),
        Some("Bob"),
        "contact_person must be preserved"
    );
    assert_eq!(
        updated.phone.as_deref(),
        Some("+1-555-0100"),
        "phone must be preserved"
    );
}

// ── T3 — email update rotates blind index ─────────────────────────────────────

#[tokio::test]
async fn email_update_rotates_blind_index_and_old_hash_no_longer_finds_user() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let config = common::test_config();
    let (client_id, old_email, _) = common::create_test_client(&pool).await;
    let new_email = format!("updated-{}", old_email);

    let old_hash = backend::crypto::hash_email(&old_email, &config.email_hash_salt).unwrap();

    admin::update_client_profile(
        &pool,
        &enc_key,
        &config.email_hash_salt,
        &client_id,
        UpdateClientProfileInput {
            name: None,
            email: Some(new_email.clone()),
            address_line1: None,
            address_line2: None,
            pin_number: None,
            contact_person: None,
            phone: None,
        },
    )
    .await
    .unwrap();

    // Old hash must no longer find the user.
    let by_old = db::users::find_by_email_hash(&pool, &old_hash)
        .await
        .unwrap();
    assert!(
        by_old.is_none(),
        "old email hash must not find user after email update"
    );

    // New hash must find the user.
    let new_hash = backend::crypto::hash_email(&new_email, &config.email_hash_salt).unwrap();
    let by_new = db::users::find_by_email_hash(&pool, &new_hash)
        .await
        .unwrap();
    assert!(
        by_new.is_some(),
        "new email hash must find user after email update"
    );
}

// ── Export content ────────────────────────────────────────────────────────────

#[tokio::test]
async fn export_includes_client_profile_fields() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let out = admin::do_export(&pool, &enc_key, &client_id).await.unwrap();
    let content = tokio::fs::read_to_string(&out.file_path).await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(
        !json["client_name"].is_null(),
        "export must include client_name"
    );
    assert!(
        !json["client_email"].is_null(),
        "export must include client_email"
    );
    assert!(
        !json["client_created_at"].is_null(),
        "export must include client_created_at"
    );

    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();
}

#[tokio::test]
async fn export_includes_all_ticket_content() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    // Insert a ticket directly for this client.
    let ticket_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, created_at,
          priority, ticket_type, recurring)
         VALUES (?, 'Export Test Ticket', 'desc', 'open', ?, ?, ?, 'medium', 'standard', 0)",
    )
    .bind(&ticket_id)
    .bind(&client_id)
    .bind(&client_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    let out = admin::do_export(&pool, &enc_key, &client_id).await.unwrap();
    let content = tokio::fs::read_to_string(&out.file_path).await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let tickets = json["tickets"].as_array().unwrap();
    assert!(!tickets.is_empty(), "export must include tickets");
    assert_eq!(tickets[0]["title"], "Export Test Ticket");

    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();
}

// ── Hard-delete orphan check ──────────────────────────────────────────────────

#[tokio::test]
async fn hard_delete_leaves_no_orphaned_rows_in_any_child_table() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, email, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    // Build a full data graph: ticket + message + internal note + magic-link JTI.
    let ticket_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, created_at,
          priority, ticket_type, recurring)
         VALUES (?, 'Orphan test', 'desc', 'open', ?, ?, ?, 'medium', 'standard', 0)",
    )
    .bind(&ticket_id)
    .bind(&desk_id)
    .bind(&client_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    post_message(
        &pool,
        &enc_key,
        None,
        PostMessageInput {
            ticket_id: ticket_id.clone(),
            sender_id: client_id.clone(),
            sender_role: "client".into(),
            sender_session_type: "full".into(),
            ticket_scope: None,
            body: "Message".into(),
            attachment_path: None,
        },
    )
    .await
    .unwrap();

    notes::create_note(
        &pool,
        &enc_key,
        &ticket_id,
        &desk_id,
        "Internal note.".into(),
    )
    .await
    .unwrap();

    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    db::jwt_revocations::create(&pool, "orphan-jti", &client_id, &expires_at)
        .await
        .unwrap();
    // Plant an auth event so the cascade assertion is meaningful.
    backend::db::auth_events::create(&pool, &client_id, "login_ok")
        .await
        .unwrap();

    // Export then hard-delete.
    admin::do_export(&pool, &enc_key, &client_id).await.unwrap();
    admin::hard_delete_client(
        &pool,
        &enc_key,
        &client_id,
        &format!("permanently delete {email}"),
    )
    .await
    .unwrap();
    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();

    // Every child table must be clean — no orphaned rows anywhere.
    macro_rules! count {
        ($q:expr, $id:expr) => {{
            sqlx::query_scalar::<_, i64>($q)
                .bind($id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }};
    }

    assert_eq!(
        count!(
            "SELECT COUNT(*) FROM tickets WHERE client_id = ?",
            &client_id
        ),
        0,
        "tickets"
    );
    assert_eq!(
        count!(
            "SELECT COUNT(*) FROM thread_entries WHERE ticket_id = ?",
            &ticket_id
        ),
        0,
        "thread_entries"
    );
    assert_eq!(
        count!(
            "SELECT COUNT(*) FROM internal_notes WHERE ticket_id = ?",
            &ticket_id
        ),
        0,
        "internal_notes"
    );
    assert_eq!(
        count!(
            "SELECT COUNT(*) FROM magic_links WHERE user_id = ?",
            &client_id
        ),
        0,
        "magic_links"
    );
    assert_eq!(
        count!(
            "SELECT COUNT(*) FROM sessions WHERE user_id = ?",
            &client_id
        ),
        0,
        "sessions"
    );
    assert_eq!(
        count!(
            "SELECT COUNT(*) FROM jwt_revocations WHERE user_id = ?",
            &client_id
        ),
        0,
        "jwt_revocations"
    );
    assert_eq!(
        count!(
            "SELECT COUNT(*) FROM auth_events WHERE user_id = ?",
            &client_id
        ),
        0,
        "auth_events"
    );
    assert!(
        db::users::find_by_id(&pool, &client_id)
            .await
            .unwrap()
            .is_none(),
        "user must not exist after hard delete"
    );
}

// ── Invoice + sub_client cascade tests ───────────────────────────────────────

async fn insert_invoice(
    pool: &sqlx::SqlitePool,
    client_id: &str,
    status: &str,
    recurring: bool,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO invoices
         (id, client_id, number, status, currency, terms, issued_date, due_date,
          project_type, project_location, billed_to_name, billed_to_role,
          billed_to_addr1, billed_to_addr2, billed_to_pin, billed_to_email, billed_to_phone,
          items, notes, editor_note, recurring, recur_interval, next_recur_date, created_at)
         VALUES (?, ?, ?, ?, 'KES', 'Net 14', '2026-01-01', '2026-01-15',
                 '', '', 'Test Client', 'Test Contact', '', '', '', '', '',
                 '[]', '[]', '', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(client_id)
    .bind(format!("INV-{id}"))
    .bind(status)
    .bind(recurring as i64)
    .bind(if recurring {
        Some("monthly")
    } else {
        None::<&str>
    })
    .bind(if recurring {
        Some("2026-01-01")
    } else {
        None::<&str>
    })
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn insert_sub_client(pool: &sqlx::SqlitePool, client_id: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sub_clients (id, client_id, name, created_at) VALUES (?, ?, 'Sub', ?)",
    )
    .bind(&id)
    .bind(client_id)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn hard_delete_removes_draft_invoice_retains_sent() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, email, _) = common::create_test_client(&pool).await;

    let draft_id = insert_invoice(&pool, &client_id, "draft", false).await;
    // recurring = true so the UPDATE … SET recurring = 0 path is actually exercised.
    let sent_id = insert_invoice(&pool, &client_id, "sent", true).await;

    admin::do_export(&pool, &enc_key, &client_id).await.unwrap();
    admin::hard_delete_client(
        &pool,
        &enc_key,
        &client_id,
        &format!("permanently delete {email}"),
    )
    .await
    .unwrap();
    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();

    // Draft must be gone.
    let draft_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoices WHERE id = ?")
        .bind(&draft_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(draft_count, 0, "draft invoice must be deleted with client");

    // Sent invoice must survive with client_id NULL and all recurrence fields cleared.
    let row: (Option<String>, i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT client_id, recurring, recur_interval, next_recur_date FROM invoices WHERE id = ?",
    )
    .bind(&sent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        row.0.is_none(),
        "sent invoice client_id must be NULL after deletion"
    );
    assert_eq!(row.1, 0, "sent invoice recurring must be disabled");
    assert!(
        row.2.is_none(),
        "recur_interval must be NULL after disabling recurrence"
    );
    assert!(
        row.3.is_none(),
        "next_recur_date must be NULL after disabling recurrence"
    );
}

#[tokio::test]
async fn hard_delete_with_sub_clients_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, email, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    let sub_id = insert_sub_client(&pool, &client_id).await;

    // Ticket tagged with the sub-client — cascade must handle the FK chain.
    let ticket_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, sub_client_id, created_at,
          priority, ticket_type, recurring)
         VALUES (?, 'Sub test', 'desc', 'open', ?, ?, ?, ?, 'medium', 'standard', 0)",
    )
    .bind(&ticket_id)
    .bind(&desk_id)
    .bind(&client_id)
    .bind(&sub_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    admin::do_export(&pool, &enc_key, &client_id).await.unwrap();
    admin::hard_delete_client(
        &pool,
        &enc_key,
        &client_id,
        &format!("permanently delete {email}"),
    )
    .await
    .unwrap();
    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();

    let sub_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sub_clients WHERE id = ?")
        .bind(&sub_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sub_count, 0, "sub_clients must be deleted with client");
}

#[tokio::test]
async fn export_contains_invoices() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;

    insert_invoice(&pool, &client_id, "sent", false).await;
    insert_invoice(&pool, &client_id, "draft", false).await;

    let output = admin::do_export(&pool, &enc_key, &client_id).await.unwrap();
    let raw = tokio::fs::read_to_string(&output.file_path).await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();

    let invoices = json["invoices"].as_array().unwrap();
    assert_eq!(
        invoices.len(),
        2,
        "export must include all invoices for the client"
    );
    assert!(
        invoices
            .iter()
            .all(|inv| inv["billed_to_role"].as_str() == Some("Test Contact")),
        "export must include billed_to_role for every invoice"
    );

    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();
}

#[tokio::test]
async fn orphan_check_extended_with_invoices_and_sub_clients() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, email, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;

    // Plant invoice + sub_client so the cascade is exercised.
    insert_invoice(&pool, &client_id, "draft", false).await;
    insert_invoice(&pool, &client_id, "sent", false).await;
    let sub_id = insert_sub_client(&pool, &client_id).await;

    // Ticket tagged with sub-client (FK chain: ticket → sub_client → user).
    let ticket_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, sub_client_id, created_at,
          priority, ticket_type, recurring)
         VALUES (?, 'Orphan ext', 'desc', 'open', ?, ?, ?, ?, 'medium', 'standard', 0)",
    )
    .bind(&ticket_id)
    .bind(&desk_id)
    .bind(&client_id)
    .bind(&sub_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .unwrap();

    admin::do_export(&pool, &enc_key, &client_id).await.unwrap();
    admin::hard_delete_client(
        &pool,
        &enc_key,
        &client_id,
        &format!("permanently delete {email}"),
    )
    .await
    .unwrap();
    tokio::fs::remove_dir_all(format!("exports/{client_id}"))
        .await
        .ok();

    // User is gone.
    assert!(
        db::users::find_by_id(&pool, &client_id)
            .await
            .unwrap()
            .is_none(),
        "user must not exist after hard delete"
    );
    // Sub-clients are gone.
    let sub_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sub_clients WHERE client_id = ?")
        .bind(&client_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sub_count, 0, "sub_clients must be gone");
    // Draft invoices are gone; sent invoices are orphaned (client_id = NULL).
    let draft_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices WHERE client_id = ? AND status = 'draft'",
    )
    .bind(&client_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(draft_count, 0, "draft invoices must be deleted");
    // The sent invoice survives but is orphaned.
    let sent_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices WHERE client_id IS NULL AND status = 'sent'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sent_count, 1, "sent invoice must survive as orphan");
}
