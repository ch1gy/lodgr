use lettre::{
    message::{Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::config::{Config, SmtpTls};

#[derive(Clone)]
pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    pub fn from_config(config: &Config) -> Option<anyhow::Result<Self>> {
        let host = config.smtp_host.as_deref()?;
        let user = config.smtp_user.clone().unwrap_or_default();
        let password: String = config
            .smtp_password
            .as_ref()
            .map(|z| z.as_str().to_owned())
            .unwrap_or_default();
        let from_addr = config.smtp_from.clone().unwrap_or_else(|| user.clone());

        Some((|| {
            let from = build_mailbox("Lodgr Support", &from_addr)
                .map_err(|e| anyhow::anyhow!("invalid SMTP_FROM address: {e}"))?;

            let transport = match config.smtp_tls {
                SmtpTls::Starttls => {
                    let creds = Credentials::new(user, password);
                    AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
                        .port(config.smtp_port)
                        .credentials(creds)
                        .build()
                }
                SmtpTls::None => {
                    tracing::warn!(
                        "SMTP_TLS=none — plaintext SMTP, local development only. \
                         Never use this in production."
                    );
                    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
                        .port(config.smtp_port);
                    if !user.is_empty() {
                        builder = builder.credentials(Credentials::new(user, password));
                    }
                    builder.build()
                }
            };

            Ok(SmtpMailer { transport, from })
        })())
    }

    /// Send a ticket lifecycle notification. The body intentionally contains
    /// only the ticket title and a login prompt — never any encrypted content.
    pub async fn send_ticket_notification(
        &self,
        to_email: &str,
        to_name: &str,
        ticket_title: &str,
        event: TicketEvent,
    ) {
        let subject = event.subject(ticket_title);
        let body = event.body(to_name, ticket_title);

        let to = match build_mailbox(to_name, to_email) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    recipient = %mask_email(to_email),
                    "skipping ticket notification — could not build recipient mailbox: {e}"
                );
                return;
            }
        };

        let html = render_html(
            event.heading(),
            &event.html_paragraphs(to_name, ticket_title),
            None,
        );

        let email = match Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(body))
                    .singlepart(SinglePart::html(html)),
            ) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("failed to build ticket notification email: {e}");
                return;
            }
        };

        if let Err(e) = self.transport.send(email).await {
            tracing::warn!(recipient = %mask_email(to_email), "failed to send email: {e}");
        }
    }

    /// Send a magic link. The URL is always included so the desk can also
    /// copy it manually and deliver via any channel (WhatsApp, SMS, etc.).
    pub async fn send_magic_link(&self, to_email: &str, to_name: &str, magic_url: &str) {
        let body = format!(
            "Hello {to_name},\n\n\
             A support access link has been generated for you:\n\n\
             {magic_url}\n\n\
             This link expires in 1 hour and can only be used once.\n\n\
             If you did not request this, you can safely ignore it."
        );

        let to = match build_mailbox(to_name, to_email) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    recipient = %mask_email(to_email),
                    "skipping magic link email — could not build recipient mailbox: {e}"
                );
                return;
            }
        };

        let paragraphs = vec![
            format!("Hello {to_name},"),
            "A support access link has been generated for you.".to_owned(),
            "This link expires in 1 hour and can only be used once.".to_owned(),
            "If you did not request this, you can safely ignore it.".to_owned(),
        ];
        let html = render_html(
            "Your support access link",
            &paragraphs,
            Some(("Open support link", magic_url)),
        );

        let email = match Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject("Your support access link")
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(body))
                    .singlepart(SinglePart::html(html)),
            ) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("failed to build magic link email: {e}");
                return;
            }
        };

        if let Err(e) = self.transport.send(email).await {
            tracing::warn!(recipient = %mask_email(to_email), "failed to send magic link email: {e}");
        }
    }

    /// Notify a user (desk or client) of a successful password login on their
    /// own account. Best-effort security alert — never blocks or fails the
    /// login itself.
    pub async fn send_login_alert(
        &self,
        to_email: &str,
        to_name: &str,
        role: &str,
        ip: &str,
        when: &str,
    ) {
        let body = format!(
            "Hello {to_name},\n\n\
             Your {role} account was just signed in to.\n\n\
             Time: {when}\n\
             IP address: {ip}\n\n\
             If this wasn't you, change your password immediately and contact \
             support."
        );

        let to = match build_mailbox(to_name, to_email) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    recipient = %mask_email(to_email),
                    "skipping login alert — could not build recipient mailbox: {e}"
                );
                return;
            }
        };

        let paragraphs = vec![
            format!("Hello {to_name},"),
            format!("Your {role} account was just signed in to."),
            format!("Time: {when}"),
            format!("IP address: {ip}"),
            "If this wasn't you, change your password immediately and contact support.".to_owned(),
        ];
        let html = render_html("New sign-in", &paragraphs, None);

        let email = match Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(format!("New sign-in to your {role} account"))
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(body))
                    .singlepart(SinglePart::html(html)),
            ) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("failed to build login alert email: {e}");
                return;
            }
        };

        if let Err(e) = self.transport.send(email).await {
            tracing::warn!(recipient = %mask_email(to_email), "failed to send login alert: {e}");
        }
    }
}

