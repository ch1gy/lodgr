# Changelog

All notable changes to this project will be documented in this file.

---

## [Unreleased] — test suite: fix broken tests + add T1–T5, F1–F2

### Backend — existing tests fixed (broken after Phase 10 encryption refactor)

The Phase 10 PII-at-rest work changed the signatures of `admin::create_client`,
`admin::update_client_profile`, and `auth::login` (all now require `enc_key` and
`email_hash_salt`), updated `db::users::create` to store ciphertext + nonce +
blind-index hash, and added `email_hash_salt` to `Config`. None of the existing
integration tests were updated alongside those changes, so the entire suite was
failing to compile. Fixed in this pass:

- **`tests/common/mod.rs`** — added `email_hash_salt` to `test_config()`; updated
  `create_test_client` and `create_test_desk` to encrypt the email field and store
  the Argon2id blind-index hash using the test key/salt.
- **`tests/auth.rs`** — threaded `enc_key` through all `auth::login` call sites.
- **`tests/admin.rs`** — updated `create_client` and `update_client_profile` calls
  with `enc_key` and `email_hash_salt`; added missing `phone` field to all
  `UpdateClientProfileInput` initialisers.
- **`tests/lockout.rs`** — added `enc_key` to `auth::login` calls; fixed
  `WHERE email = ?` → `WHERE id = ?` (email column now stores ciphertext,
  not plaintext — matching by plaintext would silently match zero rows).
- **`tests/validation.rs`** — updated the three `create_client` call sites.

### Backend — new tests (T1–T5)

#### T1 — `keep_or_reencrypt` unit tests (5 tests, inline in `services/admin.rs`)
Directly tests the helper extracted during the C3 audit fix:
- Re-encrypts when `new_val` is `Some`; result decrypts correctly.
- New ciphertext differs from old (AES-GCM random nonce ensures this).
- Passes existing `(ct, nonce)` through unchanged when `new_val` is `None`.
- Returns `(None, None)` when both `new_val` and existing are `None`.
- Empty string clears the field (`encrypt_opt` short-circuits on `""`).

#### T2 — Partial profile update preserves all untouched PII fields (1 test, `tests/admin.rs`)
Creates a client with a full PII profile (address, pin, contact, phone), then
updates only `name`. Asserts every other field decrypts to its original value.
Guards against regressions in `keep_or_reencrypt` usage in `update_client_profile`.

#### T3 — Email update rotates blind index (1 test, `tests/admin.rs`)
Updates a client's email, then verifies the old Argon2id hash no longer finds
the user via `find_by_email_hash` and the new hash does. Ensures login lookup
stays consistent with stored email after an email change.

#### T4 — Ticket status state machine (12 tests, inline in `ticket_status.rs`)
Exhaustive transition table covering every valid and invalid path:
- Valid: `open→acknowledged`, `open→pending`, `pending→acknowledged`, `acknowledged→closed`.
- Invalid: all other combinations (`open→close`, `pending→close`, `closed→*`,
  `acknowledged→pend`, `pending→pend`).
- Unknown status string returns an error.

#### T5 — Invoice CRUD (7 tests, `tests/invoices.rs`)
- Create defaults to `"draft"` status.
- All fields round-trip through `find_by_id`, including `billed_to_email` and `billed_to_phone`.
- `list` returns all invoices; `list_for_client` filters by client.
- `update` changes `status` and `number`.
- `delete` removes the row; subsequent `find_by_id` returns `None`.
- `next_seq` starts at 1 and increments with each invoice.

### Frontend — Vitest setup + new tests (F1–F2)

Added Vitest to `devDependencies`; wired `npm test` script and `test.environment`
config in `vite.config.ts`.

#### F1 — `extractApiError` (5 tests, `src/utils/format.test.ts`)
- Returns `response.data.error` when present.
- Falls back to custom fallback when `data.error` is missing.
- Falls back when there is no `response` at all (e.g. `Error('Network Error')`).
- Uses the default `'Something went wrong'` fallback when none supplied.
- Handles `null` and `undefined` without throwing.

