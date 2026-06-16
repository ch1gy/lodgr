use lettre::{
    message::{header::ContentType, Mailbox},
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

        let email = match Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body)
        {
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

        let email = match Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject("Your support access link")
            .header(ContentType::TEXT_PLAIN)
            .body(body)
        {
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
