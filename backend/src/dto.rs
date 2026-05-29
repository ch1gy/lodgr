/// All API-facing response types. DB models must NEVER derive Serialize.
use serde::Serialize;

use crate::models::{ClientExport, InternalNote, ThreadEntry, Ticket, User};

// ── Auth ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
}

// ── Magic links ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MagicLinkResponse {
    /// Full URL the client can open — copyable for any delivery channel
    /// (email, WhatsApp, SMS, etc.).
    pub url: String,
}

// ── Auth / Me ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

impl From<User> for MeResponse {
    fn from(u: User) -> Self {
        MeResponse {
            id: u.id,
            name: u.name,
            email: u.email,
            role: u.role,
            created_at: u.created_at,
        }
    }
}

// ── Admin ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ClientResponse {
    pub id: String,
    pub name: String,
    pub email: String,
    pub deleted_at: Option<String>,
    /// Consecutive failed login attempts. Reset to 0 on successful login or
    /// magic link exchange.
    pub failed_attempts: i64,
    /// RFC-3339 timestamp the account is locked until. `None` = not locked.
    /// `"9999-01-01T00:00:00+00:00"` = permanent lockout.
    pub locked_until: Option<String>,
}

impl From<User> for ClientResponse {
    fn from(u: User) -> Self {
        ClientResponse {
            id: u.id,
            name: u.name,
            email: u.email,
            deleted_at: u.deleted_at,
            failed_attempts: u.failed_attempts,
            locked_until: u.locked_until,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DeleteSessionsResponse {
    pub deleted: u64,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub export_id: String,
    pub download_url: String,
}

impl From<ClientExport> for ExportResponse {
    fn from(e: ClientExport) -> Self {
        ExportResponse {
            export_id: e.id,
            download_url: format!(
                "/admin/exports/{}/{}",
                e.client_id,
                e.file_path
                    .split(['/', '\\'])
                    .last()
                    .unwrap_or("export.json")
            ),
        }
    }
}

// ── Paginated wrapper ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PaginatedTickets {
    pub tickets: Vec<TicketResponse>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
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
    pub priority: String,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub estimated_completion: Option<String>,
    pub ticket_type: String,
    pub recurring: bool,
    pub recurring_interval_days: Option<i64>,
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
            priority: t.priority,
            category: t.category,
            due_date: t.due_date,
            estimated_completion: t.estimated_completion,
            ticket_type: t.ticket_type,
            recurring: t.recurring != 0,
            recurring_interval_days: t.recurring_interval_days,
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

#[derive(Debug, Serialize)]
pub struct TicketWithThreadResponse {
    #[serde(flatten)]
    pub ticket: TicketResponse,
    pub thread: Vec<ThreadEntryResponse>,
}

// ── Internal notes ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct InternalNoteResponse {
    pub id: String,
    pub ticket_id: String,
    pub author_id: String,
    pub body: String,
    pub created_at: String,
}

impl From<InternalNote> for InternalNoteResponse {
    fn from(n: InternalNote) -> Self {
        InternalNoteResponse {
            id: n.id,
            ticket_id: n.ticket_id,
            author_id: n.author_id,
            body: n.body,
            created_at: n.created_at,
        }
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: String,
}