#### F2 — `safeDecode` (5 tests, `src/auth/AuthContext.test.ts`)
- Returns `null` for `null`, empty string, and malformed tokens.
- Decodes a desk full-session token and exposes `sub`, `role`, `session_type`.
- Decodes a scoped client token and exposes `ticket_scope` and `jti`.

---

## [Unreleased] — code-audit cleanup (B1, L1, L2, I1–I3, C1–C5, S1–S4)

### Bug fixes

#### B1 — `useState` lazy-init never re-ran on `clientDropOpen` change (`InvoicesPage.tsx`)
The outside-click handler for the client dropdown was registered inside a
`useState` initializer, which runs only once at mount — so the listener was
never re-added when the dropdown re-opened. Replaced with a `useEffect` keyed
on `[clientDropOpen]` so the `mousedown` listener is attached and removed
correctly on each open/close cycle.

### Safety / correctness

#### L1 — DB error in `hard_delete_client` was silently swallowed (`services/admin.rs`)
`db::tickets::list_all_for_client(...).unwrap_or_default()` meant that if the
query failed, the delete would proceed against an empty ticket list while
orphaning every upload directory on disk. Changed to `?` so the error
propagates and the delete is aborted.

#### L2 — Decryption failure produced empty email address for desk-recovery link (`services/auth.rs`)
`crypto::decrypt(...).unwrap_or_default()` silently fell back to `""` as the
email address, causing `send_desk_recovery_link` to attempt delivery to an empty
string. Replaced with a `match` that logs an error and skips the spawn on
decryption failure.

### Consistency / style

#### I1 — Raw `fetchWithAuth` helper replaced with axios wrapper (`InvoicesPage.tsx`)
`handlePreview` and `handleDownload` used a hand-rolled `fetch` with a manual
`Authorization` header. Replaced with `api.get<string>(..., { responseType: 'text' })`
and `api.get<Blob>(..., { responseType: 'blob' })` so all HTTP calls go through
the shared interceptors.

#### I2 — `extractApiError` utility extracted; five cast-patterns replaced
Added `extractApiError(err, fallback)` to `utils/format.ts`. Replaced identical
inline `(err as ...).response?.data?.error` casts in `CreateTicketModal`,
`ClientsPage`, `SettingsPage`, `ReportsPage`, and `TicketDetailPage`.

#### I3 — `window.confirm()` replaced with `ConfirmModal` (`InvoicesPage.tsx`)
The delete-invoice confirmation used the blocking `window.confirm()`. Replaced
with the existing `ConfirmModal` component driven by a `confirmDeleteId` state
variable, consistent with the rest of the app.

### Complexity reduction

#### C1 — `login()` match arm extracted into two helpers (`services/auth.rs`)
The `(Some(u), false)` arm (~97 lines) was split into:
- `maybe_auto_lockout_ticket(pool, user_id, attempts)` — async, creates the
  security-log ticket when needed.
- `spawn_desk_recovery_link(pool, config, mailer, enc_key, …)` — sync, spawns
  the magic-link email and subsumes the L2 fix.

#### C2 — Duplicate `tokio::spawn` email blocks collapsed into helper (`services/tickets.rs`)
`create()` and `apply_transition()` both contained identical notification-spawn
blocks. Extracted into `spawn_ticket_notification_email(mailer, user, title, event)`.

#### C3 — Five repeated encrypt-or-keep blocks collapsed into helper (`services/admin.rs`)
`update_client_profile` had five identical `if new_val.is_some() { encrypt } else { keep }`
blocks. Extracted into `keep_or_reencrypt(key, new_val, existing_ct, existing_nonce)`
and replaced with five one-liner calls.

#### C4 — `EditPropsPanel` and `ReadOnlyProps` extracted from `TicketDetailPage.tsx`
~185 lines of inline component definitions (plus their constants) moved to
`frontend/src/components/EditPropsPanel.tsx` and `ReadOnlyProps.tsx`.
`TicketDetailPage.tsx` now imports them.

