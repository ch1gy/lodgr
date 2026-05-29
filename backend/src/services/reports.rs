use std::collections::HashMap;
use std::io::BufWriter;

use printpdf::{BuiltinFont, Mm, PdfDocument};
use sqlx::SqlitePool;

use crate::{
    db,
    error::{AppError, AppResult},
    models::Ticket,
};

pub struct MonthlyReport {
    pub pdf_bytes: Vec<u8>,
}

pub async fn monthly_report(
    pool: &SqlitePool,
    client_id: &str,
    year: i32,
    month: u32,
) -> AppResult<MonthlyReport> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    // Month prefix for filtering by created_at (stored as RFC-3339 text).
    let month_prefix = format!("{year}-{month:02}");
    let all_tickets = db::tickets::list_all_for_client(pool, client_id).await?;
    let month_tickets: Vec<_> = all_tickets
        .into_iter()
        .filter(|t| t.created_at.starts_with(&month_prefix))
        .collect();

    // PDF construction is CPU-bound (font embedding, page layout). Move it off
    // the async executor so it doesn't stall other in-flight requests.
    let user_name = user.name;
    let user_email = user.email;
    let pdf_bytes = tokio::task::spawn_blocking(move || {
        build_pdf(&user_name, &user_email, year, month, &month_tickets)
    })
    .await
    .map_err(|e| AppError::Internal(format!("pdf thread: {e}")))??;

    Ok(MonthlyReport { pdf_bytes })
}

fn build_pdf(
    user_name: &str,
    user_email: &str,
    year: i32,
    month: u32,
    tickets: &[Ticket],
) -> AppResult<Vec<u8>> {
    let mut by_status: HashMap<&str, usize> = HashMap::new();
    let mut by_priority: HashMap<&str, usize> = HashMap::new();
    let mut by_type: HashMap<&str, usize> = HashMap::new();

    for t in tickets {
        *by_status.entry(t.status.as_str()).or_default() += 1;
        *by_priority.entry(t.priority.as_str()).or_default() += 1;
        *by_type.entry(t.ticket_type.as_str()).or_default() += 1;
    }

    let total = tickets.len();
    let title = format!("Lodgr Support Report — {user_name} {year}/{month:02}");
    let (doc, page1, layer1) = PdfDocument::new(&title, Mm(210.0), Mm(297.0), "Page 1");
    let layer = doc.get_page(page1).get_layer(layer1);

    let regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Internal(format!("pdf font: {e}")))?;
    let bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Internal(format!("pdf font: {e}")))?;

    let y = std::cell::Cell::new(270.0_f32);
    const LH: f32 = 7.0;

    let put = |text: &str, font: &printpdf::IndirectFontRef, size: f32| {
        layer.use_text(text, size, Mm(20.0), Mm(y.get()), font);
        y.set(y.get() - LH * (size / 12.0));
    };
    let gap = |mm: f32| y.set(y.get() - mm);

    put(&title, &bold, 16.0);
    gap(4.0);
    put(
        &format!("Client: {user_name} <{user_email}>"),
        &regular,
        11.0,
    );
    put(
        &format!(
            "Period: {year}/{month:02}   Generated: {}",
            chrono::Utc::now().format("%Y-%m-%d")
        ),
        &regular,
        11.0,
    );
    gap(4.0);

    put("Summary", &bold, 13.0);
    put(
        &format!("Total tickets opened this month: {total}"),
        &regular,
        11.0,
    );
    gap(2.0);

    put("By status:", &bold, 11.0);
    for (k, v) in &by_status {
        put(&format!("  {k}: {v}"), &regular, 11.0);
    }
    gap(2.0);

    put("By priority:", &bold, 11.0);
    for (k, v) in &by_priority {
        put(&format!("  {k}: {v}"), &regular, 11.0);
    }
    gap(2.0);

    put("By type:", &bold, 11.0);
    for (k, v) in &by_type {
        put(&format!("  {k}: {v}"), &regular, 11.0);
    }
    gap(4.0);

    put("Tickets", &bold, 13.0);
    let mut shown = 0usize;
    for t in tickets {
        if y.get() < 20.0 {
            break;
        }
        put(
            &format!("[{}] {} ({})", t.status.to_uppercase(), t.title, t.priority),
            &regular,
            10.0,
        );
        put(
            &format!(
                "    ID: {}   Created: {}",
                t.id,
                t.created_at.get(..10).unwrap_or(&t.created_at),
            ),
            &regular,
            9.0,
        );
        shown += 1;
    }
    let truncated = total - shown;
    if truncated > 0 {
        gap(2.0);
        put(
            &format!(
                "... {truncated} ticket(s) not shown — download the full export for complete data."
            ),
            &regular,
            9.0,
        );
    }

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf)
        .map_err(|e| AppError::Internal(format!("pdf save: {e}")))?;
    buf.into_inner()
        .map_err(|e| AppError::Internal(format!("pdf flush: {e}")))
}
