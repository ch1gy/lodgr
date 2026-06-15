use std::process::Stdio;

use sqlx::SqlitePool;
use tokio::io::AsyncWriteExt;

use crate::{
    crypto::{self, EncryptionKey},
    db,
    error::{AppError, AppResult},
    models::Ticket,
};

pub struct MonthlyReport {
    pub pdf_bytes: Vec<u8>,
}

pub async fn range_report(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    client_id: &str,
    from: &str,
    to: &str,
) -> AppResult<MonthlyReport> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }

    let client_email = crypto::decrypt(enc_key, &user.email_nonce, &user.email)
        .unwrap_or_else(|err| {
            tracing::warn!(user_id = %client_id, "could not decrypt client email for report: {err}");
            String::from("(email unavailable)")
        });

    let all_tickets = db::tickets::list_all_for_client(pool, client_id).await?;
    // created_at is stored as RFC-3339; compare the first 10 chars (YYYY-MM-DD)
    let tickets: Vec<_> = all_tickets
        .into_iter()
        .filter(|t| {
            let d = t.created_at.get(..10).unwrap_or(&t.created_at);
            d >= from && d <= to
        })
        .collect();

    let period = format!("{} – {}", from, to);
    let html = render_report_html(&user.name, &client_email, &period, &tickets);

    let worker =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pdf-worker.js");
    let mut child = tokio::process::Command::new("node")
        .arg(&worker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(format!("spawn node: {e}")))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("no stdin".into()))?;
    stdin
        .write_all(html.as_bytes())
        .await
        .map_err(|e| AppError::Internal(format!("write stdin: {e}")))?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AppError::Internal(format!("pdf worker: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!("pdf-worker: {err}")));
    }

    Ok(MonthlyReport {
        pdf_bytes: output.stdout,
    })
}

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}


