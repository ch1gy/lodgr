# Changelog

All notable changes to this project will be documented in this file.

---

## [Unreleased] — L3 fix: mask recipient email in SMTP failure logs

### Backend (`backend/src/email.rs`)
Both SMTP send-failure log lines (`send_ticket_notification` and `send_magic_link`)
previously logged the full recipient email address in plaintext. Replaced with a
masked form via a new private `mask_email` helper: `alice@example.com` →
`a***@example.com`. The domain and first character are retained for diagnostic
value; the full address no longer appears in log files. Closes L3 from the Phase 2
OWASP review.

---

## [Unreleased] — Phase 3 CI pipeline

### CI — automated build, lint, and security audit

#### `.github/workflows/ci.yml` (new file)
GitHub Actions workflow triggered on push and pull requests to `master`.
Runs on `ubuntu-latest` from the workspace root (where `Cargo.lock` lives).
Steps: `cargo fmt --check`, `cargo clippy --deny warnings`, `cargo test`,
`cargo audit`. The audit step explicitly ignores `RUSTSEC-2023-0071` (Marvin
Attack on the `rsa` crate — already documented as no-risk in this project; `rsa`
is a transitive dep of `sqlx-mysql` and the affected code path is never reachable).
Cargo registry and build artifacts are cached keyed on `Cargo.lock` hash.
Closes H5 from the Phase 2 OWASP review.

---

## [Unreleased] — Phase 2 FADP documentation

### Docs — five compliance documents in new `/docs` directory

#### `docs/PRIVACY-NOTICE.md` (new file)
Client-facing data processing notice. Covers: what data Lodgr stores (name, email,
ticket content, encrypted message threads, attachments, session info, login events),
why it is stored, how long it is kept (30 days post soft-delete for accounts; 30
days log rotation; 24h for export files), who can access it, SMTP transfer note,
and all data subject rights. Includes deployer fill-in placeholders for contact
details. Carries the multi-jurisdiction disclaimer (FADP / Kenya DPA).

#### `docs/ROPA.md` (new file)
Internal Record of Processing Activities. Five processing activities documented in a
structured table: ticket management, authentication, audit logging, email
notifications, data export. Each row covers data categories, data subjects, legal
basis, retention period, recipients, and cross-border transfer risk. SMTP provider
row flags the transfer risk and requires deployer action to name their actual
provider. Carries the disclaimer.

#### `docs/DATA-SUBJECT-REQUESTS.md` (new file)
Process document for handling data subject rights requests. Covers all four rights
(access, rectification, erasure, portability), explains what happens technically for
each one (export via `POST /admin/clients/:id/export`, profile update via
`PATCH /admin/clients/:id`, soft/hard delete flow), gives response time guidance
(30 days), provides a copy-paste client request email template, and includes
desk-operator handling instructions. Carries the disclaimer.

#### `docs/INCIDENT-RESPONSE.md` (new file)
One-page incident response plan. Covers what counts as a reportable breach, a
five-step response procedure (contain, assess, document, notify clients, report to
authority), jurisdiction-specific guidance for Switzerland (FDPIC, 72-hour window
under nFADP Art. 24) and Kenya (ODPC), and a reference to Lodgr's built-in
detection capabilities: structured audit logs, refresh token reuse detection, account
lockout events. Carries the disclaimer.

#### `docs/DATA-RETENTION-SCHEDULE.md` (new file)
Complete retention schedule with source file references for every deletion mechanism
in the codebase. Covers: user accounts, tickets, message threads, attachments,
internal notes, sessions, magic links, log files, export files, audit events, and
export event records. Each row links the retention period to the specific task,
function, or struct that enforces it. Carries the disclaimer.

---

## [Unreleased] — v1 completion (FADP gap closures)

### Backend — five items from Phase 3 FADP assessment

#### 1. `hard_delete_expired_users` now ensures export exists before deleting (`backend/src/tasks.rs`)
The background 30-day expiry task called `cascade_hard_delete_user` directly with no
export guard — a silent divergence from the manual `hard_delete_client` path, which
requires a prior export. The task now mirrors that guard: it checks
`db::exports::exists_for_client` before proceeding. If no export exists it calls
`export_client` first; if that fails the user is skipped and a warning is logged.
Both the export-generated and deleted events are logged with `user_id`.

