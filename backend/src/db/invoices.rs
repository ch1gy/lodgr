use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::Invoice};

const COLS: &str =
    "i.id, i.client_id, i.number, i.status, i.currency, i.terms, i.issued_date, i.due_date, \
     i.project_type, i.project_location, i.billed_to_name, i.billed_to_role, \
     i.billed_to_addr1, i.billed_to_addr2, i.billed_to_pin, i.billed_to_email, i.billed_to_phone, \
     i.items, i.notes, i.editor_note, \
     i.kra_number, i.tax_rate, i.recurring, i.recur_interval, i.next_recur_date, i.created_at, \
     i.sub_client_id, sc.name AS sub_client_name";

const JOIN: &str = "FROM invoices i LEFT JOIN sub_clients sc ON sc.id = i.sub_client_id";

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
    pub tax_rate: f64,
    pub recurring: bool,
    pub recur_interval: Option<&'a str>,
    pub next_recur_date: Option<&'a str>,
    pub sub_client_id: Option<&'a str>,
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Invoice>> {
    Ok(
        sqlx::query_as(&format!("SELECT {COLS} {JOIN} ORDER BY i.created_at DESC"))
            .fetch_all(pool)
            .await?,
    )
}

pub async fn list_for_client(pool: &SqlitePool, client_id: &str) -> AppResult<Vec<Invoice>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {COLS} {JOIN} WHERE i.client_id = ? ORDER BY i.created_at DESC"
    ))
    .bind(client_id)
    .fetch_all(pool)
    .await?)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<Invoice>> {
    Ok(
        sqlx::query_as(&format!("SELECT {COLS} {JOIN} WHERE i.id = ?"))
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
         kra_number, tax_rate, recurring, recur_interval, next_recur_date, created_at, \
         sub_client_id) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
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
    .bind(n.tax_rate)
    .bind(n.recurring as i64)
    .bind(n.recur_interval)
    .bind(n.next_recur_date)
    .bind(&created_at)
    .bind(n.sub_client_id)
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
        tax_rate: n.tax_rate,
        recurring: n.recurring as i64,
        recur_interval: n.recur_interval.map(|s| s.to_owned()),
        next_recur_date: n.next_recur_date.map(|s| s.to_owned()),
        created_at,
        sub_client_id: n.sub_client_id.map(|s| s.to_owned()),
        sub_client_name: None,
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
    pub tax_rate: f64,
    pub recurring: bool,
    pub recur_interval: Option<&'a str>,
    pub next_recur_date: Option<&'a str>,
    pub sub_client_id: Option<&'a str>,
}

pub async fn update(pool: &SqlitePool, id: &str, u: UpdateInvoice<'_>) -> AppResult<()> {
    sqlx::query(
        "UPDATE invoices SET number=?, status=?, currency=?, terms=?, issued_date=?, due_date=?, \
         project_type=?, project_location=?, billed_to_name=?, billed_to_role=?, \
         billed_to_addr1=?, billed_to_addr2=?, billed_to_pin=?, billed_to_email=?, billed_to_phone=?, \
         items=?, notes=?, \
         editor_note=?, kra_number=?, tax_rate=?, recurring=?, recur_interval=?, next_recur_date=?, \
         sub_client_id=? \
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
    .bind(u.tax_rate)
    .bind(u.recurring as i64)
    .bind(u.recur_interval)
    .bind(u.next_recur_date)
    .bind(u.sub_client_id)
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
        "SELECT {COLS} {JOIN} \
         WHERE i.recurring = 1 AND i.client_id IS NOT NULL \
         AND i.next_recur_date IS NOT NULL AND i.next_recur_date <= ?"
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
