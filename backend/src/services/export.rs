use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    crypto::{self, EncryptionKey},
    db,
    error::{AppError, AppResult},
    models::ClientExport,
};

#[derive(Serialize)]
struct ExportTicket {
    id: String,
    title: String,
    description: String,
    status: String,
    priority: String,
    category: Option<String>,
    ticket_type: String,
    created_at: String,
    deleted_at: Option<String>,
    thread: Vec<ExportEntry>,
}

#[derive(Serialize)]
struct ExportEntry {
    id: String,
    sender_id: String,
    body: String,
    attachment_path: Option<String>,
    created_at: String,
}

#[derive(Serialize)]
struct ExportInvoice {
    id: String,
    number: String,
    status: String,
    currency: String,
    terms: String,
    issued_date: String,
    due_date: String,
    project_type: String,
    project_location: String,
    billed_to_name: String,
    billed_to_role: String,
    billed_to_addr1: String,
    billed_to_addr2: String,
    billed_to_pin: String,
    billed_to_email: String,
    billed_to_phone: String,
    kra_number: Option<String>,
    editor_note: String,
    items: serde_json::Value,
    notes: serde_json::Value,
    created_at: String,
}

#[derive(Serialize)]
struct ExportDocument {
    client_id: String,
    client_name: String,
    client_email: String,
    client_created_at: String,
    exported_at: String,
    tickets: Vec<ExportTicket>,
    invoices: Vec<ExportInvoice>,
}

pub struct ExportOutput {
    pub record: ClientExport,
    pub file_path: String,
}

pub async fn export_client(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    client_id: &str,
) -> AppResult<ExportOutput> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    let tickets = db::tickets::list_all_for_client(pool, client_id).await?;
    let mut export_tickets = Vec::with_capacity(tickets.len());

    for ticket in &tickets {
        let raw_entries = db::thread::list_for_ticket(pool, &ticket.id).await?;
        let mut thread = Vec::with_capacity(raw_entries.len());

        for entry in raw_entries {
            let body = if entry.body.is_empty() {
                String::new()
            } else if let Some(nonce) = &entry.body_nonce {
                crypto::decrypt(enc_key, nonce, &entry.body).unwrap_or_else(|_| {
                    tracing::warn!(
                        entry_id = %entry.id,
                        ticket_id = %entry.ticket_id,
                        "decryption failed during export — entry body replaced with placeholder"
                    );
                    "[decryption failed]".to_owned()
                })
            } else {
                entry.body.clone()
            };

            thread.push(ExportEntry {
                id: entry.id,
                sender_id: entry.sender_id,
                body,
                attachment_path: entry.attachment_path,
                created_at: entry.created_at,
            });
        }

        export_tickets.push(ExportTicket {
            id: ticket.id.clone(),
            title: ticket.title.clone(),
            description: ticket.description.clone(),
            status: ticket.status.clone(),
            priority: ticket.priority.clone(),
            category: ticket.category.clone(),
            ticket_type: ticket.ticket_type.clone(),
            created_at: ticket.created_at.clone(),
            deleted_at: ticket.deleted_at.clone(),
            thread,
        });
    }

    let client_email = if user.email.is_empty() {
        String::new()
    } else {
        crypto::decrypt(enc_key, &user.email_nonce, &user.email).unwrap_or_else(|_| {
            tracing::warn!(client_id = %client_id, "failed to decrypt email for export");
            "[encrypted]".to_owned()
        })
    };

    let raw_invoices = db::invoices::list_for_client(pool, client_id).await?;
    let export_invoices: Vec<ExportInvoice> = raw_invoices
        .into_iter()
        .map(|inv| ExportInvoice {
            id: inv.id,
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
            kra_number: inv.kra_number,
            editor_note: inv.editor_note,
            items: serde_json::from_str(&inv.items).unwrap_or(serde_json::Value::Array(vec![])),
            notes: serde_json::from_str(&inv.notes).unwrap_or(serde_json::Value::Array(vec![])),
            created_at: inv.created_at,
        })
        .collect();

    let exported_at = chrono::Utc::now().to_rfc3339();
    let doc = ExportDocument {
        client_id: client_id.to_owned(),
        client_name: user.name.clone(),
        client_email,
        client_created_at: user.created_at.clone(),
        exported_at: exported_at.clone(),
        tickets: export_tickets,
        invoices: export_invoices,
    };

    let json = tokio::task::spawn_blocking(move || {
        serde_json::to_string_pretty(&doc)
            .map_err(|e| AppError::Internal(format!("serialise export: {e}")))
    })
    .await
    .map_err(|e| AppError::Internal(format!("export thread: {e}")))??;

    let dir = format!("exports/{client_id}");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::Internal(format!("create export dir: {e}")))?;

    let filename = format!("{}.json", Uuid::new_v4());
    let file_path = format!("{dir}/{filename}");

    tokio::fs::write(&file_path, &json)
        .await
        .map_err(|e| AppError::Internal(format!("write export: {e}")))?;

    let export_id = Uuid::new_v4().to_string();
    let record = db::exports::create(pool, &export_id, client_id, &file_path).await?;

    Ok(ExportOutput { record, file_path })
}
