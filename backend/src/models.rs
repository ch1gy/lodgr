/// Database row types. NEVER derive Serialize — map to a DTO first.
use serde::Deserialize;

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)] // all columns selected for FromRow mapping; not every field is read in app code
pub struct User {
    pub id: String,
    pub name: String,
    /// AES-256-GCM ciphertext hex — decrypt before exposing to callers.
    pub email: String,
    pub email_nonce: String,
    /// Argon2id blind index for WHERE lookups — never decrypt, never expose.
    pub email_hash: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
    pub failed_attempts: i64,
    pub locked_until: Option<String>,
    #[sqlx(default)]
    pub address_line1: Option<String>,
    #[sqlx(default)]
    pub address_line1_nonce: Option<String>,
    #[sqlx(default)]
    pub address_line2: Option<String>,
    #[sqlx(default)]
    pub address_line2_nonce: Option<String>,
    #[sqlx(default)]
    pub pin_number: Option<String>,
    #[sqlx(default)]
    pub pin_number_nonce: Option<String>,
    #[sqlx(default)]
    pub contact_person: Option<String>,
    #[sqlx(default)]
    pub contact_person_nonce: Option<String>,
    #[sqlx(default)]
    pub phone: Option<String>,
    #[sqlx(default)]
    pub phone_nonce: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Ticket {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_by: String,
    pub client_id: String,
    pub created_at: String,
    pub priority: String,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub estimated_completion: Option<String>,
    pub ticket_type: String,
    pub recurring: i64,
    pub recurring_interval_days: Option<i64>,
    pub last_recurred_at: Option<String>,
    pub deleted_at: Option<String>,
    pub sub_client_id: Option<String>,
    /// Populated only when the query does a LEFT JOIN on sub_clients.
    #[sqlx(default)]
    pub sub_client_name: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubClient {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ThreadEntry {
    pub id: String,
    pub ticket_id: String,
    pub sender_id: String,
    pub body: String,
    pub body_nonce: Option<String>,
    pub attachment_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InternalNote {
    pub id: String,
    pub ticket_id: String,
    pub author_id: String,
    pub body: String,
    pub body_nonce: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub created_at: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub replaced_by: Option<String>,
    pub session_type: String,
    pub scoped_ticket_id: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct MagicLink {
    pub id: String,
    pub token_hash: String,
    pub user_id: String,
    pub scope: String,
    pub ticket_id: Option<String>,
    pub expires_at: String,
    pub used_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct ClientExport {
    pub id: String,
    pub client_id: String,
    pub file_path: String,
    pub created_at: String,
}

fn default_session_type() -> String {
    "full".to_owned()
}

/// JWT payload. Optional fields use `#[serde(default)]` so tokens issued
/// before those fields existed (and password-login tokens that never carry them)
/// still deserialise correctly.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: i64,
    #[serde(default = "default_session_type")]
    pub session_type: String,
    #[serde(default)]
    pub ticket_scope: Option<String>,
    /// JWT ID — present only on magic-link-issued tokens.
    /// Password-login access tokens are stateless and never carry this field.
    /// When Some, the AuthUser extractor verifies the jti against jwt_revocations.
    #[serde(default)]
    pub jti: Option<String>,
}

impl Claims {
    pub fn is_scoped(&self) -> bool {
        self.session_type == "scoped"
    }

    /// For a scoped session, verify the caller has access to `ticket_id`.
    pub fn check_ticket_access(&self, ticket_id: &str) -> crate::error::AppResult<()> {
        if self.is_scoped() {
            match &self.ticket_scope {
                Some(scope) if scope == ticket_id => Ok(()),
                _ => Err(crate::error::AppError::Forbidden),
            }
        } else {
            Ok(())
        }
    }

    pub fn require_full_session(&self) -> crate::error::AppResult<()> {
        if self.is_scoped() {
            Err(crate::error::AppError::Forbidden)
        } else {
            Ok(())
        }
    }
}

/// Raw invoice row from the DB. Items and notes are stored as JSON TEXT.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Invoice {
    pub id: String,
    /// NULL for issued invoices whose client was hard-deleted (retained for bookkeeping).
    pub client_id: Option<String>,
    pub number: String,
    pub status: String,
    pub currency: String,
    pub terms: String,
    pub issued_date: String,
    pub due_date: String,
    pub project_type: String,
    pub project_location: String,
    pub billed_to_name: String,
    pub billed_to_role: String,
    pub billed_to_addr1: String,
    pub billed_to_addr2: String,
    pub billed_to_pin: String,
    pub billed_to_email: String,
    pub billed_to_phone: String,
    pub items: String,
    pub notes: String,
    pub editor_note: String,
    pub kra_number: Option<String>,
    pub tax_rate: f64,
    pub recurring: i64,
    pub recur_interval: Option<String>,
    pub next_recur_date: Option<String>,
    pub created_at: String,
    pub sub_client_id: Option<String>,
    /// Populated only when the query does a LEFT JOIN on sub_clients.
    #[sqlx(default)]
    pub sub_client_name: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthEvent {
    pub id: String,
    pub user_id: String,
    pub event_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeskProfile {
    pub id: String,
    pub name: String,
    pub tagline: String,
    pub email: String,
    pub website: String,
    pub city: String,
    pub phone: String,
    pub vat_number: String,
}
