/// All API-facing response types. DB models must NEVER derive Serialize.
use serde::{Deserialize, Serialize};

use crate::models::{
    AuthEvent, ClientExport, DeskProfile, InternalNote, Invoice, Session, SubClient, ThreadEntry,
    Ticket, User,
};

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

// ── Auth events ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AuthEventResponse {
    pub id: String,
    pub event_type: String,
    pub created_at: String,
}

impl From<AuthEvent> for AuthEventResponse {
    fn from(e: AuthEvent) -> Self {
        AuthEventResponse {
            id: e.id,
            event_type: e.event_type,
            created_at: e.created_at,
        }
    }
}

// ── Sessions ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub created_at: String,
    pub expires_at: String,
    pub session_type: String,
}

impl From<Session> for SessionResponse {
    fn from(s: Session) -> Self {
        SessionResponse {
            id: s.id,
            created_at: s.created_at,
            expires_at: s.expires_at,
            session_type: s.session_type,
        }
    }
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

// ── Sub-clients ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SubClientResponse {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub created_at: String,
}

impl From<SubClient> for SubClientResponse {
    fn from(s: SubClient) -> Self {
        SubClientResponse {
            id: s.id,
            client_id: s.client_id,
            name: s.name,
            created_at: s.created_at,
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
    pub failed_attempts: i64,
    pub locked_until: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub pin_number: Option<String>,
    pub contact_person: Option<String>,
    pub phone: Option<String>,
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
            address_line1: u.address_line1,
            address_line2: u.address_line2,
            pin_number: u.pin_number,
            contact_person: u.contact_person,
            phone: u.phone,
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
                    .next_back()
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
    pub sub_client_id: Option<String>,
    pub sub_client_name: Option<String>,
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
            sub_client_id: t.sub_client_id,
            sub_client_name: t.sub_client_name,
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

// ── Invoices ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    pub qty: u32,
    pub rate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceNote {
    pub k: String,
    pub v: String,
}

#[derive(Debug, Serialize)]
pub struct InvoiceResponse {
    pub id: String,
    /// NULL when the client has been hard-deleted (invoice retained for bookkeeping).
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
    pub items: Vec<InvoiceItem>,
    pub notes: Vec<InvoiceNote>,
    pub editor_note: String,
    pub kra_number: Option<String>,
    pub tax_rate: f64,
    pub recurring: bool,
    pub recur_interval: Option<String>,
    pub next_recur_date: Option<String>,
    pub created_at: String,
    pub sub_client_id: Option<String>,
    pub sub_client_name: Option<String>,
}

impl From<Invoice> for InvoiceResponse {
    fn from(inv: Invoice) -> Self {
        let items: Vec<InvoiceItem> = serde_json::from_str(&inv.items).unwrap_or_default();
        let notes: Vec<InvoiceNote> = serde_json::from_str(&inv.notes).unwrap_or_default();
        InvoiceResponse {
            id: inv.id,
            client_id: inv.client_id,
            number: inv.number,
            status: inv.status,
            currency: inv.currency,
            terms: inv.terms,
            issued_date: inv.issued_date,
            due_date: inv.due_date,
            project_type: inv.project_type,
            project_location: inv.project_location,
            billed_to_name: inv.billed_to_name,
            billed_to_role: inv.billed_to_role,
            billed_to_addr1: inv.billed_to_addr1,
            billed_to_addr2: inv.billed_to_addr2,
            billed_to_pin: inv.billed_to_pin,
            billed_to_email: inv.billed_to_email,
            billed_to_phone: inv.billed_to_phone,
            items,
            notes,
            editor_note: inv.editor_note,
            kra_number: inv.kra_number,
            tax_rate: inv.tax_rate,
            recurring: inv.recurring != 0,
            recur_interval: inv.recur_interval,
            next_recur_date: inv.next_recur_date,
            created_at: inv.created_at,
            sub_client_id: inv.sub_client_id,
            sub_client_name: inv.sub_client_name,
        }
    }
}

// ── Desk profile ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DeskProfileResponse {
    pub name: String,
    pub tagline: String,
    pub email: String,
    pub website: String,
    pub city: String,
    pub phone: String,
    pub vat_number: String,
}

impl From<DeskProfile> for DeskProfileResponse {
    fn from(p: DeskProfile) -> Self {
        DeskProfileResponse {
            name: p.name,
            tagline: p.tagline,
            email: p.email,
            website: p.website,
            city: p.city,
            phone: p.phone,
            vat_number: p.vat_number,
        }
    }
}
