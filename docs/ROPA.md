# Record of Processing Activities (ROPA)

> **Disclaimer:** This document reflects Lodgr's best-effort implementation of data
> privacy principles inspired by the Swiss Federal Act on Data Protection (FADP / nDSG,
> in force 1 September 2023) and the Kenya Data Protection Act 2019. It does not
> constitute formal legal compliance certification. Consult a qualified data protection
> lawyer for formal compliance assessment. Deployers are responsible for adapting this
> document to their own jurisdiction.
>
> **Deployer action required:** Update the SMTP provider row under "Cross-Border
> Transfer Risk" to reflect your actual email provider and where it processes data.
> If you do not use email notifications (`SMTP_HOST` is not set), remove that row or
> mark it as not applicable.

---

## Controller

**Name:** [Deployer name — fill in before use]
**Contact:** [Deployer contact email — fill in before use]
**Lodgr version:** v1

---

## Processing Activities

| Processing Purpose | Data Categories | Data Subjects | Legal Basis | Retention Period | Recipients | Cross-Border Transfer Risk |
|---|---|---|---|---|---|---|
| **Ticket management** — creating, tracking, and resolving IT support requests | Name, email, ticket title, description, status, priority, category, due dates, ticket type; message thread bodies (encrypted at rest with AES-256-GCM); file attachments | Clients (end-users of the support desk) | Performance of a support service contract | Account data: 30 days after soft deletion, then permanently auto-deleted by background task. Attachments: deleted with the ticket. Tickets: deleted with the account | Desk operator only | None if self-hosted in Switzerland or EEA. Risk arises only if the server is hosted by a cloud provider outside adequate countries |
| **Authentication** — verifying identity and maintaining sessions | Email address (login identifier); Argon2id password hash; SHA-256 refresh token hash; magic link token hash; account lockout state (failed attempt count, lockout timestamp); session metadata (created/expires timestamps, session type) | Desk operator; clients | Performance of contract (access required to use the service) | Sessions: expired and revoked sessions purged on startup and every 24 hours. Magic links: consumed on first use, expire after configured TTL (default 1 hour). Account records: per ticket management row above | Desk operator only | None — all authentication data is local to the deployment server |
| **Audit logging** — recording security-relevant events for incident detection and response | User IDs (UUID, not email), source IP addresses, event types (login success/failure, magic link creation and exchange, export, soft/hard delete, password change, token reuse detection), timestamps | Desk operator; clients | Legitimate interest — security monitoring, fraud prevention, incident response | 30 days, enforced by daily log rotation with a maximum of 30 log files (`tracing-appender` rolling file appender) | Desk operator only — log files are local to the deployment server | None — log files are local |
| **Email notifications** — informing clients of ticket lifecycle events and delivering magic link access URLs | Recipient name, recipient email address, ticket title (subject line only — no message body content is included in emails); magic link URL (one-time token embedded in URL, HTTPS delivery) | Clients | Performance of contract (notifications are a feature of the service, not a separate purpose) | Not stored by Lodgr — data is in transit only. The email provider may retain a copy per their own retention policy | SMTP provider configured by the deployer | **Yes — risk present if SMTP provider routes or processes data outside Switzerland / EEA.** Deployer must identify their provider and confirm adequacy or document appropriate safeguards. [Fill in: provider name, country of processing] |
| **Data export** — generating a portable copy of client data for data subject access requests or pre-deletion records | All data from the ticket management row above, decrypted and serialised to JSON | Client (subject of the export) | Legal obligation (data subject access / portability right); legitimate interest (pre-deletion data record) | Export files are deleted from disk immediately after download via RAII guard, and any file not downloaded is purged within 24 hours by the hourly cleanup task. The `client_exports` DB record (ID, client ID, file path, timestamp) is retained as an audit trail of the export event | Desk operator; client (download) | None — export files are local to the deployment server until downloaded |

---

## Notes

- **Password hash security:** Passwords are hashed with Argon2id (64 MiB / 3 iter / 4-thread parallelism). A compromised database does not yield usable passwords.
- **Token hash security:** Refresh tokens and magic link tokens are stored as SHA-256 hashes only. Raw tokens are never written to any server-side storage.
- **Encryption at rest:** Message thread bodies and internal desk notes are encrypted with AES-256-GCM before storage. The encryption key is derived from a passphrase at startup and never written to the database.
- **No third-party analytics:** Lodgr contains no analytics, tracking pixels, or any integration with third-party data processors beyond the optional SMTP relay.
- **Internal notes:** The desk operator can create internal notes on tickets that are not visible to clients. These are encrypted at rest but are explicitly not included in client data exports. They represent the operator's own working notes, not client personal data in the same sense.
