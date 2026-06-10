use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::Invoice};

const COLS: &str = "id, client_id, number, status, currency, terms, issued_date, due_date, \
     project_type, project_location, billed_to_name, billed_to_role, \
     billed_to_addr1, billed_to_addr2, billed_to_pin, billed_to_email, billed_to_phone, \
     items, notes, editor_note, \
     kra_number, recurring, recur_interval, next_recur_date, created_at";

pub struct NewInvoice<'a> {
    pub id: &'a str,
    pub client_id: &'a str,
    pub number: &'a str,
    pub currency: &'a str,
    pub terms: &'a str,
    pub issued_date: &'a str,
    pub due_date: &'a str,
    pub project_type: &'a str,
    pub project_location: &'a str,
    pub billed_to_name: &'a str,
    pub billed_to_role: &'a str,
    pub billed_to_addr1: &'a str,
    pub billed_to_addr2: &'a str,
    pub billed_to_pin: &'a str,
    pub billed_to_email: &'a str,
    pub billed_to_phone: &'a str,
    pub items_json: &'a str,
    pub notes_json: &'a str,
    pub editor_note: &'a str,
    pub kra_number: Option<&'a str>,
    pub recurring: bool,
    pub recur_interval: Option<&'a str>,
    pub next_recur_date: Option<&'a str>,
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Invoice>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {COLS} FROM invoices ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await?)
}