#### C5 — `LineItemsEditor` and `NotesEditor` extracted from `InvoicesPage.tsx`
The line-items grid and notes editor were duplicated verbatim inside both
`CreateInvoiceModal` and `EditInvoiceModal`. Extracted into two sub-components
(`LineItemsEditor`, `NotesEditor`) defined once above the modals; both modals
now render them via props.

### Style nits

#### S1 — Duplicate `use std::` merged (`backend/src/auth.rs`)
`use std::{net::SocketAddr, str::FromStr}` and `use std::sync::Arc` were on
separate lines. Merged into one.

#### S2 — `HeaderName::from_str(name).unwrap()` made consistent (`backend/src/auth.rs`)
`HeaderValue::from_str` already used `.map_err(|e| AppError::Internal(...))?`.
`HeaderName::from_str` used `.unwrap()`. Applied the same `.map_err(...)? ` pattern
to both call sites (`logout` and `build_token_response`).

#### S3 — `DeleteOnDrop` struct moved after imports (`backend/src/routes/admin.rs`)
The struct definition appeared between the external `use` block and the `use crate::…`
block, breaking the conventional import-then-definitions ordering. Moved to after
all `use` statements.

#### S4 — Inline `import('./types').Client` replaced with top-level import (`frontend/src/api/admin.ts`)
`updateClient` return type used `import('./types').Client` inline. `Client` is
already imported at the top of the file; the inline import was redundant.

---

## [Unreleased] — lockout hardening, theme toggle, test suite fixes

### Backend — lockout auto-actions

#### Auto-ticket on client permanent lockout (`services/auth.rs`, `db/tickets.rs`)
When a client account hits the permanent lockout tier (9 consecutive wrong
passwords), the server auto-opens an `urgent` `security_log` ticket so the desk
sees it in the queue without checking the admin panel. The ticket is created in
the `(Some(u), false)` arm of `login`, after `increment_failed_attempts` writes
the permanent sentinel. Guard: `db::tickets::has_recent_security_lockout_ticket`
checks for an existing `security_log` ticket created within the last 24 hours —
a brute-force attacker hammering a locked account cannot flood the queue.

#### Desk lockout recovery via magic link (`services/magic.rs`, `services/auth.rs`)
When the desk account hits permanent lockout, the server auto-generates a recovery
magic link and sends it to the desk's registered email (requires SMTP configured).
Rate-limited to one email per 5 minutes via `db::magic_links::has_recent_active_for_user`.
If SMTP is not configured, a `tracing::error!` prints the manual SQL recovery
command to the log. New function: `services::magic::send_desk_recovery_link`.

#### Magic link exchange resets lockout (`services/magic.rs::exchange_magic_link`)
Exchanging a valid magic link now calls `db::users::reset_lockout` after marking
the link as used. A client (or desk) whose account was locked can authenticate
via magic link and have their `failed_attempts` and `locked_until` cleared in the
same operation. Previously, lockout state persisted even after a successful
magic link exchange.

#### `services::auth::login` signature change
Added `mailer: Option<&SmtpMailer>` as the third parameter to support the desk
recovery flow. Updated the single call site in `src/auth.rs` to extract
`State(mailer): State<Option<Arc<SmtpMailer>>>` and pass `mailer.as_deref()`.

### Frontend — login page theme toggle + token cleanup

#### Theme toggle on login page (`pages/LoginPage.tsx`, `theme/ThemeContext.tsx`)
The `LGT / DRK` toggle (already present on every authenticated page) is now also
shown in the login page's status band. Default theme changed to **light**
regardless of OS preference — only an explicit saved choice overrides this.
Removed the `matchMedia('prefers-color-scheme: dark')` listener from
`ThemeContext` since default light makes system-following unnecessary.

#### Token-based row hover in `list.css`
`.lg-row:hover` background changed from raw `rgba(13, 13, 13, 0.04)` to
`color-mix(in oklab, var(--ink) 4%, transparent)`. In dark mode `--ink` is the
light cream color, so the hover tint is automatically correct in both themes
without a separate `[data-theme="dark"]` override (which is now removed).

