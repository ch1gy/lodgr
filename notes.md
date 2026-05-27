# Lodgr — Dev Notes

---

## Next Session — Start Here

The frontend is shipped. The remaining work is all backend features and one
outstanding security item.

**Highest priority:**

1. **File download endpoint** — BROKEN FEATURE. Attachments upload and paths are
   stored, but there is no `GET /uploads/:filename` route. The frontend shows
   "download n/a". Fix: a desk-auth-gated `ServeDir` or streaming handler.

2. **Client password self-service** — Backend one-liner. Change `PATCH /auth/password`
   extractor from `DeskUser` to a new `FullSessionUser`. Frontend Settings page
   already has the form built and the placeholder copy written.

3. **`GET /health`** — Trivial. Every deploy needs it.

**Then:** ticket server-side filtering, unread counts, and SSE (see PLANNED.md).

---

## Security Posture

**Rating: 9 / 10**

Frontend ships in-memory token storage (never localStorage), auto-refreshes on 401,
and runs entirely same-origin in production — no CORS surface. The two remaining
items below keep this from being a 10.

**What's strong**
- AES-256-GCM with per-entry nonces on all message bodies and internal notes
- Argon2id at correct parameters for both passwords and key derivation
- SHA-256 refresh token storage — DB breach doesn't yield usable tokens
- Refresh token rotation with theft detection (replay = all sessions wiped)
- Per-account lockout: 5 failures → exponential backoff → permanent
- Scoped magic link sessions, DeskUser requires full session
- Full security header stack (CSP, HSTS, Referrer-Policy, Permissions-Policy, etc.)
- Parameterized SQL throughout — no injection surface
- Structured audit logging on every auth and admin event
- Daily log rotation, 30-day retention
- Input validation: enums, lengths, date format, email format, common passwords
- Hard delete fully cascading in a single transaction
- Export download-and-delete — no plaintext files left on disk after download
- Frontend: in-memory token, httpOnly refresh cookie, autofill CSS overrides,
  QR codes hardcoded light-on-dark for scanner reliability

**Still open**
- No file download endpoint — attachment paths stored but files unserveable
- No cargo audit in CI (one-off run done, no CVEs affecting this project)

---

## Completed Items

### ✅ WAL journal mode
`.journal_mode(SqliteJournalMode::Wal)` on pool creation. One line. Done.

### ✅ cargo audit — RUSTSEC-2023-0071
`rsa` transitive via `sqlx-mysql`. We don't use MySQL, RSA, or any affected code
path. No action required. Re-run before any public deployment.

### ✅ COMMON_PASSWORDS → `HashSet`
`static OnceLock<HashSet<&'static str>>` — O(1) average lookup vs the old O(n)
slice scan.

### ✅ Connection pool size explicit
`.max_connections(10)` — limit is now visible in code.

### ✅ `ClientResponse` exposes lockout state
`failed_attempts` and `locked_until` added to the DTO. Frontend can show 🔒 badge
and conditional Unlock button without guessing.

### ✅ Export download-and-delete
Files deleted from disk immediately after the desk downloads them. Closes the HIGH
severity finding. DB record preserved; only the filesystem copy is removed.

### ✅ Account lockout
Migration 010. 5 failures → 1 min → 5 min → 15 min → 1 hour → permanent.
`POST /admin/clients/:id/unlock` for client accounts.
Desk DB recovery: `sqlite3 data/support.db "UPDATE users SET failed_attempts=0, locked_until=NULL WHERE email='desk@local'"`

### ✅ Frontend — full React + TypeScript SPA
All pages built: Login, Ticket List, Ticket Detail, Clients, Reports, Settings,
Magic Landing. Mobile-responsive with bottom tab bar, FAB, and phone-specific
layouts. Dark mode with View Transitions API reveal. Password generator with
passphrase and random modes.

---

## Code Quality & Dead Code

### Dead Code (backend)

**models.rs**
- `Notification` struct — never constructed. `notify()` uses a raw query; this struct is dead weight.
- `Session` — `.token_hash`, `.created_at`, `.replaced_by`, `.session_type`, `.scoped_ticket_id` never read after mapping.
- `MagicLink` — `.token_hash`, `.created_at` never read after fetch.
- `ClientExport` — `.created_at` never read.
- `User` — `.created_at` never read.
- `Ticket` — `.last_recurred_at` stored and updated but never read.

**db/tickets.rs**
- `hard_delete()` — dead. Admin service now uses inline transaction SQL.

**db/users.rs**
- `soft_delete()` and `restore()` — dead. Admin service was rewritten to inline transaction SQL. `hard_delete()` is still alive (used in `tasks.rs`).

**db/exports.rs**
- `find_latest_for_client()` — never called.

**services/magic.rs**
- `MagicLinkOutput.raw_token` — populated but never accessed by any caller.

---

### Architecture

**Broken layering in services/admin.rs**
`soft_delete_client`, `restore_client`, and `hard_delete_client` contain raw
`sqlx::query` calls in service-layer code. Correct for atomicity, wrong layer.
Simplest fix: delete the now-dead `db/` functions, accept the inline SQL.

**Notification table grows forever**
No read endpoint, no cleanup, no pagination. `notify()` inserts a row on every
ticket event. Either add `GET /notifications` or drop DB persistence entirely and
keep only the `tracing::info!` log line.

**PDF generation blocks the async thread**
`monthly_report` and large exports run synchronously. Both need
`tokio::task::spawn_blocking(|| { ... })`.

**Background tasks have no restart logic**
`recurring_tickets` and `hard_delete_expired_users` die silently on panic. Needs
a respawn loop or panic handler.

**COMMON_PASSWORDS** — fixed (now `HashSet`). ✅

**config.rs stores SMTP password as String**
`Config` is cloned into every handler. SMTP password not zeroed on drop. Acceptable
at this scale — note for a future hardening pass.

---

### Scalability

**SQLite ceiling**
Single writer, serialised writes. Fine for one desk. Migration path to Postgres is
one `Cargo.toml` change + query adjustments.

**WAL mode** — enabled. ✅

**Connection pool** — explicit `.max_connections(10)`. ✅

**In-memory rate limiter resets on restart**
Attacker who knows deploy cadence can time attempts around it. Not exploitable here.

**`GET /admin/clients` is unbounded**
Returns every client in one response. Needs pagination at real scale.