#### 2. `PATCH /admin/clients/:id` — client profile update (`backend/src/routes/admin.rs`, `backend/src/services/admin.rs`, `backend/src/db/users.rs`)
New desk-only endpoint accepting `{ name?: string, email?: string }`. Either or both
fields may be omitted; omitting both is a no-op that returns the current state.
Applies the same email format validation used at client creation. Email uniqueness is
enforced at the DB level and mapped to a `409 Conflict`. Returns the updated
`ClientResponse`. Change is logged with `desk_user_id` and `client_id`.
New DB helper: `db::users::update_profile(pool, user_id, name, email)`.

#### 3. Export includes client profile fields (`backend/src/services/export.rs`)
`ExportDocument` previously contained `client_id` (a UUID) but not the client's
human-readable identity. Added `client_name`, `client_email`, and `client_created_at`
fields populated from the user record already fetched at the top of `export_client`.
An export without profile data was incomplete for FADP data-portability purposes.

#### 4. `GET /auth/me` — authenticated profile endpoint (`backend/src/auth.rs`, `backend/src/dto.rs`)
New endpoint available to any authenticated user (desk or client, full or scoped
session). Returns `{ id, name, email, role, created_at }` — no sensitive fields
(no password hash, no failed_attempts, no locked_until). Clients previously had no
way to retrieve their own profile data from the API. New DTO: `MeResponse`.

#### 5. Migration 011: drop notifications table (`backend/migrations/011_drop_notifications.sql`)
The notifications table has received no writes since the L6 fix (Phase 3) removed
the DB write path from `notify.rs`. It existed only as dead schema that accumulated
cascade-delete compatibility. Dropping it removes audit confusion without affecting
any live functionality. All cascade-delete SQL that referenced the table is preserved
for forward compatibility with databases that haven't yet run this migration.

---

## [Unreleased] — Documentation update

### README.md
- Fixed Argon2id params table: password hashing now correctly shows 64 MiB / 3 iter / 4-thread (was 19 MiB / 2 iter / 1-thread — stale from before M12 explicit-params fix)
- Fixed `ENCRYPTION_SALT` generation command in Setup and env vars table: `openssl rand -hex 32` (was `-hex 16`)
- Added minimum salt length note to env vars table (32 hex chars / 16 bytes)
- Fixed audit logging section: magic link exchange logs `link_id`, not `token_hash`
- Fixed `notify.rs` description in project structure: "log-only dispatch" (DB write removed in L6)
- Updated `tasks.rs` description to include stale export cleanup task
- Updated first-run behaviour list to include hourly export cleanup task
- Added `GET /health` curl example with 503 degraded case
- Added `GET /uploads/:ticket_id/:filename` file download curl example
- Added attachment extension allowlist to the "Post with attachment" example
- Added export 60-second rate limit note to export curl example
- Added magic link revocation note to both magic link curl examples
- Fixed `recurring_interval_days` in Ticket Fields Reference: `1–365` (was `integer ≥ 1`)
- Updated Password Rules: Unicode char count clarification, 101 common passwords (was 100)
- Added `zeroize 1` to the backend stack table
- Updated Argon2id explanation text to match new parameters

### notes.md
- Full rewrite — removed stale "Next Session" items (all completed), updated security
  posture to reflect all three security passes, moved all resolved findings to the
  completed-items history section, updated architecture notes

### PLANNED.md
- Marked `GET /health`, file download, `spawn_blocking`, and background task restart
  as ✅ Done in the smaller items table
- Added security hardening backlog table for the remaining Phase 2 open findings
- Added CI pipeline to the open items table

---

## [Unreleased] — Phase 3 security fixes

### Backend — eleven findings from Phase 2 OWASP review (second batch)

#### H3. Magic link creation invalidates prior unconsumed links (`backend/src/db/magic_links.rs`, `backend/src/services/magic.rs`)
No limit existed on outstanding unconsumed links per user. Generating a new
link now deletes all prior unused links for the same `(user_id, scope,
ticket_id)` combination before inserting the new one — capping outstanding
links to 1 per user/scope. New DB helper: `delete_unused_for_user_scope`.