### Tests — 4 new + 24 call-site fixes

**`tests/lockout.rs`** (new file — 4 tests)
- `permanent_lockout_auto_creates_urgent_security_log_ticket` — verifies the
  auto-ticket is created with `urgent` priority on the 9th bad attempt
- `lockout_ticket_not_duplicated_within_24h` — after a manual unlock and a second
  lockout within 24 h, only one ticket exists (deduplication guard works)
- `magic_link_exchange_resets_lockout` — verifies `failed_attempts = 0` and
  `locked_until = NULL` after a magic link is exchanged on a locked account
- `desk_permanent_lockout_does_not_create_ticket` — desk lockout must not create
  a `security_log` ticket (client-only behaviour)

**`tests/auth.rs`** — 24 call sites updated to pass `None` as the new `mailer`
parameter; all 22 existing auth tests continue to pass.

Total: **110 tests, all passing.**

---

## [Unreleased] — auth_events: per-client login activity

### Backend — `GET /admin/clients/:id/auth-events` (desk only)

Adds a queryable record of login and logout events per client. No IP addresses
stored — the 30-day rotating log files already serve security monitoring; this
table answers "when did this client last log in?" and "is the account being used?"

**New migration** `013_auth_events.sql`: table `auth_events(id, user_id FK,
event_type, created_at)` with indexes on `user_id` and `created_at`.

**Events written** (non-fatal — failures are logged, never block the operation):
- `login_ok` in `services/auth.rs::login` on successful password login
- `magic_ok` in `services/magic.rs::exchange_magic_link` on successful magic link exchange
- `logout` in `services/auth.rs::logout` when a session is explicitly ended

**Route** `GET /admin/clients/:id/auth-events` — desk only, returns the 50 most
recent events for that client, newest first. Response: `[{ id, event_type, created_at }]`.

**Cascade**: `auth_events` rows are deleted with the user in
`cascade_delete_user_data` (added alongside the existing `jwt_revocations` delete).

**Tests** (2 new in `tests/auth.rs`):
- `successful_login_writes_login_ok_event`
- `logout_writes_logout_event`

---

## [Unreleased] — extended test suite + cascade bug fix

### Bug fix — `cascade_delete_user_data` missing `jwt_revocations` delete
`jwt_revocations` has a `REFERENCES users(id)` FK. Hard-deleting a client who
had exchanged a magic link since M8 shipped would fail with an FK constraint
violation. Added `DELETE FROM jwt_revocations WHERE user_id = ?` to the cascade
in `services/admin.rs`. Caught by the new orphan-check test.

### Tests (7 new across 4 files)

**`tests/auth.rs`**
- `scoped_session_cannot_change_password` — `require_full_session()` returns
  Forbidden for scoped magic-link Claims; verifies the `FullSessionUser` gate
- `change_password_revokes_outstanding_magic_jtis` — after a password change,
  any active JTI for that user is revoked (M8 + self-service interaction)

**`tests/tickets.rs`**
- `client_post_message_is_stored_and_appears_in_thread` — `post_message` success
  path; the most-used write operation had no success-case test

**`tests/admin.rs`**
- `hard_delete_leaves_no_orphaned_rows_in_any_child_table` — builds a full graph
  (ticket + message + note + JTI), hard-deletes, asserts all child tables clean;
  this test would have caught the `jwt_revocations` cascade bug above

**`tests/notes.rs`** (new file)
- `desk_can_create_note_and_list_it_back` — `create_note` + `list_notes` round-trip
- `note_body_is_encrypted_at_rest` — raw DB column must be hex ciphertext, not plaintext

---

## [Unreleased] — client password self-service

### Backend (`backend/src/middleware.rs`, `backend/src/auth.rs`, `backend/src/services/auth.rs`)
New `FullSessionUser` extractor accepts desk or client full-sessions and rejects
scoped magic-link tokens. `PATCH /auth/password` now uses `FullSessionUser` instead
of `DeskUser`. On success, `change_password` also calls
`db::jwt_revocations::revoke_for_user` so any outstanding magic-link JTIs are
revoked alongside the refresh-token wipe.

