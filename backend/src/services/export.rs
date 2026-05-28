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
struct ExportDocument {
    client_id: String,
    exported_at: String,
    tickets: Vec<ExportTicket>,
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
            let body = if let Some(nonce) = &entry.body_nonce {
                crypto::decrypt(enc_key, nonce, &entry.body).unwrap_or_else(|_| {
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

    let exported_at = chrono::Utc::now().to_rfc3339();
    let doc = ExportDocument {
        client_id: client_id.to_owned(),
        exported_at: exported_at.clone(),
        tickets: export_tickets,
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
    let record =
        db::exports::create(pool, &export_id, client_id, &file_path).await?;

    Ok(ExportOutput { record, file_path })
}
