/// Emit a structured log line when a notification event occurs.
/// The DB write-side has been removed — notifications are log-only; the
/// notifications table exists for cascade-delete purposes only.
pub fn notify(recipient_id: &str, ticket_id: &str, message: &str) {
    tracing::info!(
        user_id = %recipient_id,
        ticket_id = %ticket_id,
        msg = %message,
        "notification dispatched"
    );
}