### Frontend (`frontend/src/pages/SettingsPage.tsx`)
Removed the `isDesk` guard on the password section. The `PasswordSection` form
was already fully built; the client placeholder is replaced with the same form
desk users see. No API changes needed — it wires to `PATCH /auth/password`
which now accepts client tokens.

---

## [Unreleased] — M8: magic-link JWT revocation

### Backend — closes the last open OWASP finding with a real breach angle

Magic-link JWTs were non-revocable for up to 24h after exchange. A leaked link
URL could not be invalidated before expiry. This implements the design from
PLANNED.md §M8.

#### Schema — `backend/migrations/012_jwt_revocations.sql`
New table `jwt_revocations(jti PRIMARY KEY, user_id REFERENCES users(id),
revoked_at TEXT, expires_at TEXT NOT NULL)` with indexes on `expires_at` and
`user_id`.

#### Claims (`backend/src/models.rs`)
Added `#[serde(default)] pub jti: Option<String>`. `Option` + serde default
means existing tokens and password-login tokens (which carry no jti) decode
unchanged — no live-session breakage.

#### Issuance (`backend/src/services/magic.rs::exchange_magic_link`)
Generates a UUID jti, inserts an active row into `jwt_revocations` (with
`expires_at` = JWT `exp`), and sets `jti: Some(...)` on the Claims before
encoding. The DB insert happens before the token is returned — no window where
a token without a DB record can be presented.

#### Check (`backend/src/middleware.rs::AuthUser`)
After JWT decode, if `claims.jti` is `Some(jti)`, the extractor looks up the
row. Fail-closed contract: missing row → 401 (forged/out-of-sync), revoked_at
non-NULL → 401, DB error → 401. Only a present, active, non-expired row
allows the request. Password-login tokens (`jti: None`) skip this block
entirely — zero DB overhead on the hot path.

#### Revocation (`backend/src/services/admin.rs::delete_client_sessions`)
Extended to call `db::jwt_revocations::revoke_for_user` alongside the existing
refresh-token wipe. The existing frontend "Revoke sessions" button now kills
both refresh tokens and outstanding magic-link JWTs with no UI change.

#### Cleanup (`backend/src/main.rs`)
`db::jwt_revocations::delete_expired` is called at startup and in the existing
daily cleanup loop. Cleanup deletes only rows past `expires_at` — the JWT
`exp` check in `AuthUser` already rejects expired tokens before the jti check
fires, so cleanup cannot prematurely deny a live token.

#### Tests — `backend/tests/magic_revocation.rs` (9 new tests)
Active jti allowed; revoked jti denied; missing jti denied (fail-closed);
access token with no jti allowed without lookup; `revoke_for_user` marks all
active rows; `revoke_for_user` does not affect other users; cleanup removes
expired rows only; full exchange flow creates jti row and preserves
ticket_scope; `delete_client_sessions` revokes magic JTIs.

Total: 94 tests, all passing.

---

## [Unreleased] — default-deny authorization + negative authorization tests

### Backend — invert ownership checks to default-deny (`services/tickets.rs`, `routes/messages.rs`, `services/messages.rs`)

Four sites previously used `if role == "client" { check ownership }` guards,
which fail open for any role that is neither "desk" nor "client". An unexpected
or fabricated role string would bypass ownership checks entirely. All four
sites inverted to `if role == "desk" { full access } else { enforce ownership }`:

- `services::tickets::get_with_thread` — desk/non-desk branch replaces
  separate `deleted_at` and `role == "client"` checks
- `routes::messages::post_message` — `role == "client"` → `role != "desk"`
- `routes::messages::get_attachment` — two `role == "client"` checks collapsed
  into a single `role != "desk"` block
- `services::messages::post_message` — `sender_role == "client"` → `sender_role != "desk"`

No behavior change for desk or for a client accessing their own data.

### Tests — `tests/authorization.rs` (7 new tests)

