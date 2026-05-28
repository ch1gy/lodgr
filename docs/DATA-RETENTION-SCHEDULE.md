# Data Retention Schedule

> **Disclaimer:** This document reflects Lodgr's best-effort implementation of data
> privacy principles inspired by the Swiss Federal Act on Data Protection (FADP / nDSG,
> in force 1 September 2023) and the Kenya Data Protection Act 2019. It does not
> constitute formal legal compliance certification. Consult a qualified data protection
> lawyer for formal compliance assessment. Deployers are responsible for adapting this
> schedule to their own jurisdiction.

---

All retention periods and deletion mechanisms listed here are implemented in the
Lodgr backend codebase. References to source files are provided so the implementation
can be verified.

---

| Data Type | Retention Period | Deletion Mechanism | Notes |
|---|---|---|---|
| **User accounts** (name, email, password hash, role, created_at) | Until deletion request, or automatically 30 days after soft deletion | Background task `hard_delete_expired_users` in `tasks.rs` runs daily. Soft delete sets `deleted_at`; hard delete cascades all child rows in a single transaction | An export is generated before hard deletion (both on the manual desk-initiated path and the automatic background path) |
| **Ticket content** (title, description, status, priority, category, dates, type) | Deleted with the user account (cascade) | Same background task; `DELETE FROM tickets WHERE client_id = ?` inside the cascade transaction | Soft-deleted tickets are hidden from clients but visible to the desk during the recovery window |
| **Message threads** (encrypted body, sender, timestamp) | Deleted with the ticket (cascade) | `DELETE FROM thread_entries WHERE ticket_id IN (SELECT id FROM tickets WHERE client_id = ?)` | Bodies are encrypted with AES-256-GCM; nonces are stored alongside ciphertext. Deletion is of the ciphertext |
| **File attachments** | Deleted with the ticket | `tokio::fs::remove_dir_all("uploads/{ticket_id}")` called after the DB cascade transaction commits | Stored in `uploads/<ticket_id>/` on the server filesystem. If the ticket is deleted without attachments, no directory exists and the remove is silently ignored |
| **Internal desk notes** (encrypted body, author, timestamp) | Deleted with the ticket (cascade) | `DELETE FROM internal_notes WHERE ticket_id IN (...)` | Not visible to clients; encrypted at rest. Not included in client data exports |
| **Sessions** (SHA-256 token hash, expiry, type) | Expired and revoked sessions purged on startup and every 24 hours | `db::sessions::delete_expired_and_revoked` called at startup and by a daily background loop in `main.rs` | Raw refresh tokens are never stored. All sessions for a user are wiped on password change, soft delete, and refresh token reuse detection |
| **Magic links** (SHA-256 token hash, scope, expiry) | Consumed on first use; expire after configured TTL (default 1 hour via `MAGIC_LINK_TTL_SECS`) | `db::magic_links::mark_used` on exchange; generating a new link for the same user/scope deletes all prior unused links | Raw tokens are never stored. Outstanding unexchanged links capped at 1 per user/scope |
| **Log files** (user IDs, IP addresses, event types, timestamps) | 30 days | Daily rotation with a maximum of 30 log files, enforced by `tracing-appender` rolling file appender configured in `main.rs` | Files are in `logs/`. Contain structured JSON lines with IP addresses and email addresses on auth failure events. No purge mechanism exists beyond the 30-file cap |
| **Export files** (full decrypted client data as JSON) | Maximum 24 hours; deleted immediately on download | RAII `DeleteOnDrop` guard fires on download or panic; hourly `tasks::clean_old_exports()` removes any file older than 24 hours from `exports/` | Export files contain plaintext client data. The `client_exports` DB table retains a record of the export event (ID, client ID, timestamp) permanently as an audit trail — only the file on disk is deleted |
| **Audit events** | 30 days (embedded in log files) | Log rotation (see log files row) | No permanent queryable audit table exists in v1. Deletion events, export events, and auth events are logged to structured log files only. After 30 days, this audit trail is gone |
| **Export event records** (`client_exports` table) | Retained indefinitely | Not automatically purged | The DB record (not the file) is kept as proof that an export was taken before deletion. Does not contain the exported data itself |

---

## Implementation references

| Mechanism | Source file |
|---|---|
| Soft delete | `services/admin.rs::soft_delete_client` |
| Hard delete (manual, desk-initiated) | `services/admin.rs::hard_delete_client` |
| Hard delete (automatic, 30-day expiry) | `tasks.rs::hard_delete_expired_users` |
| Session cleanup | `db/sessions.rs::delete_expired_and_revoked`; `main.rs` startup + daily loop |
| Log rotation config | `main.rs` — `tracing_appender::rolling::RollingFileAppender` with `max_log_files(30)` |
| Export RAII deletion | `routes/admin.rs::get_export_file` — `DeleteOnDrop` struct |
| Export hourly cleanup | `tasks.rs::clean_old_exports` — removes files older than 24 hours from `exports/` |
| Upload directory cleanup | `routes/tickets.rs::delete` and `services/admin.rs::hard_delete_client` — `tokio::fs::remove_dir_all` |
