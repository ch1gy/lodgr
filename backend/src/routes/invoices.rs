use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    db,
    dto::{InvoiceItem, InvoiceNote, InvoiceResponse},
    error::{AppError, AppResult},
    middleware::{AuthUser, DeskUser},
    models::DeskProfile,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt_num(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

// ── Tax math ─────────────────────────────────────────────────────────────────

/// Thin pub wrapper for integration tests.
pub fn render_invoice_html_pub(inv: &InvoiceResponse, desk: &DeskProfile) -> String {
    render_invoice_html(inv, desk)
}

/// Compute VAT and grand total. Single authoritative location — frontend must
/// mirror this rule: `vat = round(subtotal * rate / 100)`, half-up.
pub fn compute_totals(subtotal: u64, tax_rate: f64) -> (u64, u64) {
    if tax_rate <= 0.0 {
        return (0, subtotal);
    }
    let vat = (subtotal as f64 * tax_rate / 100.0).round() as u64;
    (vat, subtotal + vat)
}

/// Format a rate like 16.0 → "16", 8.1 → "8.1".
fn fmt_rate(r: f64) -> String {
    if r == r.trunc() {
        format!("{}", r as u64)
    } else {
        format!("{r}")
    }
}

// ── HTML renderer ─────────────────────────────────────────────────────────────

fn render_invoice_html(inv: &InvoiceResponse, desk: &DeskProfile) -> String {
    let subtotal: u64 = inv.items.iter().map(|it| it.qty as u64 * it.rate).sum();
    let (vat, total) = compute_totals(subtotal, inv.tax_rate);

    let item_rows: String = inv
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let sub_html = it
                .sub
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| format!("<div class=\"sub\">{}</div>", html_esc(s)))
                .unwrap_or_default();
            format!(
                "<div class=\"inv__item\">\
                    <div class=\"num\">{num:02}</div>\
                    <div>\
                        <div class=\"name\">{name}</div>\
                        {sub}\
                    </div>\
                    <div class=\"qty\">{qty}</div>\
                    <div class=\"rate\"><span class=\"pre\">@</span>{rate}</div>\
                    <div class=\"amt\">{amt}</div>\
                </div>",
                num = i + 1,
                name = html_esc(&it.name),
                sub = sub_html,
                qty = it.qty,
                rate = fmt_num(it.rate),
                amt = fmt_num(it.qty as u64 * it.rate),
            )
        })
        .collect();

    let addr_html = {
        let mut lines = Vec::new();
        if !inv.billed_to_role.is_empty() {
            lines.push(format!(
                "<div class=\"inv__metaLine\">{}</div>",
                html_esc(&inv.billed_to_role)
            ));
        }
        if !inv.billed_to_addr1.is_empty() {
            lines.push(format!(
                "<div class=\"inv__metaLine\">{}</div>",
                html_esc(&inv.billed_to_addr1)
            ));
        }
        if !inv.billed_to_addr2.is_empty() {
            lines.push(format!(
                "<div class=\"inv__metaLine\">{}</div>",
                html_esc(&inv.billed_to_addr2)
            ));
        }
        if !inv.billed_to_pin.is_empty() {
            lines.push(format!(
                "<div class=\"inv__metaMono\"><span class=\"k\">PIN</span>{}</div>",
                html_esc(&inv.billed_to_pin)
            ));
        }
        if !inv.billed_to_email.is_empty() {
            lines.push(format!(
                "<div class=\"inv__metaMono\"><span class=\"k\">Email</span>{}</div>",
                html_esc(&inv.billed_to_email)
            ));
        }
        if !inv.billed_to_phone.is_empty() {
            lines.push(format!(
                "<div class=\"inv__metaMono\"><span class=\"k\">Tel</span>{}</div>",
                html_esc(&inv.billed_to_phone)
            ));
        }
        lines.join("")
    };

    let note_rows: String = inv
        .notes
        .iter()
        .map(|n| {
            format!(
                "<div class=\"row\">\
                    <div class=\"k\">{}</div>\
                    <div class=\"v\">{}</div>\
                </div>",
                html_esc(&n.k),
                html_esc(&n.v),
            )
        })
        .collect();

    let proj_month = NaiveDate::parse_from_str(&inv.issued_date, "%Y-%m-%d")
        .map(|d| d.format("%B %Y").to_string())
        .unwrap_or_else(|_| inv.issued_date.clone());

    let vol_num = inv
        .number
        .split('-')
        .next_back()
        .unwrap_or(&inv.number)
        .trim_start_matches('0');

    let kra_line = inv
        .kra_number
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|k| {
            format!(
                "<div class=\"inv__metaMono\" style=\"margin-top:8px\">\
                    <span class=\"k\">KRA No.</span>{}</div>",
                html_esc(k)
            )
        })
        .unwrap_or_default();

    let subtotals_html = if inv.tax_rate > 0.0 {
        format!(
            "<div class=\"inv__subtotals\">\
               <div class=\"row\"><span class=\"lbl\">Subtotal</span><span>{currency} {sub}</span></div>\
               <div class=\"row\"><span class=\"lbl\">VAT ({rate}%)</span><span>{currency} {vat_amt}</span></div>\
             </div>",
            currency = html_esc(&inv.currency),
            sub = fmt_num(subtotal),
            rate = fmt_rate(inv.tax_rate),
            vat_amt = fmt_num(vat),
        )
    } else {
        String::new()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<link rel="preconnect" href="https://fonts.googleapis.com"/>
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin/>
<link href="https://fonts.googleapis.com/css2?family=Archivo+Narrow:ital,wght@0,400;0,500;0,600;1,400&family=DM+Serif+Display:ital@0;1&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet"/>
<style>
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
html,body{{background:#ece5d3;padding:0;margin:0;height:100%}}
.inv{{
  --paper:#ece5d3;--ink:#14110d;--ink-soft:#3d3830;--mid:#6b6356;--red:#c9402b;
  --display:"DM Serif Display",Georgia,serif;
  --mono:"JetBrains Mono","Courier New",monospace;
  --sans:"Archivo Narrow","Arial Narrow",Arial,sans-serif;
  width:100%;margin:0;min-height:100vh;display:flex;flex-direction:column;
  background:var(--paper);color:var(--ink);font-family:var(--sans);-webkit-font-smoothing:antialiased
}}
.inv__mast,.inv__colo{{
  background:var(--ink);color:var(--paper);padding:16px 44px;
  display:flex;justify-content:space-between;align-items:baseline;
  font-family:var(--mono);font-size:11px;letter-spacing:.22em;text-transform:uppercase
}}
.inv__mast .word{{font-family:var(--display);font-size:14px;letter-spacing:.14em}}
.inv__mast .est{{font-family:var(--display);font-style:italic;font-size:14px;letter-spacing:0;text-transform:none}}
.inv__body{{padding:36px 44px 40px;flex:1}}
.inv__head{{display:grid;grid-template-columns:1fr auto;align-items:flex-start;margin-bottom:8px}}
.inv__refLabel{{font-family:var(--mono);font-size:10px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid)}}
.inv__refVal{{font-family:var(--display);font-style:italic;font-size:20px;line-height:1;color:var(--ink);margin-top:4px}}
.inv__stamp{{font-family:var(--mono);font-size:10px;letter-spacing:.18em;color:var(--red);text-transform:uppercase;border:1px solid var(--red);padding:5px 9px;transform:rotate(-3deg);line-height:1}}
.inv__title{{font-family:var(--display);font-size:64px;line-height:.95;letter-spacing:-0.02em;margin:28px 0 6px;font-weight:400}}
.inv__title em{{color:var(--red);font-style:italic}}
.inv__strap{{font-family:var(--mono);font-size:11px;color:var(--mid);letter-spacing:.14em;text-transform:uppercase}}
.inv__meta{{display:grid;grid-template-columns:1fr 1fr 1fr;gap:32px;margin-top:36px;padding:22px 0;border-top:1px solid var(--ink);border-bottom:1px solid var(--ink)}}
.inv__metaLabel{{font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid);margin-bottom:8px}}
.inv__metaName{{font-family:var(--display);font-size:18px;line-height:1.1;color:var(--ink);margin-bottom:6px}}
.inv__metaLine{{font-family:var(--sans);font-weight:500;font-size:13px;color:var(--ink-soft);line-height:1.55}}
.inv__metaMono{{font-family:var(--mono);font-weight:500;font-size:11px;color:var(--ink-soft);letter-spacing:.04em;margin-top:4px}}
.inv__metaMono .k{{color:var(--mid);letter-spacing:.18em;text-transform:uppercase;font-size:9.5px;margin-right:6px}}
.inv__items{{margin-top:32px}}
.inv__itemsHead{{display:grid;grid-template-columns:32px 1fr 70px 110px 130px;gap:16px;padding:8px 0;border-bottom:1px solid var(--ink);font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid)}}
.inv__itemsHead .r{{text-align:right}}
.inv__item{{display:grid;grid-template-columns:32px 1fr 70px 110px 130px;gap:16px;padding:14px 0;border-bottom:1px solid #14110d22;align-items:baseline;page-break-inside:avoid}}
.inv__item .num{{font-family:var(--mono);font-weight:500;font-size:10px;color:var(--mid);letter-spacing:.14em}}
.inv__item .name{{font-family:var(--display);font-size:17px;line-height:1.1;color:var(--ink);margin:0 0 2px}}
.inv__item .name::before{{content:'';display:inline-block;width:4px;height:4px;background:var(--red);margin-right:8px;transform:translateY(-3px)}}
.inv__item .sub{{font-family:var(--sans);font-weight:500;font-size:12.5px;color:var(--ink-soft);line-height:1.5;margin-left:12px}}
.inv__item .qty,.inv__item .rate,.inv__item .amt{{font-family:var(--mono);font-weight:500;font-size:12.5px;color:var(--ink);text-align:right;font-variant-numeric:tabular-nums;letter-spacing:.02em}}
.inv__item .rate .pre{{color:var(--mid);font-style:italic;font-family:var(--display);margin-right:3px;font-weight:400}}
.inv__totals{{margin-top:18px;display:grid;grid-template-columns:1fr 360px;gap:32px;align-items:start;page-break-inside:avoid}}
.inv__editor{{font-family:var(--display);font-style:italic;font-size:14px;line-height:1.55;color:var(--ink-soft);max-width:320px}}
.inv__editor::before{{content:'— Editor';display:block;font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--red);font-style:normal;margin-bottom:6px}}
.inv__grand{{margin-top:14px;padding-top:18px;border-top:2px solid var(--ink);display:grid;grid-template-columns:auto 1fr;gap:18px;align-items:end}}
.inv__grand .metaCol{{font-family:var(--mono);font-size:9.5px;color:var(--mid);letter-spacing:.22em;text-transform:uppercase;line-height:1.4}}
.inv__grand .metaCol em{{display:block;font-family:var(--display);font-style:italic;font-size:13px;color:var(--red);text-transform:none;letter-spacing:0;margin-top:4px}}
.inv__grand .total{{font-family:var(--display);font-size:64px;line-height:.92;letter-spacing:-0.02em;text-align:right;color:var(--ink);font-variant-numeric:tabular-nums}}
.inv__grand .total .ccy{{font-size:22px;font-style:italic;color:var(--mid);margin-right:4px;vertical-align:19px}}
.inv__subtotals{{margin-top:14px;padding-top:14px;border-top:1px solid #14110d44}}
.inv__subtotals .row{{display:flex;justify-content:space-between;font-family:var(--mono);font-size:11px;color:var(--ink-soft);padding:3px 0;letter-spacing:.04em}}
.inv__subtotals .row .lbl{{color:var(--mid);text-transform:uppercase;letter-spacing:.14em;font-size:10px}}
.inv__due{{margin-top:8px;text-align:right;font-family:var(--mono);font-weight:500;font-size:11px;color:var(--ink);letter-spacing:.04em}}
.inv__due .lbl{{color:var(--mid);margin-right:8px;text-transform:uppercase;letter-spacing:.18em;font-size:10px}}
.inv__notes{{margin-top:32px;padding-top:22px;border-top:1px solid var(--ink);page-break-inside:avoid}}
.inv__notes .row{{display:grid;grid-template-columns:110px 1fr;gap:22px;padding:12px 0;border-bottom:1px solid #14110d22;page-break-inside:avoid}}
.inv__notes .row:last-child{{border-bottom:none}}
.inv__notes .row:first-child{{padding-top:0}}
.inv__notes .k{{font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid);padding-top:2px}}
.inv__notes .v{{font-family:var(--sans);font-weight:500;font-size:13px;color:var(--ink-soft);line-height:1.55}}
.inv__colo .vol{{font-family:var(--display);font-style:italic;font-size:13px;letter-spacing:0;text-transform:none}}
.inv__colo .right{{display:flex;gap:16px;align-items:baseline}}
.inv__colo .sep{{opacity:.4}}
.inv__print-btn{{
  position:fixed;bottom:24px;right:24px;
  background:var(--ink);color:var(--paper);
  font-family:var(--mono);font-size:10px;letter-spacing:.18em;text-transform:uppercase;
  border:none;padding:10px 18px;cursor:pointer;
}}
.inv__print-btn:hover{{background:var(--red)}}
@media print{{
  @page{{size:A4;margin:0}}
  html,body{{background:var(--paper)!important;padding:0}}
  .inv{{width:100%}}
  .inv__print-btn{{display:none}}
  .inv__mast{{position:running(header)}}
}}
</style>
</head>
<body>
<article class="inv">
  <header class="inv__mast">
    <span class="word">{me_name}</span>
    <span class="est">est. {proj_month}</span>
  </header>
  <section class="inv__body">
    <div class="inv__head">
      <div>
        <div class="inv__refLabel">Invoice &#8470;</div>
        <div class="inv__refVal">{number}</div>
      </div>
      <div class="inv__stamp">Issued &#183; {issued_date}</div>
    </div>
    <h1 class="inv__title">An <em>Invoice.</em></h1>
    <div class="inv__strap">{proj_type} &#183; {proj_loc} &#183; {proj_month}</div>
    <div class="inv__meta">
      <div>
        <div class="inv__metaLabel">Billed to</div>
        <div class="inv__metaName">{billed_name}</div>
        {addr_html}
      </div>
      <div>
        <div class="inv__metaLabel">From</div>
        <div class="inv__metaName">{me_name}</div>
        <div class="inv__metaLine">{me_role}</div>
        <div class="inv__metaLine">{me_email}</div>
        <div class="inv__metaLine">{me_site}</div>
      </div>
      <div>
        <div class="inv__metaLabel">Dates</div>
        <div class="inv__metaMono"><span class="k">Issued</span>{issued_date}</div>
        <div class="inv__metaMono"><span class="k">Due</span>{due_date}</div>
        <div class="inv__metaMono"><span class="k">Terms</span>{terms}</div>
        <div class="inv__metaMono"><span class="k">Currency</span>{currency}</div>
        {kra_line}
      </div>
    </div>
    <div class="inv__items">
      <div class="inv__itemsHead">
        <span>&#8470;</span><span>Description</span>
        <span class="r">Qty</span><span class="r">Rate</span><span class="r">Amount</span>
      </div>
      {item_rows}
    </div>
    <div class="inv__totals">
      <div class="inv__editor">{editor_note}</div>
      <div>
        {subtotals_html}
        <div class="inv__grand">
          <div class="metaCol">Total due<br/><em>{terms_lower}</em></div>
          <div class="total"><span class="ccy">{currency}</span>{total}</div>
        </div>
        <div class="inv__due"><span class="lbl">Due by</span>{due_date}</div>
      </div>
    </div>
    {notes_section}
  </section>
  <footer class="inv__colo">
    <span class="vol">Vol. 1 &#183; No. {vol_num}</span>
    <span class="right">
      <span>{me_email}</span><span class="sep">&#183;</span>
      <span>{me_site}</span><span class="sep">&#183;</span>
      <span>{me_city}</span>
    </span>
  </footer>
</article>
<button class="inv__print-btn" onclick="window.print()">Print / Save PDF ↓</button>
</body>
</html>"#,
        me_name = html_esc(&desk.name),
        me_role = html_esc(&desk.tagline),
        me_email = html_esc(&desk.email),
        me_site = html_esc(&desk.website),
        me_city = html_esc(&desk.city),
        number = html_esc(&inv.number),
        issued_date = html_esc(&inv.issued_date),
        due_date = html_esc(&inv.due_date),
        terms = html_esc(&inv.terms),
        terms_lower = html_esc(&inv.terms.to_lowercase()),
        currency = html_esc(&inv.currency),
        proj_type = html_esc(&inv.project_type),
        proj_loc = html_esc(&inv.project_location),
        proj_month = html_esc(&proj_month),
        billed_name = html_esc(&inv.billed_to_name),
        addr_html = addr_html,
        kra_line = kra_line,
        item_rows = item_rows,
        editor_note = html_esc(&inv.editor_note),
        subtotals_html = subtotals_html,
        total = fmt_num(total),
        vol_num = html_esc(vol_num),
        notes_section = if note_rows.is_empty() {
            String::new()
        } else {
            format!("<div class=\"inv__notes\">{note_rows}</div>")
        },
    )
}

// ── Request/response types ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListQuery {
    pub client_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateInvoiceRequest {
    pub number: Option<String>,
    pub currency: Option<String>,
    pub terms: Option<String>,
    pub issued_date: String,
    pub due_date: String,
    pub project_type: Option<String>,
    pub project_location: Option<String>,
    pub billed_to_name: String,
    pub billed_to_role: Option<String>,
    pub billed_to_addr1: Option<String>,
    pub billed_to_addr2: Option<String>,
    pub billed_to_pin: Option<String>,
    pub billed_to_email: Option<String>,
    pub billed_to_phone: Option<String>,
    pub items: Vec<InvoiceItem>,
    pub notes: Option<Vec<InvoiceNote>>,
    pub editor_note: Option<String>,
    pub kra_number: Option<String>,
    pub tax_rate: Option<f64>,
    pub client_id: String,
    pub recurring: Option<bool>,
    pub recur_interval: Option<String>,
    pub next_recur_date: Option<String>,
    pub sub_client_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateInvoiceRequest {
    pub number: Option<String>,
    pub status: Option<String>,
    pub currency: Option<String>,
    pub terms: Option<String>,
    pub issued_date: Option<String>,
    pub due_date: Option<String>,
    pub project_type: Option<String>,
    pub project_location: Option<String>,
    pub billed_to_name: Option<String>,
    pub billed_to_role: Option<String>,
    pub billed_to_addr1: Option<String>,
    pub billed_to_addr2: Option<String>,
    pub billed_to_pin: Option<String>,
    pub billed_to_email: Option<String>,
    pub billed_to_phone: Option<String>,
    pub items: Option<Vec<InvoiceItem>>,
    pub notes: Option<Vec<InvoiceNote>>,
    pub editor_note: Option<String>,
    pub kra_number: Option<String>,
    pub tax_rate: Option<f64>,
    pub recurring: Option<bool>,
    pub recur_interval: Option<String>,
    pub next_recur_date: Option<String>,
    /// Explicit empty string clears the sub-client; omitting keeps current value.
    pub sub_client_id: Option<String>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn list(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
    Query(q): Query<ListQuery>,
) -> AppResult<impl IntoResponse> {
    let invoices = if claims.role == "desk" {
        if let Some(cid) = q.client_id {
            db::invoices::list_for_client(&pool, &cid).await?
        } else {
            db::invoices::list(&pool).await?
        }
    } else {
        // Clients only ever see their own invoices, regardless of query params.
        db::invoices::list_for_client(&pool, &claims.sub).await?
    };
    let dtos: Vec<InvoiceResponse> = invoices.into_iter().map(InvoiceResponse::from).collect();
    Ok(Json(dtos))
}

pub async fn create(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Json(body): Json<CreateInvoiceRequest>,
) -> AppResult<impl IntoResponse> {
    if body.items.is_empty() {
        return Err(AppError::BadRequest(
            "at least one line item is required".into(),
        ));
    }

    let tax_rate = body.tax_rate.unwrap_or(0.0);
    if !tax_rate.is_finite() || !(0.0..=100.0).contains(&tax_rate) {
        return Err(AppError::BadRequest(
            "tax_rate must be a finite number between 0 and 100".into(),
        ));
    }

    if let Some(sc_id) = body.sub_client_id.as_deref().filter(|s| !s.is_empty()) {
        match db::sub_clients::find_by_id(&pool, sc_id).await? {
            Some(sc) if sc.client_id == body.client_id => {}
            Some(_) => {
                return Err(AppError::BadRequest(
                    "sub_client does not belong to this client".into(),
                ))
            }
            None => return Err(AppError::BadRequest("sub_client not found".into())),
        }
    }

    // Auto-generate number if not provided.
    let auto_number;
    let number = match body
        .number
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(n) => n,
        None => {
            let seq = db::invoices::next_seq(&pool).await?;
            auto_number = format!("INV-{seq:04}");
            &auto_number
        }
    };

    let id = Uuid::new_v4().to_string();
    let items_json = serde_json::to_string(&body.items)
        .map_err(|e| AppError::Internal(format!("items serialization: {e}")))?;
    let notes_json = serde_json::to_string(&body.notes.unwrap_or_default())
        .map_err(|e| AppError::Internal(format!("notes serialization: {e}")))?;

    let inv = db::invoices::create(
        &pool,
        db::invoices::NewInvoice {
            id: &id,
            client_id: &body.client_id,
            number,
            currency: body.currency.as_deref().unwrap_or("KES"),
            terms: body.terms.as_deref().unwrap_or("Net 14"),
            issued_date: &body.issued_date,
            due_date: &body.due_date,
            project_type: body.project_type.as_deref().unwrap_or(""),
            project_location: body.project_location.as_deref().unwrap_or(""),
            billed_to_name: &body.billed_to_name,
            billed_to_role: body.billed_to_role.as_deref().unwrap_or(""),
            billed_to_addr1: body.billed_to_addr1.as_deref().unwrap_or(""),
            billed_to_addr2: body.billed_to_addr2.as_deref().unwrap_or(""),
            billed_to_pin: body.billed_to_pin.as_deref().unwrap_or(""),
            billed_to_email: body.billed_to_email.as_deref().unwrap_or(""),
            billed_to_phone: body.billed_to_phone.as_deref().unwrap_or(""),
            items_json: &items_json,
            notes_json: &notes_json,
            editor_note: body.editor_note.as_deref().unwrap_or(""),
            kra_number: body.kra_number.as_deref(),
            tax_rate,
            recurring: body.recurring.unwrap_or(false),
            recur_interval: body.recur_interval.as_deref(),
            next_recur_date: body.next_recur_date.as_deref(),
            sub_client_id: body.sub_client_id.as_deref().filter(|s| !s.is_empty()),
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(InvoiceResponse::from(inv))))
}

pub async fn get(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let inv = db::invoices::find_by_id(&pool, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    if claims.role != "desk" && inv.client_id.as_deref() != Some(claims.sub.as_str()) {
        return Err(AppError::NotFound);
    }
    Ok(Json(InvoiceResponse::from(inv)))
}

pub async fn update(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateInvoiceRequest>,
) -> AppResult<impl IntoResponse> {
    let inv = db::invoices::find_by_id(&pool, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let items_json = if let Some(ref items) = body.items {
        serde_json::to_string(items)
            .map_err(|e| AppError::Internal(format!("items serialization: {e}")))?
    } else {
        inv.items.clone()
    };
    let notes_json = if let Some(ref notes) = body.notes {
        serde_json::to_string(notes)
            .map_err(|e| AppError::Internal(format!("notes serialization: {e}")))?
    } else {
        inv.notes.clone()
    };

    let tax_rate = match body.tax_rate {
        Some(r) => {
            if !r.is_finite() || !(0.0..=100.0).contains(&r) {
                return Err(AppError::BadRequest(
                    "tax_rate must be a finite number between 0 and 100".into(),
                ));
            }
            r
        }
        None => inv.tax_rate,
    };

    // kra_number: explicit null in JSON clears it; omitting keeps current value.
    // Since serde Option<Option<String>> is complex, we just accept Option<String>
    // and use the provided value or fall back to current.
    let kra = body.kra_number.as_deref().or(inv.kra_number.as_deref());

    let target_client_id = inv.client_id.as_deref();
    let sub_client_id = match body.sub_client_id.as_deref() {
        Some("") => None,
        Some(sc_id) => {
            match db::sub_clients::find_by_id(&pool, sc_id).await? {
                Some(sc) if Some(sc.client_id.as_str()) == target_client_id => {}
                Some(_) => {
                    return Err(AppError::BadRequest(
                        "sub_client does not belong to this client".into(),
                    ))
                }
                None => return Err(AppError::BadRequest("sub_client not found".into())),
            }
            Some(sc_id)
        }
        None => inv.sub_client_id.as_deref(),
    };

    db::invoices::update(
        &pool,
        &id,
        db::invoices::UpdateInvoice {
            number: body.number.as_deref().unwrap_or(&inv.number),
            status: body.status.as_deref().unwrap_or(&inv.status),
            currency: body.currency.as_deref().unwrap_or(&inv.currency),
            terms: body.terms.as_deref().unwrap_or(&inv.terms),
            issued_date: body.issued_date.as_deref().unwrap_or(&inv.issued_date),
            due_date: body.due_date.as_deref().unwrap_or(&inv.due_date),
            project_type: body.project_type.as_deref().unwrap_or(&inv.project_type),
            project_location: body
                .project_location
                .as_deref()
                .unwrap_or(&inv.project_location),
            billed_to_name: body
                .billed_to_name
                .as_deref()
                .unwrap_or(&inv.billed_to_name),
            billed_to_role: body
                .billed_to_role
                .as_deref()
                .unwrap_or(&inv.billed_to_role),
            billed_to_addr1: body
                .billed_to_addr1
                .as_deref()
                .unwrap_or(&inv.billed_to_addr1),
            billed_to_addr2: body
                .billed_to_addr2
                .as_deref()
                .unwrap_or(&inv.billed_to_addr2),
            billed_to_pin: body.billed_to_pin.as_deref().unwrap_or(&inv.billed_to_pin),
            billed_to_email: body
                .billed_to_email
                .as_deref()
                .unwrap_or(&inv.billed_to_email),
            billed_to_phone: body
                .billed_to_phone
                .as_deref()
                .unwrap_or(&inv.billed_to_phone),
            items_json: &items_json,
            notes_json: &notes_json,
            editor_note: body.editor_note.as_deref().unwrap_or(&inv.editor_note),
            kra_number: kra,
            tax_rate,
            recurring: body.recurring.unwrap_or(inv.recurring != 0),
            recur_interval: body
                .recur_interval
                .as_deref()
                .or(inv.recur_interval.as_deref()),
            next_recur_date: body
                .next_recur_date
                .as_deref()
                .or(inv.next_recur_date.as_deref()),
            sub_client_id,
        },
    )
    .await?;

    let updated = db::invoices::find_by_id(&pool, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(InvoiceResponse::from(updated)))
}

pub async fn delete(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    db::invoices::find_by_id(&pool, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    db::invoices::delete(&pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Generates and streams a PDF download of the invoice via Puppeteer.
pub async fn pdf_download(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let inv = db::invoices::find_by_id(&pool, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    if claims.role != "desk" && inv.client_id.as_deref() != Some(claims.sub.as_str()) {
        return Err(AppError::NotFound);
    }
    let desk = db::desk_profile::get(&pool).await?;
    let dto = InvoiceResponse::from(inv);
    let number = dto.number.clone();
    let html = render_invoice_html(&dto, &desk);

    let worker = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pdf-worker.js");
    let mut child = tokio::process::Command::new("node")
        .arg(&worker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::error::AppError::Internal(format!("spawn node: {e}")))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| crate::error::AppError::Internal("no stdin".into()))?;
    stdin
        .write_all(html.as_bytes())
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("write stdin: {e}")))?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("pdf worker: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(crate::error::AppError::Internal(format!(
            "pdf-worker: {err}"
        )));
    }

    let filename = format!("invoice-{number}.pdf");
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        output.stdout,
    ))
}

/// Returns a print-ready HTML page for the invoice.
/// Sets a permissive CSP so Google Fonts loads correctly.
pub async fn print_html(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let inv = db::invoices::find_by_id(&pool, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    if claims.role != "desk" && inv.client_id.as_deref() != Some(claims.sub.as_str()) {
        return Err(AppError::NotFound);
    }
    let desk = db::desk_profile::get(&pool).await?;
    let dto = InvoiceResponse::from(inv);
    let html = render_invoice_html(&dto, &desk);

    Ok((
        [
            (
                header::CONTENT_TYPE,
                "text/html; charset=utf-8",
            ),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'self'; style-src 'self' https://fonts.googleapis.com 'unsafe-inline'; font-src 'self' https://fonts.gstatic.com; script-src 'unsafe-inline'; object-src 'none'; frame-ancestors 'none'",
            ),
        ],
        Html(html),
    ))
}