fn render_report_html(
    client_name: &str,
    client_email: &str,
    period: &str,
    tickets: &[Ticket],
) -> String {
    let total = tickets.len();
    let generated = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Aggregate counts
    let mut by_status: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut by_priority: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut by_type: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for t in tickets {
        *by_status.entry(t.status.as_str()).or_default() += 1;
        *by_priority.entry(t.priority.as_str()).or_default() += 1;
        *by_type.entry(t.ticket_type.as_str()).or_default() += 1;
    }

    let stat_rows = |map: &std::collections::BTreeMap<&str, usize>| -> String {
        map.iter()
            .map(|(k, v)| format!(
                "<div class=\"rp__kv\"><span class=\"k\">{}</span><span class=\"v\">{:02}</span></div>",
                html_esc(k), v
            ))
            .collect()
    };

    let ticket_rows: String = tickets
        .iter()
        .map(|t| {
            let date = t.created_at.get(..10).unwrap_or(&t.created_at);
            format!(
                "<div class=\"rp__ticket\">\
                    <div class=\"rp__ticket-head\">\
                        <span class=\"rp__ticket-status {status_cls}\">{status}</span>\
                        <span class=\"rp__ticket-title\">{title}</span>\
                        <span class=\"rp__ticket-meta\">{priority} · {ttype}</span>\
                    </div>\
                    <div class=\"rp__ticket-sub\">\
                        <span class=\"rp__ticket-id\">#{id}</span>\
                        <span class=\"rp__ticket-date\">{date}</span>\
                    </div>\
                </div>",
                status_cls = html_esc(t.status.as_str()),
                status = html_esc(&t.status.to_uppercase()),
                title = html_esc(&t.title),
                priority = html_esc(&t.priority),
                ttype = html_esc(&t.ticket_type),
                id = html_esc(t.id.get(..8).unwrap_or(&t.id)),
                date = html_esc(date),
            )
        })
        .collect();

    // Per-ticket detail pages (one page each, appended after overview)
    let detail_pages: String = tickets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let date = t.created_at.get(..10).unwrap_or(&t.created_at);
            let title_short = if t.title.len() > 60 {
                format!("{}…", html_esc(&t.title[..60]))
            } else {
                html_esc(&t.title)
            };
            let sub = t.sub_client_name.as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| format!(
                    "<div class=\"rp__det-sub\"><span class=\"rp__det-sub-label\">Sub-client</span>{}</div>",
                    html_esc(s)
                ))
                .unwrap_or_default();
            let desc = html_esc(&t.description);
            format!(
                "<div class=\"rp__detail-page\">\
                    <table style=\"width:100%;border-collapse:collapse\">\
                      <thead style=\"display:table-header-group\">\
                        <tr><td style=\"padding:0\">\
                          <div class=\"rp__det-mast\">\
                            <span class=\"word\">Lodgr.</span>\
                            <span class=\"rp__det-pg\">{num:02} / {total:02}</span>\
                          </div>\
                          <div class=\"rp__det-cont\">\
                            <span class=\"rp__det-cont-id\">#{id}</span>\
                            <span class=\"rp__det-cont-ttl\">{title_short}</span>\
                          </div>\
                        </td></tr>\
                      </thead>\
                      <tbody style=\"display:table-row-group\">\
                        <tr><td>\
                          <div class=\"rp__det-body\">\
                            <div class=\"rp__det-eyebrow\">\
                                <span class=\"rp__ticket-status {status_cls}\">{status}</span>\
                                <span class=\"rp__det-id\">#{id}</span>\
                                <span class=\"rp__det-date\">{date}</span>\
                            </div>\
                            <h2 class=\"rp__det-title\">{title}</h2>\
                            <div class=\"rp__det-meta\">\
                                <span>{priority}</span>\
                                <span class=\"sep\">·</span>\
                                <span>{ttype}</span>\
                            </div>\
                            {sub}\
                            <div class=\"rp__det-rule\"></div>\
                            <div class=\"rp__det-desc\">{desc}</div>\
                          </div>\
                        </td></tr>\
                      </tbody>\
                    </table>\
                </div>",
                num = i + 1,
                total = tickets.len(),
                status_cls = html_esc(t.status.as_str()),
                status = html_esc(&t.status.to_uppercase()),
                id = html_esc(t.id.get(..8).unwrap_or(&t.id)),
                date = html_esc(date),
                title = html_esc(&t.title),
                title_short = title_short,
                priority = html_esc(&t.priority),
                ttype = html_esc(&t.ticket_type),
                sub = sub,
                desc = desc,
            )
        })
        .collect();

    let empty_state = if total == 0 {
        "<div class=\"rp__empty\">No tickets were opened this period.</div>"
    } else {
        ""
    };

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<link rel="preconnect" href="https://fonts.googleapis.com"/>
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin/>
<link href="https://fonts.googleapis.com/css2?family=Archivo+Narrow:ital,wght@0,400;0,500;0,600;1,400&family=DM+Serif+Display:ital@0;1&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet"/>
<script src="https://cdn.jsdelivr.net/npm/marked@9/marked.min.js"></script>
<style>
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
:root{{
  --paper:#ece5d3;--ink:#14110d;--ink-soft:#3d3830;--mid:#6b6356;--red:#c9402b;
  --display:"DM Serif Display",Georgia,serif;
  --mono:"JetBrains Mono","Courier New",monospace;
  --sans:"Archivo Narrow","Arial Narrow",Arial,sans-serif;
}}
html,body{{background:var(--paper);padding:0;margin:0;height:100%;font-family:var(--sans);color:var(--ink);-webkit-font-smoothing:antialiased}}
.rp{{
  width:100%;margin:0;min-height:100vh;display:flex;flex-direction:column;
  background:var(--paper);
}}
.rp__mast,.rp__colo{{
  background:var(--ink);color:var(--paper);padding:16px 44px;
  display:flex;justify-content:space-between;align-items:baseline;
  font-family:var(--mono);font-size:11px;letter-spacing:.22em;text-transform:uppercase
}}
.rp__mast .word{{font-family:var(--display);font-size:14px;letter-spacing:.14em}}
.rp__mast .period{{font-family:var(--display);font-style:italic;font-size:14px;letter-spacing:0;text-transform:none}}
.rp__body{{padding:36px 44px 40px;flex:1}}
.rp__head{{display:grid;grid-template-columns:1fr auto;align-items:flex-start;margin-bottom:8px}}
.rp__refLabel{{font-family:var(--mono);font-size:10px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid)}}
.rp__refVal{{font-family:var(--display);font-style:italic;font-size:20px;line-height:1;color:var(--ink);margin-top:4px}}
.rp__stamp{{font-family:var(--mono);font-size:10px;letter-spacing:.18em;color:var(--red);text-transform:uppercase;border:1px solid var(--red);padding:5px 9px;transform:rotate(-3deg);line-height:1}}
.rp__title{{font-family:var(--display);font-size:64px;line-height:.95;letter-spacing:-0.02em;margin:28px 0 6px;font-weight:400}}
.rp__title em{{color:var(--red);font-style:italic}}
.rp__strap{{font-family:var(--mono);font-size:11px;color:var(--mid);letter-spacing:.14em;text-transform:uppercase}}
.rp__meta{{display:grid;grid-template-columns:1fr 1fr 1fr;gap:32px;margin-top:36px;padding:22px 0;border-top:1px solid var(--ink);border-bottom:1px solid var(--ink)}}
.rp__metaLabel{{font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid);margin-bottom:8px}}
.rp__metaName{{font-family:var(--display);font-size:18px;line-height:1.1;color:var(--ink);margin-bottom:6px}}
.rp__metaLine{{font-family:var(--sans);font-weight:500;font-size:13px;color:var(--ink-soft);line-height:1.55}}
.rp__metaMono{{font-family:var(--mono);font-weight:500;font-size:11px;color:var(--ink-soft);letter-spacing:.04em;margin-top:4px}}
.rp__metaMono .k{{color:var(--mid);letter-spacing:.18em;text-transform:uppercase;font-size:9.5px;margin-right:6px}}
.rp__summary{{margin-top:32px;display:grid;grid-template-columns:1fr 1fr 1fr;gap:24px}}
.rp__sum-block{{border-top:2px solid var(--ink);padding-top:14px}}
.rp__sum-label{{font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid);margin-bottom:12px}}
.rp__kv{{display:flex;justify-content:space-between;align-items:baseline;padding:6px 0;border-bottom:1px solid #14110d18}}
.rp__kv:last-child{{border-bottom:none}}
.rp__kv .k{{font-family:var(--sans);font-weight:500;font-size:12px;color:var(--ink-soft);text-transform:capitalize}}
.rp__kv .v{{font-family:var(--display);font-style:italic;font-size:18px;color:var(--ink)}}
.rp__total{{font-family:var(--display);font-size:64px;line-height:.92;letter-spacing:-0.02em;color:var(--ink);margin-top:8px}}
.rp__total-label{{font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid);margin-bottom:4px}}
.rp__tickets{{margin-top:32px}}
.rp__tickets-head{{font-family:var(--mono);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid);padding:0 0 10px;border-bottom:1px solid var(--ink);margin-bottom:0}}
.rp__ticket{{padding:14px 0;border-bottom:1px solid #14110d22;page-break-inside:avoid}}
.rp__ticket:last-child{{border-bottom:none}}
.rp__ticket-head{{display:grid;grid-template-columns:80px 1fr auto;gap:16px;align-items:baseline}}
.rp__ticket-status{{font-family:var(--mono);font-size:9px;letter-spacing:.18em;text-transform:uppercase;padding:3px 7px;border:1px solid currentColor}}
.rp__ticket-status.open{{color:var(--red);border-color:var(--red)}}
.rp__ticket-status.closed{{color:var(--mid);border-color:var(--mid)}}
.rp__ticket-status.pending{{color:#b07d00;border-color:#b07d00}}
.rp__ticket-status.acknowledged{{color:var(--ink-soft);border-color:var(--ink-soft)}}
.rp__ticket-title{{font-family:var(--display);font-size:17px;line-height:1.1;color:var(--ink)}}
.rp__ticket-meta{{font-family:var(--mono);font-size:10px;letter-spacing:.1em;color:var(--mid);text-transform:uppercase;white-space:nowrap}}
.rp__ticket-sub{{display:flex;gap:20px;margin-top:5px;padding-left:96px}}
.rp__ticket-id{{font-family:var(--mono);font-size:10px;color:var(--mid);letter-spacing:.08em}}
.rp__ticket-date{{font-family:var(--mono);font-size:10px;color:var(--mid);letter-spacing:.08em}}
.rp__empty{{font-family:var(--display);font-style:italic;font-size:18px;color:var(--mid);padding:32px 0}}
.rp__colo .vol{{font-family:var(--display);font-style:italic;font-size:13px;letter-spacing:0;text-transform:none}}
.rp__colo .right{{display:flex;gap:16px;align-items:baseline}}
.rp__colo .sep{{opacity:.4}}
/* ── Per-ticket detail pages ── */
.rp__detail-page{{
  page-break-before:always;
  background:var(--paper);
}}
.rp__det-mast{{
  background:var(--ink);color:var(--paper);padding:16px 44px;
  display:flex;justify-content:space-between;align-items:baseline;
  font-family:var(--mono);font-size:11px;letter-spacing:.22em;text-transform:uppercase;
  flex-shrink:0;
}}
.rp__det-mast .word{{font-family:var(--display);font-size:14px;letter-spacing:.14em}}
.rp__det-pg{{font-family:var(--display);font-style:italic;font-size:14px;letter-spacing:0;text-transform:none}}
.rp__det-body{{padding:48px 44px 80px;flex:1}}
.rp__det-eyebrow{{display:flex;align-items:baseline;gap:16px;margin-bottom:20px}}
.rp__det-id{{font-family:var(--mono);font-size:11px;letter-spacing:.14em;color:var(--mid)}}
.rp__det-date{{font-family:var(--mono);font-size:11px;letter-spacing:.14em;color:var(--mid)}}
.rp__det-title{{font-family:var(--display);font-size:52px;line-height:1.0;letter-spacing:-0.02em;font-weight:400;color:var(--ink);margin-bottom:12px}}
.rp__det-meta{{font-family:var(--mono);font-size:10px;letter-spacing:.18em;text-transform:uppercase;color:var(--mid);display:flex;gap:10px}}
.rp__det-meta .sep{{opacity:.4}}
.rp__det-sub{{margin-top:14px;font-family:var(--mono);font-size:11px;color:var(--ink-soft);letter-spacing:.06em}}
.rp__det-sub-label{{color:var(--mid);font-size:9.5px;letter-spacing:.22em;text-transform:uppercase;margin-right:10px}}
.rp__det-rule{{height:1px;background:var(--ink);margin:28px 0}}
.rp__det-desc{{font-family:var(--sans);font-weight:500;font-size:14px;line-height:1.75;color:var(--ink-soft);max-width:640px}}
.rp__det-desc h1,.rp__det-desc h2,.rp__det-desc h3{{break-after:avoid;page-break-after:avoid}}
.rp__det-desc h1,.rp__det-desc h2{{font-family:var(--display);font-weight:400;font-style:italic;color:var(--ink);margin:20px 0 6px;line-height:1.1}}
.rp__det-desc h1{{font-size:28px}}
.rp__det-desc h2{{font-size:22px}}
.rp__det-desc h3{{font-family:var(--mono);font-size:10px;letter-spacing:.22em;text-transform:uppercase;color:var(--mid);margin:18px 0 6px}}
.rp__det-desc p{{margin:0 0 10px}}
.rp__det-desc p:last-child{{margin-bottom:0}}
.rp__det-desc strong{{font-weight:600;color:var(--ink)}}
.rp__det-desc em{{font-style:italic;color:var(--ink)}}
.rp__det-desc ul,.rp__det-desc ol{{padding-left:20px;margin:6px 0 10px}}
.rp__det-desc li{{margin-bottom:3px}}
.rp__det-desc code{{font-family:var(--mono);font-size:12px;background:#14110d0f;padding:1px 5px;letter-spacing:.02em}}
.rp__det-desc pre{{font-family:var(--mono);font-size:12px;background:#14110d0f;padding:12px 16px;margin:10px 0;overflow:auto}}
.rp__det-desc blockquote{{border-left:3px solid var(--red);padding-left:14px;color:var(--mid);margin:10px 0}}
.rp__det-desc hr{{border:none;border-top:1px solid var(--rule,#ddd);margin:16px 0}}
.rp__det-desc a{{color:var(--red)}}
/* Continuation bar — only visible when <thead> repeats on overflow pages */
.rp__det-cont{{
  display:flex;align-items:baseline;gap:14px;
  padding:10px 44px 10px;border-bottom:1px solid #14110d22;
  background:var(--paper);
  margin:0 0 0 0;
}}
.rp__det-cont-id{{font-family:var(--mono);font-size:10px;letter-spacing:.14em;color:var(--mid)}}
.rp__det-cont-ttl{{font-family:var(--display);font-style:italic;font-size:14px;color:var(--ink-soft)}}
@media print{{
  @page{{size:A4;margin:0}}
  html,body{{background:var(--paper)!important;padding:0}}
  .rp{{width:100%}}
  .rp__det-eyebrow{{page-break-after:avoid;break-after:avoid}}
  .rp__det-title{{page-break-after:avoid;break-after:avoid}}
  .rp__det-meta{{page-break-after:avoid;break-after:avoid}}
  .rp__det-rule{{page-break-after:avoid;break-after:avoid}}
  /* Bottom safe-zone — stop content hitting the paper edge */
  .rp__det-desc{{padding-bottom:60px}}
}}
</style>
</head>
<body>
<article class="rp">
  <header class="rp__mast">
    <span class="word">Lodgr.</span>
    <span class="period">{period}</span>
  </header>
  <section class="rp__body">
    <div class="rp__head">
      <div>
        <div class="rp__refLabel">Support Report</div>
        <div class="rp__refVal">{client_name_esc}</div>
      </div>
      <div class="rp__stamp">Generated · {generated}</div>
    </div>
    <h1 class="rp__title">A month <em>on paper.</em></h1>
    <div class="rp__strap">Monthly report · {period}</div>
    <div class="rp__meta">
      <div>
        <div class="rp__metaLabel">Client</div>
        <div class="rp__metaName">{client_name_esc}</div>
        <div class="rp__metaLine">{client_email_esc}</div>
      </div>
      <div>
        <div class="rp__metaLabel">Period</div>
        <div class="rp__metaName">{period}</div>
        <div class="rp__metaMono"><span class="k">Generated</span>{generated}</div>
      </div>
      <div>
        <div class="rp__total-label">Tickets this period</div>
        <div class="rp__total">{total:02}</div>
      </div>
    </div>
    <div class="rp__summary">
      <div class="rp__sum-block">
        <div class="rp__sum-label">— By status</div>
        {by_status}
      </div>
      <div class="rp__sum-block">
        <div class="rp__sum-label">— By priority</div>
        {by_priority}
      </div>
      <div class="rp__sum-block">
        <div class="rp__sum-label">— By type</div>
        {by_type}
      </div>
    </div>
    <div class="rp__tickets" style="margin-top:36px">
      <div class="rp__tickets-head">— Tickets</div>
      {ticket_rows}
      {empty_state}
    </div>
  </section>
  <footer class="rp__colo">
    <span class="vol">Monthly Support Report · {period}</span>
    <span class="right">
      <span>{client_name_esc}</span><span class="sep">·</span>
      <span>lodgr.app</span>
    </span>
  </footer>
</article>
{detail_pages}
<script>
  if (typeof marked !== 'undefined') {{
    document.querySelectorAll('.rp__det-desc').forEach(function(el) {{
      el.innerHTML = marked.parse(el.textContent || '');
    }});
  }}
</script>
</body>
</html>"#,
        period = html_esc(&period),
        client_name_esc = html_esc(client_name),
        client_email_esc = html_esc(client_email),
        generated = html_esc(&generated),
        total = total,
        by_status = stat_rows(&by_status),
        by_priority = stat_rows(&by_priority),
        by_type = stat_rows(&by_type),
        ticket_rows = ticket_rows,
        empty_state = empty_state,
        detail_pages = detail_pages,
    )
}
