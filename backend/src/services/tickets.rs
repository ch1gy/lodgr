use chrono::NaiveDate;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    crypto::{self, EncryptionKey},
    db,
    email::{SmtpMailer, TicketEvent},
    error::{AppError, AppResult},
    models::{Claims, ThreadEntry, Ticket, User},
    notify,
    ticket_status::{transition, TransitionAction},
};

#[derive(Debug)]
pub struct TicketWithThread {
    pub ticket: Ticket,
    pub thread: Vec<ThreadEntry>,
}

pub struct CreateTicketInput<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub priority: &'a str,
    pub category: Option<&'a str>,
    pub due_date: Option<&'a str>,
    pub estimated_completion: Option<&'a str>,
    pub ticket_type: &'a str,
    pub recurring: bool,
    pub recurring_interval_days: Option<i64>,
    /// Desk only: file on behalf of a specific client UUID.
    /// None → use the caller's own ID (clients always use self).
    pub client_id: Option<&'a str>,
    /// Desk only: override the initial status (defaults to 'open').
    pub initial_status: Option<&'a str>,
    /// Optional sub-client label (must belong to the ticket's client).
    pub sub_client_id: Option<&'a str>,
}

/// Returns (page of tickets, total count). page is 1-indexed; limit capped at 100.
pub async fn list(
    pool: &SqlitePool,
    claims: &Claims,
    page: u32,
    limit: u32,
) -> AppResult<(Vec<Ticket>, i64)> {
    let limit = limit.clamp(1, 100) as i64;
    let offset = ((page.max(1) - 1) as i64) * limit;

    if claims.role == "desk" {
        db::tickets::list_all_paginated(pool, limit, offset).await
    } else if claims.is_scoped() {
        if let Some(tid) = &claims.ticket_scope {
            // Scoped sessions see exactly one ticket.
            let t = db::tickets::find_by_id(pool, tid)
                .await?
                .ok_or(AppError::NotFound)?;
            Ok((vec![t], 1))
        } else {
            db::tickets::list_for_client_paginated(pool, &claims.sub, limit, offset).await
        }
    } else {
        db::tickets::list_for_client_paginated(pool, &claims.sub, limit, offset).await
    }
}

