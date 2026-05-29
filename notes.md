# Lodgr — Dev Notes

---

## Current Status (2026-05-28)

The backend is feature-complete for v1. The backend has been reviewed against the
OWASP Top 10 across three passes; findings and open items are logged in CHANGELOG.md
and PLANNED.md. The frontend is shipped. No actively broken features remain.

**What's still open (backend):**
- Client password self-service — `PATCH /auth/password` is desk-only; frontend form exists
- Ticket server-side filtering — currently fetch-all, filtered client-side
- Unread message counts — no `unread_count` on `TicketResponse`
- SSE for real-time updates — currently 30-second polling
- `GET /admin/clients` is unbounded — needs pagination at scale

See [PLANNED.md](PLANNED.md) for full detail on each.

---

## Security Posture

Reviewed against the OWASP Top 10 across three passes: Phase 1 (v1 fixes), Phase 2
(OWASP Top 10 2025 / ASVS v5.0 / API Security Top 10 2023), Phase 3 (remaining
findings from Phase 2). Open findings are tracked in PLANNED.md.

**What's strong**
- AES-256-GCM with per-entry nonces on all message bodies and internal notes
- Argon2id explicit params (64 MiB / 3 iter / 4-thread) for passwords; (64 MiB / 3 iter / 1-thread) for key derivation
- SHA-256 refresh token + magic link token storage — DB breach doesn't yield usable tokens; token hash never logged
- Refresh token rotation with theft detection (replay = all sessions wiped)
- Per-account lockout: 5 failures → exponential backoff → permanent
- Scoped magic link sessions; DeskUser requires full session; scoped JWT cannot reach admin routes
- Magic link generation revokes all prior unconsumed links (1 outstanding per user/scope)
- JWT validation pins HS256 algorithm; requires `sub` and `exp` claims
- Full security header stack (CSP, HSTS, X-Frame-Options, Referrer-Policy, Permissions-Policy, nosniff)
- Parameterized SQL throughout — no injection surface
- Structured audit logging on every auth and admin event; token hashes excluded from all log lines
- Daily log rotation, 30-day retention
- Input validation: enums, lengths, date format, email format, common passwords (101 entries including `changeme`), Unicode-correct char count, file extension allowlist
- Password length check counts Unicode code points (not bytes)
- Hard delete fully cascading in a single transaction
- Export: RAII drop guard (crash-safe deletion), download-and-delete, 60-second rate limit per client, hourly stale-file cleanup
- File downloads: auth-gated, path-traversal-safe, Content-Type locked to `application/octet-stream`
- Background tasks: exponential backoff restart (5 s → 10 s → 30 s → 60 s → 5 min); resets after stable run
- `/health` DB ping cached 10 s — connection pool protected from health-check flooding
- `ENCRYPTION_PASSPHRASE`, `jwt_secret`, and `smtp_password` zeroed in memory on drop (`Zeroizing<String>`)
- `spawn_with_restart` supervisor for all background tasks
- FK enforcement enabled at the SQLite layer (`.foreign_keys(true)`)
- WAL journal mode; explicit pool cap of 10 connections

**Still open (from Phase 2 review — not yet fixed)**
- No CI pipeline; `cargo audit` not automated (H5)
- Magic link JWTs (both full and scoped) are non-revocable; 24-hour TTL (M8)
- No CORS middleware — explicit `CorsLayer` needed for non-same-origin deploys (M7)
- `fs::read` loads entire attachment into memory; no download rate limit (M4)
- SMTP failure logs include recipient email address (L3)
- Argon2id params not explicitly set for `verify_password` (reads from PHC string — functionally correct, cosmetically inconsistent)

---

## Architecture Notes

### Layer discipline
- `db/` — SQL only, no business logic
- `routes/` — HTTP concerns only (extractors, request parsing, response shaping)
- `services/` — business logic (validation, orchestration, encryption)
- `crypto.rs` — the ONLY file that imports `aes_gcm` directly; all crypto goes through it

**Known exception:** `services/admin.rs` contains raw `sqlx::query` calls for the
cascade hard-delete transaction. This is intentional — atomicity requires a single
`tx`, which can't span two module boundaries cleanly. The dead `db/` functions that
previously duplicated this logic have been removed.

### Notifications table
Notifications are written on every ticket/message event and cascade-deleted on
ticket/user deletion, but no endpoint reads them. The DB write was removed from
`notify.rs` (L6 fix) — it now emits a structured log line only. The table still
exists in the schema (and is cleaned up in cascades); no migration to drop it has
been written yet. This is low priority.

### SQLite ceiling
Single writer, serialized writes. Fine for one desk. Migration path to Postgres is
one `Cargo.toml` change + query adjustments (mostly `?` → `$1` placeholders).

### In-memory rate limiter
Per-IP token-bucket rate limiter resets on process restart. An attacker who knows
deploy cadence can time attempts around it. Not exploitable in this deployment model.

---

## Completed Items (history)