#### M1. Password length check counts Unicode chars, not bytes (`backend/src/services/auth.rs`)
`password.len()` counted UTF-8 bytes. A password of 8 multi-byte characters
(e.g. emoji) passed the minimum; a 128-char password with multi-byte chars
could be incorrectly rejected. Both checks changed to `password.chars().count()`.

#### M5. `/health` DB ping cached with 10 s TTL (`backend/src/routes/health.rs`)
Every unauthenticated health request previously hit the DB connection pool.
A flood of health probes could exhaust all 10 connections. Added
`HEALTH_DB_OK: AtomicBool` + `HEALTH_LAST_CHECK_SECS: AtomicU64` globals
— the DB ping fires at most once every 10 seconds. Concurrent requests
within the TTL return the cached result without touching the pool.

#### M6. Export generation rate-limited to 1 per 60 s per client (`backend/src/db/exports.rs`, `backend/src/routes/admin.rs`, `backend/src/error.rs`)
Export files contain fully decrypted plaintext. No limit prevented rapid
repeat generation. Added `db::exports::recent_export_exists(pool, client_id, 60)`
— checked at the start of `export_client`; returns `429 Too Many Requests`
if an export was created within the last 60 seconds for that client.
Added `AppError::TooManyRequests(String)` variant mapping to `HTTP 429`.

#### M12. Argon2id parameters made explicit (`backend/src/services/auth.rs`)
`Argon2::default()` relied on crate defaults that could change across
versions without a code-visible signal. All password-hashing calls now
construct `Argon2::new(Algorithm::Argon2id, Version::V0x13,
Params::new(65_536, 3, 4, None))` explicitly (64 MiB, 3 iterations,
4-thread parallelism). Verification uses the PHC-embedded params and is
unaffected.

#### M13. KDF salt documentation and minimum-length note (`/.env.example`)
Recommendation to use `openssl rand -hex 32` (was `-hex 16`) — gives 32
bytes (256 bits), exceeding the validated 16-byte minimum while matching
OWASP guidance for KDF salts. Added explicit "minimum 32 hex characters"
note. Note: startup length validation already existed in `crypto.rs`.

#### L1. Client ownership check returns 404, not 403 (`backend/src/services/tickets.rs`, `backend/src/routes/messages.rs`)
Returning `403 Forbidden` when a client requested another user's ticket
confirmed the ticket existed. Changed to `404 Not Found` in all three
client-role ownership checks: `services/tickets.rs::get_with_thread`,
`routes/messages.rs::post_message`, and `routes/messages.rs::get_attachment`.

#### L2. `recurring_interval_days` capped at 365 (`backend/src/services/tickets.rs`)
No upper bound existed — values like 999999 were accepted. Both the
create and update paths now reject values outside `1..=365` with a clear
`400 Bad Request`.

#### L4. `ENCRYPTION_PASSPHRASE` zeroed after key derivation (`backend/src/main.rs`, `backend/Cargo.toml`)
Added `zeroize = "1"` dependency. The passphrase `String` is now wrapped in
`Zeroizing<String>` — the memory is zeroed when the variable drops at the
end of the `enc_key` block, before the server starts accepting connections.

#### L5. `jwt_secret` and `smtp_password` use `Zeroizing<String>` (`backend/src/config.rs`, `backend/src/email.rs`)
Both secrets were held as plain `String` fields in `Config` for the full
process lifetime. Changed to `Zeroizing<String>` and
`Option<Zeroizing<String>>` respectively — zeroed on drop whenever a
`Config` clone is released (i.e. after each request).

#### L6. Notification dead write path removed (`backend/src/notify.rs`, `backend/src/db/`, `backend/src/models.rs`)
Notifications were written to the DB on every ticket/message event but
never read by any endpoint — the table accumulated indefinitely. Removed
`db/notifications.rs` and `pub mod notifications` from `db/mod.rs`.
`notify::notify` is now a synchronous log-only function (no pool argument,
no DB insert). The `Notification` model struct removed from `models.rs`.
Three call sites in `services/tickets.rs` and `services/messages.rs`
updated. Cascade `DELETE FROM notifications` SQL in delete handlers is
retained — the table still exists and is cleaned up on ticket/user deletion.