Negative-authorization test suite confirming IDOR and privilege escalation are
rejected at the service layer:

- `client_cannot_read_another_clients_ticket` — 404 on cross-client ticket read
- `unknown_role_cannot_read_ticket` — unknown role is ownership-checked, not granted desk access
- `desk_can_read_any_ticket` — regression guard confirming desk access unchanged
- `client_filing_ticket_with_another_clients_id_is_filed_as_self` — override silently ignored
- `client_cannot_post_message_to_another_clients_ticket` — 403 on cross-client message
- `unknown_role_cannot_post_message_to_another_clients_ticket` — unknown role denied
- `desk_can_post_message_to_any_ticket` — regression guard for desk message access

---

## [Unreleased] — comprehensive test suite

### Tests — 71 tests across 7 integration test files

Added `backend/src/lib.rs` to expose all modules for integration tests; updated
`main.rs` to import from the lib rather than redeclare modules. Made
`IpRateLimiter::allow` `pub` for rate-limit tests.

Also found and fixed a production bug discovered by tests: `hard_delete_client`
and `cascade_hard_delete_user` did not delete `client_exports` rows before
deleting the user, causing a FK constraint failure when FK enforcement is enabled
and an export existed. Fixed in both `services/admin.rs` and `tasks.rs`.

| File | Tests | Coverage |
|---|---|---|
| `tests/auth.rs` | 16 | Login success/failure, lockout escalation, permanent lock, refresh rotation, replay detection, expired/revoked token, password change |
| `tests/magic.rs` | 7 | Link generation, exchange, single-use enforcement, expiry, revocation on new link, full/scoped session types |
| `tests/tickets.rs` | 13 | Create validation (title, priority, type, date), all valid status transitions, invalid transitions, soft-delete visibility, hard-delete guard, upload dir cleanup |
| `tests/validation.rs` | 15 | Password strength, email format, date format, recurring interval bounds, category length |
| `tests/crypto.rs` | 5 | Encrypt/decrypt roundtrip, wrong key, wrong nonce, nonce uniqueness, odd-length hex rejection |
| `tests/rate_limit.rs` | 4 | Burst allow/reject, token bucket refill over time, independent IP buckets |
| `tests/admin.rs` | 11 | Client create/duplicate/invalid, soft delete, restore, hard delete guard, hard delete success, profile update, export content |

Shared setup in `tests/common/mod.rs`: `setup_test_db`, `test_config`,
`test_enc_key`, `create_test_client`, `create_test_desk`.

---

## [Unreleased] — full backend audit fixes (H-1, M-1 through M-11, L-1 through L-7)

### Backend — 20 findings from the full code audit

#### H-1. `DELETE FROM notifications` removed from three sites (`routes/tickets.rs`, `services/admin.rs`, `tasks.rs`)
Migration 011 dropped the `notifications` table, but `DELETE FROM notifications`
SQL remained in three places. Every call to `DELETE /tickets/:id`, the hard-delete
cascade in `services::admin`, and the background `cascade_hard_delete_user` in
`tasks.rs` would return a 500 "no such table" error. All three statements removed.

#### M-1. Ad-hoc SQL moved to `db::users::find_desk_user` (`db/users.rs`, `services/messages.rs`)
`services/messages.rs` contained a raw `SELECT id FROM users WHERE role = 'desk' LIMIT 1`
query, violating the layer discipline (SQL belongs in `db/`). Extracted to
`db::users::find_desk_user() -> AppResult<Option<User>>` and updated the call site.

#### M-2. Body length uses `chars().count()` not `.len()` (`services/messages.rs`, `services/notes.rs`)
Message and note body limits checked byte count (`.len()`), inconsistent with
password validation which uses `chars().count()`. Both changed to `chars().count()`
so a 10,000-character limit means 10,000 visible characters regardless of encoding.

#### M-3. Decryption failure during export now logs a warning (`services/export.rs`)
Silent `unwrap_or_else(|_| "[decryption failed]")` replaced with a
`tracing::warn!` call including `entry_id` and `ticket_id` before falling back
to the placeholder. Operators now see which entries failed during an export.