pub async fn list_for_client(pool: &SqlitePool, client_id: &str) -> AppResult<Vec<Invoice>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {COLS} FROM invoices WHERE client_id = ? ORDER BY created_at DESC"
    ))
    .bind(client_id)
    .fetch_all(pool)
    .await?)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<Invoice>> {
    Ok(
        sqlx::query_as(&format!("SELECT {COLS} FROM invoices WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn create(pool: &SqlitePool, n: NewInvoice<'_>) -> AppResult<Invoice> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO invoices (id, client_id, number, status, currency, terms, issued_date, \
         due_date, project_type, project_location, billed_to_name, billed_to_role, \
         billed_to_addr1, billed_to_addr2, billed_to_pin, billed_to_email, billed_to_phone, \
         items, notes, editor_note, \
         kra_number, recurring, recur_interval, next_recur_date, created_at) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(n.id)
    .bind(n.client_id)
    .bind(n.number)
    .bind("draft")
    .bind(n.currency)
    .bind(n.terms)
    .bind(n.issued_date)
    .bind(n.due_date)
    .bind(n.project_type)
    .bind(n.project_location)
    .bind(n.billed_to_name)
    .bind(n.billed_to_role)
    .bind(n.billed_to_addr1)
    .bind(n.billed_to_addr2)
    .bind(n.billed_to_pin)
    .bind(n.billed_to_email)
    .bind(n.billed_to_phone)
    .bind(n.items_json)
    .bind(n.notes_json)
    .bind(n.editor_note)
    .bind(n.kra_number)
    .bind(n.recurring as i64)
    .bind(n.recur_interval)
    .bind(n.next_recur_date)
    .bind(&created_at)
    .execute(pool)
    .await?;

    Ok(Invoice {
        id: n.id.to_owned(),
        client_id: Some(n.client_id.to_owned()),
        number: n.number.to_owned(),
        status: "draft".to_owned(),
        currency: n.currency.to_owned(),
        terms: n.terms.to_owned(),
        issued_date: n.issued_date.to_owned(),
        due_date: n.due_date.to_owned(),
        project_type: n.project_type.to_owned(),
        project_location: n.project_location.to_owned(),
        billed_to_name: n.billed_to_name.to_owned(),
        billed_to_role: n.billed_to_role.to_owned(),
        billed_to_addr1: n.billed_to_addr1.to_owned(),
        billed_to_addr2: n.billed_to_addr2.to_owned(),
        billed_to_pin: n.billed_to_pin.to_owned(),
        billed_to_email: n.billed_to_email.to_owned(),
        billed_to_phone: n.billed_to_phone.to_owned(),
        items: n.items_json.to_owned(),
        notes: n.notes_json.to_owned(),
        editor_note: n.editor_note.to_owned(),
        kra_number: n.kra_number.map(|s| s.to_owned()),
        recurring: n.recurring as i64,
        recur_interval: n.recur_interval.map(|s| s.to_owned()),
        next_recur_date: n.next_recur_date.map(|s| s.to_owned()),
        created_at,
    })
}

pub struct UpdateInvoice<'a> {
    pub number: &'a str,
    pub status: &'a str,
    pub currency: &'a str,
    pub terms: &'a str,
    pub issued_date: &'a str,
    pub due_date: &'a str,
    pub project_type: &'a str,
    pub project_location: &'a str,
    pub billed_to_name: &'a str,
    pub billed_to_role: &'a str,
    pub billed_to_addr1: &'a str,
    pub billed_to_addr2: &'a str,
    pub billed_to_pin: &'a str,
    pub billed_to_email: &'a str,
    pub billed_to_phone: &'a str,
    pub items_json: &'a str,
    pub notes_json: &'a str,
    pub editor_note: &'a str,
    pub kra_number: Option<&'a str>,
    pub recurring: bool,
    pub recur_interval: Option<&'a str>,
    pub next_recur_date: Option<&'a str>,
}

pub async fn update(pool: &SqlitePool, id: &str, u: UpdateInvoice<'_>) -> AppResult<()> {
    sqlx::query(
        "UPDATE invoices SET number=?, status=?, currency=?, terms=?, issued_date=?, due_date=?, \
         project_type=?, project_location=?, billed_to_name=?, billed_to_role=?, \
         billed_to_addr1=?, billed_to_addr2=?, billed_to_pin=?, billed_to_email=?, billed_to_phone=?, \
         items=?, notes=?, \
         editor_note=?, kra_number=?, recurring=?, recur_interval=?, next_recur_date=? \
         WHERE id=?",
    )
    .bind(u.number)
    .bind(u.status)
    .bind(u.currency)
    .bind(u.terms)
    .bind(u.issued_date)
    .bind(u.due_date)
    .bind(u.project_type)
    .bind(u.project_location)
    .bind(u.billed_to_name)
    .bind(u.billed_to_role)
    .bind(u.billed_to_addr1)
    .bind(u.billed_to_addr2)
    .bind(u.billed_to_pin)
    .bind(u.billed_to_email)
    .bind(u.billed_to_phone)
    .bind(u.items_json)
    .bind(u.notes_json)
    .bind(u.editor_note)
    .bind(u.kra_number)
    .bind(u.recurring as i64)
    .bind(u.recur_interval)
    .bind(u.next_recur_date)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM invoices WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Returns the next invoice sequence number based on the current maximum.
/// Uses MAX instead of COUNT so deletions never cause number collisions.
/// SQLite CAST stops at the first non-digit, so auto-recurring numbers like
/// "INV-0042-auto-…" correctly yield 42.
pub async fn next_seq(pool: &SqlitePool) -> AppResult<i64> {
    let (max,): (Option<i64>,) = sqlx::query_as(
        "SELECT MAX(CAST(SUBSTR(number, 5) AS INTEGER)) FROM invoices WHERE number LIKE 'INV-%'",
    )
    .fetch_one(pool)
    .await?;
    Ok(max.unwrap_or(0) + 1)
}

/// Returns recurring invoice templates whose next_recur_date <= today.
/// Excludes orphaned templates (client_id IS NULL) — recurrence was disabled by
/// cascade_delete_user_data, but belt-and-braces guard in case of direct DB edits.
pub async fn list_due_for_recurrence(pool: &SqlitePool) -> AppResult<Vec<Invoice>> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    Ok(sqlx::query_as(&format!(
        "SELECT {COLS} FROM invoices \
         WHERE recurring = 1 AND client_id IS NOT NULL \
         AND next_recur_date IS NOT NULL AND next_recur_date <= ?"
    ))
    .bind(&today)
    .fetch_all(pool)
    .await?)
}

pub async fn update_next_recur_date(pool: &SqlitePool, id: &str, next_date: &str) -> AppResult<()> {
    sqlx::query("UPDATE invoices SET next_recur_date = ? WHERE id = ?")
        .bind(next_date)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
