/// Database row types. These must NEVER derive `Serialize` — use `dto.rs`
/// for any type that is serialized into an API response.
use serde::Deserialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    /// Never serialize this field. It is a DB row type only.
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Ticket {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_by: String,
    pub client_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ThreadEntry {
    pub id: String,
    pub ticket_id: String,
    pub sender_id: String,
    /// Plaintext after decryption; ciphertext in the DB.
    pub body: String,
    /// AES-256-GCM nonce (hex). Never exposed in API responses.
    pub body_nonce: Option<String>,
    pub attachment_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Notification {
    pub id: String,
    pub ticket_id: String,
    pub recipient_id: String,
    pub message: String,
    pub sent_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub created_at: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub replaced_by: Option<String>,
}

/// JWT payload — must retain both Serialize and Deserialize for encoding/decoding.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: i64,
}