/// Build a lettre `Mailbox` from a display name and email address.
/// Returns `Err` (never panics) if either field is unparsable — e.g. when
/// `email` is still ciphertext hex (regression guard for BUG-1).
fn build_mailbox(name: &str, email: &str) -> Result<Mailbox, lettre::address::AddressError> {
    format!("{name} <{email}>").parse()
}

pub fn mask_email_pub(email: &str) -> String {
    mask_email(email)
}

/// Render a branded HTML email matching the app's editorial cream/ink/red
/// palette (see frontend/src/styles/tokens.css). Inline styles only — most
/// mail clients strip <style> blocks.
fn render_html(heading: &str, paragraphs: &[String], cta: Option<(&str, &str)>) -> String {
    const CREAM: &str = "#f2ede4";
    const INK: &str = "#0d0d0d";
    const INK_SOFT: &str = "#3d3830";
    const MID: &str = "#6a6560";
    const RULE: &str = "#c8c2b8";
    const RED: &str = "#c8322a";

    let body_html = paragraphs
        .iter()
        .map(|p| {
            format!(
                "<p style=\"margin:0 0 16px;font-family:Georgia,'Times New Roman',serif;\
                 font-size:15px;line-height:1.55;color:{INK_SOFT};\">{}</p>",
                html_escape(p)
            )
        })
        .collect::<String>();

    let cta_html = match cta {
        Some((label, url)) => format!(
            "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:8px 0 24px;\">\
               <tr><td style=\"background:{RED};\">\
                 <a href=\"{url}\" style=\"display:inline-block;padding:12px 28px;font-family:Georgia,serif;\
                    font-size:14px;font-weight:bold;color:{CREAM};text-decoration:none;letter-spacing:0.02em;\">\
                   {label}\
                 </a>\
               </td></tr>\
             </table>\
             <p style=\"margin:0 0 24px;font-family:'Courier New',monospace;font-size:11px;\
                color:{MID};word-break:break-all;\">{url}</p>",
            label = html_escape(label),
        ),
        None => String::new(),
    };

    format!(
        "<!doctype html>\
        <html><body style=\"margin:0;padding:0;background:{CREAM};\">\
        <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" \
          style=\"background:{CREAM};padding:32px 16px;\">\
          <tr><td align=\"center\">\
            <table role=\"presentation\" width=\"560\" cellpadding=\"0\" cellspacing=\"0\" \
              style=\"max-width:560px;background:{CREAM};\">\
              <tr><td style=\"padding-bottom:24px;border-bottom:1px solid {RULE};\">\
                <span style=\"font-family:Georgia,'Times New Roman',serif;font-style:italic;\
                  font-size:22px;color:{INK};\">Lodgr<span style=\"color:{RED};\">.</span></span>\
              </td></tr>\
              <tr><td style=\"padding-top:28px;padding-bottom:4px;\">\
                <h1 style=\"margin:0 0 20px;font-family:Georgia,'Times New Roman',serif;\
                  font-size:20px;font-weight:normal;color:{INK};\">{heading}</h1>\
                {body_html}\
                {cta_html}\
              </td></tr>\
              <tr><td style=\"padding-top:8px;border-top:1px solid {RULE};\">\
                <p style=\"margin:16px 0 0;font-family:'Courier New',monospace;font-size:10px;\
                  letter-spacing:0.06em;text-transform:uppercase;color:{MID};\">Lodgr Support</p>\
              </td></tr>\
            </table>\
          </td></tr>\
        </table>\
        </body></html>",
        heading = html_escape(heading),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) => {
            let first = local.chars().next().unwrap_or('?');
            format!("{first}***@{domain}")
        }
        None => "***".to_owned(),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TicketEvent {
    Created,
    Acknowledged,
    Pending,
    Closed,
    Reopened,
    NewMessage,
}

impl TicketEvent {
    fn subject(self, title: &str) -> String {
        match self {
            Self::Created => format!("[Support] New ticket: {title}"),
            Self::Acknowledged => format!("[Support] Ticket acknowledged: {title}"),
            Self::Pending => format!("[Support] Ticket awaiting your response: {title}"),
            Self::Closed => format!("[Support] Ticket resolved: {title}"),
            Self::Reopened => format!("[Support] Ticket reopened: {title}"),
            Self::NewMessage => format!("[Support] New message on: {title}"),
        }
    }

    fn heading(self) -> &'static str {
        match self {
            Self::Created => "New support ticket",
            Self::Acknowledged => "Ticket acknowledged",
            Self::Pending => "Awaiting your response",
            Self::Closed => "Ticket resolved",
            Self::Reopened => "Ticket reopened",
            Self::NewMessage => "New message",
        }
    }

    fn html_paragraphs(self, name: &str, title: &str) -> Vec<String> {
        let action = match self {
            Self::Created => "A new support ticket has been opened.",
            Self::Acknowledged => "Your ticket has been acknowledged by the support team.",
            Self::Pending => "Your ticket is awaiting additional information from you.",
            Self::Closed => "Your ticket has been resolved.",
            Self::Reopened => "Your ticket has been reopened and is now active again.",
            Self::NewMessage => "There is a new message on your ticket.",
        };
        vec![
            format!("Hello {name},"),
            action.to_owned(),
            format!("Ticket: {title}"),
            "Log in to view the full details.".to_owned(),
        ]
    }

    fn body(self, name: &str, title: &str) -> String {
        let action = match self {
            Self::Created => "A new support ticket has been opened".to_owned(),
            Self::Acknowledged => {
                "Your ticket has been acknowledged by the support team".to_owned()
            }
            Self::Pending => "Your ticket is awaiting additional information from you".to_owned(),
            Self::Closed => "Your ticket has been resolved".to_owned(),
            Self::Reopened => "Your ticket has been reopened and is now active again".to_owned(),
            Self::NewMessage => "There is a new message on your ticket".to_owned(),
        };
        format!(
            "Hello {name},\n\n{action}.\n\nTicket: {title}\n\n\
             Log in to view the full details.\n\n\
             — Support Team"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::build_mailbox;

    #[test]
    fn valid_mailbox_parses() {
        assert!(build_mailbox("Alice", "alice@example.com").is_ok());
    }

    #[test]
    fn ciphertext_hex_blob_is_rejected() {
        // Regression guard for BUG-1: raw AES-GCM ciphertext must never parse
        // as a valid email address and reach the SMTP transport.
        let hex_blob = "c61baccf48524829731acd62d8f63f1511d20251cedba7191bae8e1406a3b61d7eb";
        assert!(
            build_mailbox("Client", hex_blob).is_err(),
            "ciphertext hex should not parse as a valid email"
        );
    }

    #[test]
    fn name_with_angle_brackets_does_not_panic() {
        // Malformed names must not panic — build_mailbox returns Err gracefully.
        let _ = build_mailbox("Bad <Name>", "user@example.com");
    }

    #[test]
    fn name_with_newline_does_not_panic() {
        let _ = build_mailbox("Bad\nName", "user@example.com");
    }
}