---

## [Unreleased] — Phase 2 security fixes

### Backend — eleven findings from Phase 2 OWASP security review

#### H1. Soft-deleted ticket files now hidden from clients (`backend/src/routes/messages.rs`)
`get_attachment` checked ownership but not `ticket.deleted_at`. A client whose
ticket was soft-deleted could still download all attachments. Added a
`deleted_at` check after the ownership check — clients get `404 Not Found`
for attachments on deleted tickets.

#### H2. JWT validation pins algorithm and requires `sub` + `exp` (`backend/src/middleware.rs`)
`Validation::default()` did not pin the algorithm and did not require `sub`.
Replaced with `Validation::new(Algorithm::HS256)` and
`set_required_spec_claims(&["sub", "exp"])`.

#### H4. `dummy_hash()` no longer panics on Argon2 failure (`backend/src/services/auth.rs`)
`DUMMY_HASH` changed from `OnceLock<String>` to `OnceLock<Option<String>>`.
Init now uses `.ok()` instead of `.expect()`. If Argon2 fails, `dummy_hash()`
returns `None`, the login path falls back gracefully, and `dummy_hash_warmup()`
logs a warning instead of crashing. Return type of `dummy_hash_warmup` changed
to `()`.

#### M2. `"changeme"` added to common-password blocklist (`backend/src/services/auth.rs`)
The default desk password was not in `COMMON_PASSWORDS`. Clients could set
their password to `"changeme"`. Added the bare string to the set.

#### M3. `token_hash` removed from all `exchange_magic_link` log lines (`backend/src/services/magic.rs`)
The SHA-256 hash of an unconsumed or recently-consumed magic link was logged
on all five paths (not-found, already-used, expired, user-deleted, success).
Replaced with `link_id` where the record is available; field removed entirely
for the not-found path.

#### M9. File upload extension allowlist; download sets `Content-Type` (`backend/src/routes/messages.rs`)
Upload: validates extension against an allowlist (pdf, png, jpg, jpeg, gif,
txt, docx, zip) and rejects anything outside it with a clear error.
Download: response now includes `Content-Type: application/octet-stream`
unconditionally, preventing browser MIME sniffing.

#### M10. `spawn_with_restart` uses exponential backoff (`backend/src/main.rs`)
A permanently-panicking task previously looped at a fixed 5-second interval
with no backoff. Escalates: 5 s → 10 s → 30 s → 60 s → 5 min (cap). A task
that runs for ≥30 s before exiting resets the failure counter.

#### M11. Background cleanup for stale export files (`backend/src/tasks.rs`, `backend/src/main.rs`)
A never-downloaded export persisted on disk indefinitely (`DeleteOnDrop` only
fires during a download). Added `tasks::clean_old_exports()` — scans
`exports/` hourly, removes any file older than 24 h. Wired in `main.rs`
alongside the session cleanup loop.

#### L7. Safe `created_at` slice in PDF builder (`backend/src/services/reports.rs`)
`&t.created_at[..10]` panics on short or multi-byte values. Replaced with
`t.created_at.get(..10).unwrap_or(&t.created_at)`.

#### L8. PDF report shows truncation notice instead of silently dropping tickets (`backend/src/services/reports.rs`)
Tickets that overflow page 1 were silently dropped. Now appends:
`"... N ticket(s) not shown — download the full export for complete data."`

---

## [Unreleased] — Phase 1 v1 fixes

### Backend — four items closing the v1 gap

#### 1. File download endpoint (`backend/src/routes/messages.rs`, `backend/src/main.rs`)
Added `GET /uploads/:ticket_id/:filename` — the only actively broken user-facing
feature. Attachment paths were stored and returned in every `ThreadEntry`
response, but the URL pointed to nothing. The handler is `AuthUser`-gated: desk
can fetch any ticket's files; clients are restricted to tickets they own, and
scoped sessions are restricted to the scoped ticket. Both path segments are
sanitised with `.file_name()` guards and the resolved path is canonicalized and
checked against the `uploads/` root before reading — same double guard already
in use in `routes/admin.rs` and `routes/messages.rs`.

