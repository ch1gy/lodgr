/// All API-facing response types live here.
/// DB models (`models.rs`) must never derive `Serialize` — map to a DTO first.
use serde::Serialize;

use crate::models::{Ticket, ThreadEntry, User};

// ── Auth ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
}

// ── Admin ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ClientResponse {
    pub id: String,
    pub name: String,
    pub email: String,
}

impl From<User> for ClientResponse {
    fn from(u: User) -> Self {
        ClientResponse { id: u.id, name: u.name, email: u.email }
    }
}

#[derive(Debug, Serialize)]
pub struct DeleteSessionsResponse {
    pub deleted: u64,
}

// ── Tickets ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TicketResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_by: String,
    pub client_id: String,
    pub created_at: String,
}

impl From<Ticket> for TicketResponse {
    fn from(t: Ticket) -> Self {
        TicketResponse {
            id: t.id,
            title: t.title,
            description: t.description,
            status: t.status,
            created_by: t.created_by,
            client_id: t.client_id,
            created_at: t.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ThreadEntryResponse {
    pub id: String,
    pub ticket_id: String,
    pub sender_id: String,
    pub body: String,
    pub attachment_path: Option<String>,
    pub created_at: String,
}

impl From<ThreadEntry> for ThreadEntryResponse {
    fn from(e: ThreadEntry) -> Self {
        ThreadEntryResponse {
            id: e.id,
            ticket_id: e.ticket_id,
            sender_id: e.sender_id,
            body: e.body,
            attachment_path: e.attachment_path,
            created_at: e.created_at,
        }
    }
}

/// Flattened ticket + thread response; `ticket` fields appear at the top level.
#[derive(Debug, Serialize)]
pub struct TicketWithThreadResponse {
    #[serde(flatten)]
    pub ticket: TicketResponse,
    pub thread: Vec<ThreadEntryResponse>,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: String,
}