pub async fn create(
    pool: &SqlitePool,
    mailer: Option<&SmtpMailer>,
    claims: &Claims,
    input: CreateTicketInput<'_>,
) -> AppResult<Ticket> {
    claims.require_full_session()?;

    validate_priority(input.priority)?;
    validate_ticket_type(input.ticket_type)?;
    if let Some(cat) = input.category {
        if cat.len() > 100 {
            return Err(AppError::BadRequest(
                "category must be at most 100 characters".into(),
            ));
        }
    }
    if input.title.is_empty() || input.title.len() > 200 {
        return Err(AppError::BadRequest(
            "title must be 1–200 characters".into(),
        ));
    }
    if input.description.is_empty() || input.description.len() > 50_000 {
        return Err(AppError::BadRequest(
            "description must be 1–50,000 characters".into(),
        ));
    }
    if let Some(d) = input.due_date {
        validate_date_format(d)?;
    }
    if let Some(d) = input.estimated_completion {
        validate_date_format(d)?;
    }
    if input.recurring {
        match input.recurring_interval_days {
            Some(days) if (1..=365).contains(&days) => {}
            Some(_) => {
                return Err(AppError::BadRequest(
                    "recurring_interval_days must be between 1 and 365".into(),
                ))
            }
            None => {
                return Err(AppError::BadRequest(
                    "recurring_interval_days must be set when recurring is true".into(),
                ))
            }
        }
    }

    // Resolve client_id: desk may file on behalf of a client; everyone else
    // always files as themselves. Keep the fetched User to avoid a second
    // DB round-trip for the email notification below.
    let (resolved_client_id, prefetched_user): (String, Option<User>) = if claims.role == "desk" {
        match input.client_id {
            Some(cid) => {
                let client = db::users::find_by_id(pool, cid)
                    .await?
                    .ok_or_else(|| AppError::BadRequest("client not found".into()))?;
                if client.role != "client" {
                    return Err(AppError::BadRequest("target user is not a client".into()));
                }
                if client.deleted_at.is_some() {
                    return Err(AppError::BadRequest(
                        "cannot file a ticket for a deleted client".into(),
                    ));
                }
                (cid.to_owned(), Some(client))
            }
            None => (claims.sub.clone(), None),
        }
    } else {
        (claims.sub.clone(), None)
    };

    // Validate sub_client_id belongs to the resolved client (desk only).
    if let Some(sc_id) = input.sub_client_id {
        match db::sub_clients::find_by_id(pool, sc_id).await? {
            Some(sc) if sc.client_id == resolved_client_id => {}
            Some(_) => {
                return Err(AppError::BadRequest(
                    "sub_client does not belong to this client".into(),
                ))
            }
            None => return Err(AppError::BadRequest("sub_client not found".into())),
        }
    }

    let id = Uuid::new_v4().to_string();
    let ticket = db::tickets::create(
        pool,
        db::tickets::NewTicket {
            id: &id,
            title: input.title,
            description: input.description,
            created_by: &claims.sub,
            client_id: &resolved_client_id,
            priority: input.priority,
            category: input.category,
            due_date: input.due_date,
            estimated_completion: input.estimated_completion,
            ticket_type: input.ticket_type,
            recurring: input.recurring,
            recurring_interval_days: input.recurring_interval_days,
            sub_client_id: input.sub_client_id,
        },
    )
    .await?;

    // Desk may request a non-open initial status (e.g. 'closed' for logging
    // already-resolved work). Apply it immediately after creation.
    if claims.role == "desk" {
        if let Some(status) = input.initial_status {
            if status != "open" {
                if !["acknowledged", "pending", "closed"].contains(&status) {
                    return Err(AppError::BadRequest("invalid initial_status".into()));
                }
                db::tickets::update_status(pool, &ticket.id, status).await?;
            }
        }
    }

    notify::notify(
        &resolved_client_id,
        &ticket.id,
        "Your ticket has been created.",
    );

    // Fire-and-forget email — failure is non-fatal but logged.
    // Re-use the user we already fetched during desk-on-behalf-of-client
    // resolution; fall back to a fresh query for self-filed tickets.
    if let Some(m) = mailer {
        let user_result = if let Some(user) = prefetched_user {
            Ok(Some(user))
        } else {
            db::users::find_by_id(pool, &resolved_client_id).await
        };
        match user_result {
            Ok(Some(user)) => {
                spawn_ticket_notification_email(m, user, ticket.title.clone(), TicketEvent::Created)
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(%e, "failed to fetch user for ticket-created email"),
        }
    }

    Ok(ticket)
}

/// Enforce ticket ownership for any non-desk principal.
///
/// Rules (desk always passes):
///   • soft-deleted ticket → NotFound
///   • ticket.client_id != claims.sub → NotFound
///   • scoped session whose scope doesn't match ticket.id → Forbidden
///
/// Pure and synchronous — no I/O, no async. Both `get_with_thread` and
/// `get_attachment` delegate to this so the logic lives in exactly one place.
pub fn assert_ticket_access(claims: &Claims, ticket: &Ticket) -> AppResult<()> {
    if claims.role != "desk" {
        if ticket.deleted_at.is_some() {
            return Err(AppError::NotFound);
        }
        if ticket.client_id != claims.sub {
            return Err(AppError::NotFound);
        }
    }
    claims.check_ticket_access(&ticket.id)?;
    Ok(())
}

pub async fn get_with_thread(
    pool: &SqlitePool,
    ticket_id: &str,
    claims: &Claims,
    enc_key: &EncryptionKey,
) -> AppResult<TicketWithThread> {
    let ticket = db::tickets::find_by_id(pool, ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    assert_ticket_access(claims, &ticket)?;

    let encrypted_thread = db::thread::list_for_ticket(pool, &ticket.id).await?;

    let thread = encrypted_thread
        .into_iter()
        .map(|mut entry| {
            if entry.body.is_empty() {
                // Attachment-only message — no body to decrypt.
            } else {
                let nonce = entry.body_nonce.as_deref().ok_or_else(|| {
                    AppError::Internal(format!(
                        "thread entry {} missing nonce — pre-migration data",
                        entry.id
                    ))
                })?;
                entry.body = crypto::decrypt(enc_key, nonce, &entry.body)?;
            }
            Ok(entry)
        })
        .collect::<AppResult<Vec<ThreadEntry>>>()?;

    Ok(TicketWithThread { ticket, thread })
}

pub async fn update(
    pool: &SqlitePool,
    ticket_id: &str,
    f: db::tickets::UpdateFields<'_>,
) -> AppResult<()> {
    if let Some(p) = f.priority {
        validate_priority(p)?;
    }
    if let Some(t) = f.ticket_type {
        validate_ticket_type(t)?;
    }
    if let Some(Some(cat)) = f.category {
        if cat.len() > 100 {
            return Err(AppError::BadRequest(
                "category must be at most 100 characters".into(),
            ));
        }
    }
    if let Some(Some(d)) = f.due_date {
        validate_date_format(d)?;
    }
    if let Some(Some(d)) = f.estimated_completion {
        validate_date_format(d)?;
    }
    if let Some(Some(days)) = f.recurring_interval_days {
        if !(1..=365).contains(&days) {
            return Err(AppError::BadRequest(
                "recurring_interval_days must be between 1 and 365".into(),
            ));
        }
    }
    if db::tickets::find_by_id(pool, ticket_id).await?.is_none() {
        return Err(AppError::NotFound);
    }
    db::tickets::update_fields(pool, ticket_id, f).await
}

pub async fn transition_ack(
    pool: &SqlitePool,
    ticket_id: &str,
    mailer: Option<&SmtpMailer>,
) -> AppResult<()> {
    apply_transition(
        pool,
        ticket_id,
        TransitionAction::Acknowledge,
        TicketEvent::Acknowledged,
        "Your ticket has been acknowledged.",
        mailer,
    )
    .await
}

pub async fn transition_pend(
    pool: &SqlitePool,
    ticket_id: &str,
    mailer: Option<&SmtpMailer>,
) -> AppResult<()> {
    apply_transition(
        pool,
        ticket_id,
        TransitionAction::Pend,
        TicketEvent::Pending,
        "Your ticket is awaiting your response.",
        mailer,
    )
    .await
}

pub async fn transition_close(
    pool: &SqlitePool,
    ticket_id: &str,
    mailer: Option<&SmtpMailer>,
) -> AppResult<()> {
    apply_transition(
        pool,
        ticket_id,
        TransitionAction::Close,
        TicketEvent::Closed,
        "Your ticket has been closed.",
        mailer,
    )
    .await
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Spawn a fire-and-forget ticket notification email. Non-fatal: failures are
/// logged as warnings and never bubble up to the caller.
fn spawn_ticket_notification_email(
    mailer: &SmtpMailer,
    user: User,
    ticket_title: String,
    event: TicketEvent,
) {
    let m = mailer.clone();
    tokio::spawn(async move {
        m.send_ticket_notification(&user.email, &user.name, &ticket_title, event)
            .await;
    });
}

async fn apply_transition(
    pool: &SqlitePool,
    ticket_id: &str,
    action: TransitionAction,
    event: TicketEvent,
    notify_msg: &str,
    mailer: Option<&SmtpMailer>,
) -> AppResult<()> {
    let ticket = db::tickets::find_by_id(pool, ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let new_status = transition(&ticket.status, action)?;
    db::tickets::update_status(pool, ticket_id, new_status.as_str()).await?;
    notify::notify(&ticket.client_id, ticket_id, notify_msg);

    // Fire-and-forget email — failure is non-fatal but logged.
    if let Some(m) = mailer {
        match db::users::find_by_id(pool, &ticket.client_id).await {
            Ok(Some(user)) => spawn_ticket_notification_email(m, user, ticket.title.clone(), event),
            Ok(None) => {}
            Err(e) => tracing::warn!(%e, "failed to fetch client for transition email"),
        }
    }

    Ok(())
}

fn validate_date_format(d: &str) -> AppResult<()> {
    if NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
        return Err(AppError::BadRequest(
            "dates must be in YYYY-MM-DD format".into(),
        ));
    }
    Ok(())
}

fn validate_priority(p: &str) -> AppResult<()> {
    match p {
        "low" | "medium" | "high" | "urgent" => Ok(()),
        _ => Err(AppError::BadRequest(
            "priority must be one of: low, medium, high, urgent".into(),
        )),
    }
}

fn validate_ticket_type(t: &str) -> AppResult<()> {
    match t {
        "standard" | "maintenance" | "security_log" => Ok(()),
        _ => Err(AppError::BadRequest(
            "ticket_type must be one of: standard, maintenance, security_log".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Claims, Ticket};

    fn stub_ticket(client_id: &str, deleted: bool) -> Ticket {
        Ticket {
            id: "ticket-1".into(),
            title: "T".into(),
            description: "D".into(),
            status: "open".into(),
            created_by: "creator".into(),
            client_id: client_id.into(),
            created_at: "2024-01-01T00:00:00Z".into(),
            priority: "medium".into(),
            category: None,
            due_date: None,
            estimated_completion: None,
            ticket_type: "standard".into(),
            recurring: 0,
            recurring_interval_days: None,
            last_recurred_at: None,
            deleted_at: if deleted {
                Some("2024-01-02T00:00:00Z".into())
            } else {
                None
            },
            sub_client_id: None,
            sub_client_name: None,
        }
    }

    fn claims(sub: &str, role: &str, session_type: &str, scope: Option<&str>) -> Claims {
        Claims {
            sub: sub.into(),
            role: role.into(),
            exp: 9_999_999_999,
            session_type: session_type.into(),
            ticket_scope: scope.map(str::to_owned),
            jti: None,
        }
    }

    #[test]
    fn desk_always_allowed() {
        let t = stub_ticket("c1", false);
        assert!(assert_ticket_access(&claims("desk-1", "desk", "full", None), &t).is_ok());
    }

    #[test]
    fn desk_allowed_on_soft_deleted_ticket() {
        let t = stub_ticket("c1", true);
        assert!(assert_ticket_access(&claims("desk-1", "desk", "full", None), &t).is_ok());
    }

    #[test]
    fn owner_client_allowed() {
        let t = stub_ticket("c1", false);
        assert!(assert_ticket_access(&claims("c1", "client", "full", None), &t).is_ok());
    }

    #[test]
    fn wrong_sub_denied() {
        let t = stub_ticket("c1", false);
        let r = assert_ticket_access(&claims("c2", "client", "full", None), &t);
        assert!(matches!(r, Err(AppError::NotFound)));
    }

    #[test]
    fn deleted_ticket_denied_for_non_desk() {
        let t = stub_ticket("c1", true);
        let r = assert_ticket_access(&claims("c1", "client", "full", None), &t);
        assert!(matches!(r, Err(AppError::NotFound)));
    }

    #[test]
    fn scoped_session_wrong_scope_denied() {
        let t = stub_ticket("c1", false);
        let r = assert_ticket_access(&claims("c1", "client", "scoped", Some("ticket-other")), &t);
        assert!(matches!(r, Err(AppError::Forbidden)));
    }

    #[test]
    fn unknown_role_wrong_sub_denied() {
        let t = stub_ticket("c1", false);
        let r = assert_ticket_access(&claims("c2", "superadmin", "full", None), &t);
        assert!(matches!(r, Err(AppError::NotFound)));
    }
}