#### 2. Health endpoint (`backend/src/routes/health.rs`, `backend/src/main.rs`)
Added `GET /health` — unauthenticated. Pings the DB with `SELECT 1`. Returns
`200 { status: "ok", db: "ok", uptime_secs: N }` when healthy, `503
{ status: "degraded", db: "error" }` on DB failure. Uptime is anchored via a
`OnceLock<Instant>` initialized at server startup.

#### 3. `spawn_blocking` for PDF and export (`backend/src/services/reports.rs`, `backend/src/services/export.rs`)
Both functions were running CPU-bound work on the async executor. The PDF
builder (`printpdf` — font embedding, page layout, BufWriter flush) is now
extracted into a `build_pdf` helper called via `tokio::task::spawn_blocking`.
The export JSON serialization (`serde_json::to_string_pretty`) is also offloaded
to `spawn_blocking`. DB fetches remain async as before; only the CPU work moves
to the blocking thread pool.

#### 4. Background task restart supervisor (`backend/src/main.rs`)
Both background tasks (`recurring_tickets`, `hard_delete_expired_users`) were
spawned bare — a panic in either task would kill the loop silently with no log
and no recovery. Added a `spawn_with_restart` helper that wraps each task in an
outer supervisor loop: panics are caught via `JoinHandle` errors, logged at
`tracing::error!`, and the task is relaunched after a 5-second cooldown.
Unexpected clean exits (which shouldn't happen — both loops are infinite) are
logged at `tracing::warn!`.

---

## [Unreleased] — code-review fix pass

### Backend — seven confirmed findings from code review

#### 1. SQLite FK enforcement enabled (`backend/src/main.rs`)
Added `.foreign_keys(true)` to `SqliteConnectOptions`. SQLite disables FK
enforcement by default — every `REFERENCES` constraint in the schema was
unenforced at runtime. All child-row deletes were relying on manual cascade
SQL, with nothing at the DB level to catch an omitted table. Now FK violations
are caught by the engine.

#### 2. `hard_delete_expired_users` full cascade (`backend/src/tasks.rs`)
The background hard-delete task was calling `db::users::hard_delete` (a bare
`DELETE FROM users WHERE id = ?`) after separately deleting sessions. All
child rows — tickets, thread_entries, internal_notes, notifications,
magic_links — were left as orphaned data indefinitely. Replaced with a full
cascade transaction matching the approach in `services::admin::hard_delete_client`.
Upload directories are now cleaned up after the transaction commits.
The now-unused `db::users::hard_delete` function is dead code (warning visible
in build output) — removing it is safe but deferred.

#### 3. Upload directory cleanup on ticket delete (`backend/src/routes/tickets.rs`, `backend/src/services/admin.rs`)
The `DELETE /tickets/:id` handler deleted all child DB rows but never removed
the `uploads/<ticket_id>/` directory, leaving attachment files on disk with no
DB reference. `services::admin::hard_delete_client` had the same omission for
all of a client's tickets. Both sites now call `tokio::fs::remove_dir_all`
after their respective transactions; `NotFound` errors are silently ignored
(tickets without attachments have no directory).

#### 4. Token hash removed from magic link creation log (`backend/src/services/magic.rs`)
The `token_hash` field (the SHA-256 hash stored in the DB to look up a live
magic link) was logged at INFO on every `create_magic_link` call. This is
inconsistent with how refresh token hashes are treated elsewhere. The hash of
an unconsumed link is a sensitive lookup key; it no longer appears in logs.
The success-exchange log (link already consumed) is unchanged.

#### 5. Export crash safety — RAII drop guard (`backend/src/routes/admin.rs`)
The previous read-then-delete was not crash-safe: a process kill between
`tokio::fs::read` and `tokio::fs::remove_file` left the decrypted plaintext
export on disk with no recovery path. A `DeleteOnDrop` RAII struct now holds
the path and calls `std::fs::remove_file` synchronously on drop, firing even
on panic or task cancellation. The explicit async removal is kept for logging;
the guard silently no-ops on the already-gone file in the normal path.

#### 6. TOCTOU fix in ticket delete (`backend/src/routes/tickets.rs`)
The delete handler checked ticket existence via `find_by_id` before opening a
transaction. Two concurrent DELETE requests could both pass that check, then
serialize at the write phase — both returning `204 No Content` for the same
ticket. Removed the pre-check; the final `DELETE FROM tickets` now asserts
`rows_affected() == 0 → 404` inside the transaction, making the response
authoritative.

#### 8. `db::users::hard_delete` removed (`backend/src/db/users.rs`)
Dead code. Became unreachable when `tasks::hard_delete_expired_users` was
rewritten to use the full cascade transaction (item 2 above). The bare
`DELETE FROM users WHERE id = ?` it contained bypassed all child-row cleanup
and was the root cause of the GDPR finding. Deleted so the compiler no longer
warns and no future caller can accidentally reintroduce the unsafe path.

#### 7. Duplicate `find_by_id` eliminated (`backend/src/services/tickets.rs`)
When a desk user filed a ticket on behalf of a client (`client_id` field),
the validation block fetched the client user for role/deleted checks, then
dropped the result. The email-notification block fetched the same user again.
The validated `User` is now passed through the resolution block and re-used in
the email block, saving one DB round-trip per desk-filed ticket.

---

## [Previous] — trivial hardening pass

### Backend — trivial hardening pass

#### 1. WAL journal mode enabled (`backend/src/main.rs`)
Added `.journal_mode(SqliteJournalMode::Wal)` to `SqliteConnectOptions` at pool
creation. WAL mode allows concurrent reads and a single writer without blocking
reads during writes. It also provides better crash recovery than the default
DELETE journal. One line change; no migration needed.

#### 2. cargo audit — one finding, no action required (`RUSTSEC-2023-0071`)
Ran `cargo audit` against the full dependency tree.

**Finding: Marvin Attack — RSA timing side-channel**
- Crate: `rsa v0.9.10`
- Severity: 5.9 (MEDIUM)
- Advisory: https://rustsec.org/advisories/RUSTSEC-2023-0071
- No fixed version available.

**Risk to this project: effectively zero.**
The `rsa` crate is a transitive dependency via `sqlx-mysql`, which is pulled in
by `sqlx-macros-core` even though this project uses only the `sqlite` feature.
We do not use MySQL, RSA keys, or any code path that would exercise the affected
`rsa` crate. The code is compiled but never invoked.

**Recommendation:** Wait for sqlx to drop `sqlx-mysql` as a mandatory
`macros-core` dependency (being tracked upstream). No action needed today.
If this is a blocker for a security audit, consider pinning `sqlx` to a version
that separates mysql from the macros feature set, once available.

#### 3. COMMON_PASSWORDS → `HashSet` (`backend/src/services/auth.rs`)
Changed the common-password blocklist from a `const &[&str]` slice (O(n) linear
scan) to a `static OnceLock<HashSet<&'static str>>` (O(1) average lookup).
Initialised once on first `validate_password_strength` call. No behaviour change;
purely a performance fix for the hot path of every account-creation request.

#### 4. Connection pool size explicit (`backend/src/main.rs`)
Added `.max_connections(10)` via `SqlitePoolOptions`. Previously the pool used the
sqlx default (which is implementation-defined). Having the limit explicit and
visible in code makes capacity planning easier and prevents unbounded connection
growth if the default ever changes upstream.

#### 5. `ClientResponse` DTO exposes lockout state (`backend/src/dto.rs`)
Added `failed_attempts: i64` and `locked_until: Option<String>` fields to
`ClientResponse`. Previously the admin panel had no way to know if a client was
locked without attempting a login. The frontend admin panel can now:
- Show a 🔒 badge on locked client rows.
- Make the "Unlock" button conditionally prominent.
- No migration needed — columns already exist in the `users` table.

#### 6. Export download-and-delete (`backend/src/routes/admin.rs`)
Export files contain fully decrypted plaintext client data. Previously they
persisted on disk indefinitely after the download. Now, after reading the file
for download, `tokio::fs::remove_file` deletes it immediately. Deletion failures
are logged as errors but do not fail the download response (client already has
the data; preventing a silent loss is not possible at that point). This closes
the HIGH-severity finding from `notes.md`.

---
