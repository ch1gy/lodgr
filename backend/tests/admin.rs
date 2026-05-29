mod common;

use backend::{db, error::AppError, services::admin};

// ── Create client ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_client_succeeds() {
    let (pool, _dir) = common::setup_test_db().await;

    let user = admin::create_client(
        &pool,
        "Alice Test".into(),
        "alice@example.com".into(),
        "SecurePass123!".into(),
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

    admin::create_client(
        &pool,
        "First".into(),
        "dup@example.com".into(),
        "Pass111111!".into(),
    )
    .await
    .unwrap();

    let r = admin::create_client(
        &pool,
        "Second".into(),
        "dup@example.com".into(),
        "Pass222222!".into(),
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

    let r = admin::create_client(
        &pool,
        "Test".into(),
        "notanemail".into(),
        "ValidPass123!".into(),
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
    let (client_id, _, _) = common::create_test_client(&pool).await;

    let updated = admin::update_client_profile(&pool, &client_id, Some("New Name".into()), None)
        .await
        .unwrap();

    assert_eq!(updated.name, "New Name");
}

#[tokio::test]
async fn update_client_profile_email_to_duplicate_fails() {
    let (pool, _dir) = common::setup_test_db().await;
    let (client_a, _, _) = common::create_test_client(&pool).await;
    let (client_b, email_b, _) = common::create_test_client(&pool).await;

    let r = admin::update_client_profile(&pool, &client_a, None, Some(email_b)).await;
    assert!(
        matches!(r, Err(AppError::Conflict(_))),
        "expected Conflict when updating to an email already in use, got: {:?}",
        r
    );

    let _ = client_b; // suppress unused warning
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
