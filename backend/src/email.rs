use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::config::Config;

#[derive(Clone)]
pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl SmtpMailer {
    pub fn from_config(config: &Config) -> Option<anyhow::Result<Self>> {
        let host = config.smtp_host.as_deref()?;
        let user = config.smtp_user.clone().unwrap_or_default();
        let password = config.smtp_password.clone().unwrap_or_default();
        let from = config.smtp_from.clone().unwrap_or_else(|| user.clone());

        Some((|| {
            let creds = Credentials::new(user, password);
            let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
                .port(config.smtp_port)
                .credentials(creds)
                .build();
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

        let email = match Message::builder()
            .from(self.from.parse().unwrap())
            .to(format!("{to_name} <{to_email}>").parse().unwrap())
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body)
        {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("failed to build email: {e}");
                return;
            }
        };

        if let Err(e) = self.transport.send(email).await {
            tracing::warn!("failed to send email to {to_email}: {e}");
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

        let email = match Message::builder()
            .from(self.from.parse().unwrap())
            .to(format!("{to_name} <{to_email}>").parse().unwrap())
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
            tracing::warn!("failed to send magic link email to {to_email}: {e}");
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TicketEvent {
    Created,
    Acknowledged,
    Pending,
    Closed,
    NewMessage,
}

impl TicketEvent {
    fn subject(self, title: &str) -> String {
        match self {
            Self::Created => format!("[Support] New ticket: {title}"),
            Self::Acknowledged => format!("[Support] Ticket acknowledged: {title}"),
            Self::Pending => format!("[Support] Ticket awaiting your response: {title}"),
            Self::Closed => format!("[Support] Ticket resolved: {title}"),
            Self::NewMessage => format!("[Support] New message on: {title}"),
        }
    }

    fn body(self, name: &str, title: &str) -> String {
        let action = match self {
            Self::Created => "A new support ticket has been opened".to_owned(),
            Self::Acknowledged => "Your ticket has been acknowledged by the support team".to_owned(),
            Self::Pending => {
                "Your ticket is awaiting additional information from you".to_owned()
            }
            Self::Closed => "Your ticket has been resolved".to_owned(),
            Self::NewMessage => "There is a new message on your ticket".to_owned(),
        };
        format!(
            "Hello {name},\n\n{action}.\n\nTicket: {title}\n\n\
             Log in to view the full details.\n\n\
             — Support Team"
        )
    }
}