### ✅ File download endpoint (`GET /uploads/:ticket_id/:filename`)
Auth-gated, path-traversal-safe, ownership enforced. Returns `404` (not `403`) for
client access to other users' tickets. Deleted tickets return `404` to clients.

### ✅ `GET /health`
Unauthenticated. DB ping cached 10 s via atomics to protect the connection pool.
Returns `200 { status, db, uptime_secs }` or `503 { status: "degraded" }`.

### ✅ `spawn_blocking` for PDF and export
Both CPU-bound operations (PDF generation, JSON serialization) moved off the async
executor via `tokio::task::spawn_blocking`.

### ✅ Background task restart supervisor
`spawn_with_restart` wraps all background tasks. Exponential backoff on rapid
panics; resets after stable runs (≥30 s). All background tasks use this wrapper.

### ✅ SQLite FK enforcement
`.foreign_keys(true)` on pool creation — all REFERENCES constraints now enforced.

### ✅ `hard_delete_expired_users` full cascade
Full transaction matching `services::admin::hard_delete_client`. Upload directories
cleaned after commit.

### ✅ Upload directory cleanup on ticket delete
Both `DELETE /tickets/:id` and `hard_delete_client` now remove `uploads/<ticket_id>/`
after their respective transactions.

### ✅ Token hash removed from logs
`token_hash` removed from `create_magic_link` log and all five `exchange_magic_link`
log lines. Magic link exchange logs use `link_id` instead.

### ✅ Export crash safety — RAII drop guard
`DeleteOnDrop` struct on export file path ensures removal even on panic or task
cancellation.

### ✅ TOCTOU fix in ticket delete
Pre-check removed; `rows_affected() == 0` inside the transaction is the authoritative
check.

### ✅ Duplicate `find_by_id` eliminated in ticket create
`prefetched_user` threaded from validation block to email notification block.

### ✅ `db::users::hard_delete` removed
Dead code after tasks.rs rewrite. Removed to prevent future callers from
accidentally using the unsafe bare-delete path.

### ✅ WAL journal mode
`.journal_mode(SqliteJournalMode::Wal)` on pool creation.

### ✅ `COMMON_PASSWORDS` → `HashSet`
O(1) lookup, initialized once via `OnceLock`.

### ✅ Connection pool size explicit
`.max_connections(10)`.

### ✅ `ClientResponse` exposes lockout state
`failed_attempts` and `locked_until` fields added to DTO.

### ✅ Export download-and-delete
Files deleted from disk immediately after download. DB record preserved.

### ✅ Account lockout
5 failures → exponential backoff → permanent. Desk DB recovery command documented.

### ✅ Frontend — full React + TypeScript SPA
All pages built: Login, Ticket List, Ticket Detail, Clients, Reports, Settings,
Magic Landing. Mobile-responsive, dark mode, QR code generation.

### ✅ JWT validation pins algorithm and requires claims
`Validation::new(Algorithm::HS256)` with `set_required_spec_claims(&["sub", "exp"])`.

### ✅ `dummy_hash()` non-panicking
`OnceLock<Option<String>>` with `.ok()` instead of `.expect()`. Startup logs a
warning instead of crashing.

### ✅ `"changeme"` added to common-password blocklist

### ✅ File upload extension allowlist
Upload rejects anything outside: pdf, png, jpg, jpeg, gif, txt, docx, zip.

### ✅ Download sets `Content-Type: application/octet-stream`

### ✅ Exponential backoff in `spawn_with_restart`
5 s → 10 s → 30 s → 60 s → 5 min cap. Resets after ≥30 s stable run.

### ✅ Export cleanup background task
`tasks::clean_old_exports()` runs hourly, removes export files older than 24 hours.

### ✅ Argon2id params made explicit
`Params::new(65_536, 3, 4, None)` for password hashing;
`Params::new(65_536, 3, 1, Some(32))` for key derivation.

### ✅ Export rate limit (60 s per client)
`POST /admin/clients/:id/export` returns 429 if an export exists within 60 seconds.

### ✅ Magic link outstanding link cap
Generating a new magic link deletes all prior unconsumed links for the same
user/scope/ticket. Maximum of 1 outstanding unexchanged link per user/scope.

### ✅ Password length counts Unicode chars
`.chars().count()` replaces `.len()` (byte count).

### ✅ `/health` DB ping cached
10-second TTL via atomics. Prevents connection pool exhaustion from health-check floods.

### ✅ 403 → 404 for client ownership checks
Clients requesting other users' tickets or attachments now receive 404, not 403
(which confirmed existence).

### ✅ `recurring_interval_days` capped at 365

### ✅ Secrets zeroed with `Zeroizing`
`ENCRYPTION_PASSPHRASE` (in key derivation block), `jwt_secret`, `smtp_password`.

### ✅ Notification dead write path removed
`notify.rs` now emits a structured log line only. `db/notifications.rs` deleted.
`Notification` model struct removed from `models.rs`.