#### M-4. `PERMANENT_LOCKOUT_THRESHOLD: i64 = 9` replaces duplicate magic number (`services/auth.rs`, `main.rs`)
The permanent-lockout threshold `9` appeared twice. Now a single `pub const
PERMANENT_LOCKOUT_THRESHOLD` in `services/auth.rs`, referenced from both sites.

#### M-5. `ALLOWED_EXT` moved to module level (`routes/messages.rs`)
The file extension allowlist was declared as a `const` inside a `match` arm
inside a `while` loop. Moved to module level for discoverability.

#### M-6. Upload filename prefixed with UUID to prevent overwrite (`routes/messages.rs`)
Two messages to the same ticket with the same filename would silently overwrite
the first attachment. The saved filename is now `{8-char-uuid}-{original_name}`.
The stored URL in the DB reflects the unique name.

#### M-7. `REVOKED_SESSION_RETENTION_DAYS: i64 = 7` constant with documented rationale (`db/sessions.rs`)
The 7-day window for retaining revoked sessions was a bare magic number with no
comment. Named constant added, with a note that it must be ≥ `REFRESH_TOKEN_TTL_SECS / 86400`.

#### M-8. `MAX_SESSIONS_PER_USER: i64 = 10` replaces duplicate literal (`services/auth.rs`)
The per-user session cap `10` appeared in two `create_capped` call sites. Named constant.

#### M-9. `include_deleted` parameter removed from `list_all_paginated` (`db/tickets.rs`, `services/tickets.rs`)
The parameter was always passed as `false`; the `true` code path was dead. Removed
from the function signature and the single call site. The WHERE clause is now
unconditional. The desk can still access soft-deleted tickets individually by ID.

#### M-10. Rate limiter parameters configurable via env (`config.rs`, `main.rs`, `.env.example`)
Auth and report rate limiter values (RPS, burst) were hardcoded in `main.rs`.
Added `RATE_LIMIT_AUTH_RPS`, `RATE_LIMIT_AUTH_BURST`, `RATE_LIMIT_REPORT_RPS`,
`RATE_LIMIT_REPORT_BURST` to `Config` with the previous values as defaults.
Documented in `.env.example`.

#### M-11. `Argon2::default()` in `verify_password` replaced with explicit constructor (`services/auth.rs`)
Consistent with every other argon2 call in the file. Uses
`Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())`.
Params are still read from the PHC string; the explicit constructor just pins
the algorithm and version the same way hashing does.

#### L-1. `TicketStatus::from_str` renamed to `parse` (`ticket_status.rs`)
The private method name shadowed the standard `std::str::FromStr` trait convention
with a different signature. Renamed to `parse` to avoid the ambiguity.

#### L-4. `update_status` checks `rows_affected()` (`db/tickets.rs`)
Previously returned `Ok(())` silently if the ticket ID didn't exist. Now returns
`AppError::Internal("ticket vanished during status update")` if no row was updated.

#### L-6. `MAX_FILE_BYTES` comment cross-references `DefaultBodyLimit` (`routes/messages.rs`)
Added a comment noting that the per-file limit must be ≤ the global body limit in
`main.rs`, so a future change to one flags the need to update the other.

#### L-7. Unused `State(_pool)` removed from `get_export_file` (`routes/admin.rs`)
The handler extracted `SqlitePool` from state but never used it. Removed.

---

## [Unreleased] — resolve clippy warnings

#### Four clippy lints fixed (no logic changes)

- `crypto.rs:102` — `s.len() % 2 != 0` → `!s.len().is_multiple_of(2)`
- `dto.rs:94` — `.last()` → `.next_back()` on the split iterator in `ExportResponse::from`
- `routes/tickets.rs:40` — `pagination.limit.max(1).min(100)` → `pagination.limit.clamp(1, 100)`
- `services/tickets.rs:42` — `limit.max(1).min(100)` → `limit.clamp(1, 100)`

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
