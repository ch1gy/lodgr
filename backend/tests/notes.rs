mod common;

use backend::services::notes;

// ── Shared ticket setup ───────────────────────────────────────────────────────

async fn insert_ticket(pool: &sqlx::SqlitePool, desk_id: &str, client_id: &str) -> String {
    let ticket_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, created_at,
          priority, ticket_type, recurring)
         VALUES (?, 'Note test', 'desc', 'open', ?, ?, ?, 'medium', 'standard', 0)",
    )
    .bind(&ticket_id)
    .bind(desk_id)
    .bind(client_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();
    ticket_id
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn desk_can_create_note_and_list_it_back() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket_id = insert_ticket(&pool, &desk_id, &client_id).await;

    let note = notes::create_note(
        &pool,
        &enc_key,
        &ticket_id,
        &desk_id,
        "Desk-only observation.".into(),
    )
    .await
    .unwrap();

    // The returned note must contain the plaintext body.
    assert_eq!(note.body, "Desk-only observation.");
    assert_eq!(note.author_id, desk_id);
    assert_eq!(note.ticket_id, ticket_id);

    // list_notes must return the same note, decrypted.
    let list = notes::list_notes(&pool, &enc_key, &ticket_id)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].body, "Desk-only observation.");
}

#[tokio::test]
async fn note_body_is_encrypted_at_rest() {
    let (pool, _dir) = common::setup_test_db().await;
    let enc_key = common::test_enc_key();
    let (client_id, _, _) = common::create_test_client(&pool).await;
    let (desk_id, _, _) = common::create_test_desk(&pool).await;
    let ticket_id = insert_ticket(&pool, &desk_id, &client_id).await;

    let note = notes::create_note(
        &pool,
        &enc_key,
        &ticket_id,
        &desk_id,
        "Confidential note.".into(),
    )
    .await
    .unwrap();

    // Read the raw body column from the DB — it must not be the plaintext.
    let raw: (String,) = sqlx::query_as("SELECT body FROM internal_notes WHERE id = ?")
        .bind(&note.id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_ne!(
        raw.0, "Confidential note.",
        "note body must not be stored as plaintext"
    );
    // AES-256-GCM ciphertext is stored as lowercase hex.
    assert!(
        raw.0.chars().all(|c| c.is_ascii_hexdigit()),
        "raw body should be hex-encoded ciphertext, got: {}",
        &raw.0[..raw.0.len().min(40)]
    );
}
