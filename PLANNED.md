# Planned Features

---

## Tauri desktop wrapper (v1.x)

Wrap Lodgr as a native desktop app so the desk agent can launch it with a
single click instead of running two terminal commands.

### Why

Running `cargo run` in one terminal and `npm run dev` in another is fine for
development but awkward for daily use as a working tool. A Tauri app bundles
the Rust backend + React frontend into a single executable that opens a native
window and manages its own lifecycle.

### Planned approach — sidecar

Tauri's **sidecar** feature bundles an external binary alongside the app and
spawns it as a child process on launch. For Lodgr this means:

1. Build the Axum backend as a standalone binary (already done).
2. Register it as a Tauri sidecar — Tauri spawns it on startup and kills it on
   close.
3. The webview points at `http://localhost:3000` (same as production).
4. No changes to the backend code at all — it stays a plain HTTP server.

### What needs to happen

- Add a `tauri/` directory at the repo root with a minimal Tauri v2 project.
- Configure `tauri.conf.json`: sidecar path, window title/size, `devUrl` for
  dev and `frontendDist` for production.
- Build script: `npm run build` (frontend) → `cargo build --release` (backend)
  → `cargo tauri build` (bundles both into a platform installer).
- Handle the `.env` / secrets: on first launch show a setup screen that writes
  the config to `$APPDATA/lodgr/config.env` (Windows) or
  `~/.config/lodgr/config.env` (Linux/Mac). Subsequent launches read from
  there.
- SQLite path should default to `$APPDATA/lodgr/support.db` so data survives
  app reinstalls.

### What it does NOT change

- The server-side deployment story — the Axum binary stays deployable on its
  own (Hetzner, DO, etc.) with no Tauri dependency.
- The existing dev workflow — `cargo run` + `npm run dev` still works.

### Effort estimate

Medium. Tauri v2 + Rust sidecar is well-documented. The main complexity is the
first-launch setup screen and making the config path platform-aware.

---

## ~~Client password self-service~~ ✅ Done

Shipped in commit `38c3311`. Clients can now change their own password via
`PATCH /auth/password`. A new `FullSessionUser` extractor accepts desk or client
full-sessions and rejects scoped magic-link tokens. On success, all existing sessions
and outstanding magic-link JTIs are revoked. The `SettingsPage` password form was
already built; the placeholder was removed and the form now renders for all roles.

---

## ~~Client password self-service (v1.x) — original plan~~

Clients currently cannot change their own password. `PATCH /auth/password` is
desk-only (enforced by the `DeskUser` extractor). The Settings page already shows
a placeholder explaining this, and the password-change form is fully built for the
desk — it just needs the backend route extended to accept client full-sessions.

### Planned behaviour

Extend `PATCH /auth/password` to accept client tokens with `session_type: "full"`.
Scoped magic-link sessions must NOT be able to change a password — too limited.

### What's needed

**Backend**
- Change the route extractor from `DeskUser` to a new `FullSessionUser` that accepts
  either `role: "desk"` or `role: "client"` but rejects scoped tokens.
- Existing password strength validation and argon2id hashing already apply.
- Response (`{ access_token }` + new refresh cookie) is already correct.

**Frontend**
- `SettingsPage` — replace the client placeholder with the same `PasswordSection`
  form already shown to desk users (it wires to `authApi.changePassword` which hits
  `PATCH /auth/password` — will just work once the backend allows it).

### Security notes

- Scoped sessions must not change password — the scope is ticket-only by design.
- Requiring the current password prevents an attacker with a stolen magic link from
  permanently taking over the account.
- On success, all other sessions are already invalidated (existing desk behaviour).

---

## ~~Auto-ticket on client lockout~~ ✅ Done + ~~login event tracking~~ ✅ Done

On permanent lockout (9th consecutive wrong password) the server auto-opens an
`urgent` `security_log` ticket so the desk sees it without checking the admin panel.
Guarded by a 24-hour deduplication check (`db::tickets::has_recent_security_lockout_ticket`)
to prevent queue flooding. Implemented in `services/auth.rs`. 4 tests in `tests/lockout.rs`.

Magic link exchange now also resets `failed_attempts` and `locked_until` so the
client's lockout is cleared when they authenticate via the link.

`auth_events(id, user_id, event_type, created_at)` — events: `login_ok`, `magic_ok`,
`logout`. Route: `GET /admin/clients/:id/auth-events` — desk only.

---

## ~~Account lockout recovery via magic link — desk~~ ✅ Done

When the desk account hits permanent lockout the server auto-generates a recovery
magic link and emails it to the desk address (requires SMTP). Rate-limited to one
email per 5 minutes. If SMTP is not configured, the manual SQL recovery command is
printed to `tracing::error!`. DB recovery remains the break-glass option.

---

## Multi-desk support (v1.x)

The current system is designed for a single desk agent (`desk@local`). Multi-desk
support is planned for a future version.

### What's needed

**Schema changes**
- A `desk_accounts` table (or extending `users`) with individual desk credentials.
- A `super_admin` role or separate admin bootstrap mechanism to create desk accounts.
- Desk account creation restricted to `super_admin` only.

**Authorization changes**
- Client ownership scoping per desk: each client is assigned to a specific desk agent,
  desk agents can only manage their own clients.
- The `DeskUser` extractor carries the desk agent's ID so service functions can
  enforce ownership.
- All admin routes must filter by the requesting desk agent's assigned clients.

**Security implications**
- Session isolation: a desk agent's sessions must not be revocable by another agent.
- Magic link audit trail must record which desk agent generated each link.
- Super-admin must NOT be able to read message thread content — separation of
  administrative and operational privilege.
- No code path that allows `client` or `desk` role to create a `super_admin` account.

### Migration path

Existing `desk@local` will be preserved as a legacy single-desk account. Before
enabling multi-desk in production:
1. Create explicit desk accounts with real credentials.
2. Assign each existing client to a desk account.
3. Disable or rename `desk@local`.

---

## ~~Invoices~~ ✅ Done

Full invoice management shipped:

- **Create / edit / delete / status** (`draft → sent → paid`) via the Invoices page
- **Print to PDF** — server-rendered HTML (A4, print CSS) at `GET /admin/invoices/:id/print`
- **Recurring invoices** — monthly / quarterly / yearly templates; background task auto-creates drafts and advances `next_recur_date`
- **Auto-incrementing numbers** — backend assigns `INV-0001`, `INV-0002`, … automatically; no manual entry needed
- **Desk profile** (`GET/PUT /admin/desk-profile`) — the "From" section on every invoice (name, tagline, email, website, city, phone, VAT) is stored in the database and editable under Settings → Desk profile. Zero personal info hard-coded in the source.
- **Invoice edit modal** — all fields editable post-creation, including line items and recurring settings
- **Sub-client tag on tickets** — tickets can be tagged to an end client (sub-client) when creating; selector shows custom dropdown with "add from Clients page" hint when none exist

---

## Smaller backend items

| Item | Why | Effort | Status |
|------|-----|--------|--------|
| Ticket server-side filtering | `?status=open&priority=high&q=login`. Currently fetch-all and filter client-side. | Medium | Open |
| Unread message count | `unread_count` on `TicketResponse`. Without it every ticket has to be polled. | Small | Open |
| SSE for real-time updates | Currently polls every 30 s. SSE would push updates instantly. | Medium | Open |
| Paginate `GET /admin/clients` | Consistent with tickets. Needed at any real scale. | Small | Open |
| ~~CI pipeline + `cargo audit`~~ | ~~No automated test/audit run on push.~~ | ~~Small~~ | ✅ Done |
| ~~`GET /health`~~ | ~~Every load balancer needs this.~~ | ~~Trivial~~ | ✅ Done |
| ~~File download endpoint~~ | ~~Attachments not serveable.~~ | ~~Small~~ | ✅ Done |
| ~~`spawn_blocking` for PDF/export~~ | ~~Both blocked the async executor.~~ | ~~Small~~ | ✅ Done |
| ~~Background task restart logic~~ | ~~Both tasks died silently on panic.~~ | ~~Small~~ | ✅ Done |

---

## `client_exports` FK schema fix (v1.x)

`client_exports.client_id` has a `REFERENCES users(id)` FK but no `ON DELETE CASCADE`.
The current code deletes `client_exports` rows explicitly in `cascade_delete_user_data`
(services/admin.rs) before deleting the user row. This works, but it means every future
addition of a child table requires updating the shared cascade function — the same
structural gap that caused the FK bug that the test suite caught on day one.

### The right fix

Migrate `client_exports` to use `ON DELETE CASCADE` (or `ON DELETE SET NULL` if the
audit-trail intent is to keep the row with a null client_id).

### Why it isn't done yet

SQLite does not support `ALTER TABLE … ADD CONSTRAINT` or `ALTER TABLE … DROP CONSTRAINT`.
Changing a FK constraint requires recreating the table:

```sql
-- 1. Create replacement table with CASCADE
CREATE TABLE client_exports_new (
    id         TEXT PRIMARY KEY,
    client_id  TEXT REFERENCES users(id) ON DELETE CASCADE,
    file_path  TEXT NOT NULL,
    created_at TEXT NOT NULL
);
-- 2. Copy existing rows
INSERT INTO client_exports_new SELECT * FROM client_exports;
-- 3. Drop old table, rename
DROP TABLE client_exports;
ALTER TABLE client_exports_new RENAME TO client_exports;
```

This is low-risk for a small table with no external dependencies, but it is a
destructive operation that needs explicit human review before running in production.

---

## Magic link JWT revocation (v1.x) — M8

### The problem

When a client exchanges a magic link token (`POST /auth/magic`), Lodgr marks the
link as used (`magic_links.used_at`) and issues a JWT. That JWT is **stateless and
non-revocable**. The `used_at` flag prevents the magic link URL from being exchanged
a second time, but it has no effect on the JWT that was already issued.

The JWT is valid for the full `SCOPED_TOKEN_TTL_SECS` window — 24 hours by default.
If a magic link URL is intercepted (email provider logs, clipboard, network capture)
or a device is stolen after exchange, the attacker has a 24-hour window with no way
to cut it short. There is no session record for magic link JWTs; revocation is
structurally impossible with the current design.

This is the main remaining security gap with a real breach angle. All other open
findings (M4, M7, L3) are hardening; this one has a direct access-to-data path if
exploited.

### Design

Add a `jti` (JWT ID) claim to magic-link-issued JWTs. Store issued JTIs in a new
`jwt_revocations` table. On every authenticated request where `claims.jti` is
present, check the table. Revoke by writing a `revoked_at` timestamp against the
`jti` — done in microseconds with a single indexed point lookup.

Regular access tokens (issued by password login and refresh) keep their current
15-minute TTL and remain stateless — the short window makes DB revocation
unnecessary and would add a lookup to every single API call. Only magic-link JWTs
get a `jti` and a revocation check.

### Schema

New migration `012_jwt_revocations.sql`:

```sql
CREATE TABLE IF NOT EXISTS jwt_revocations (
    jti        TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    revoked_at TEXT,             -- NULL = active, non-NULL = revoked
    expires_at TEXT NOT NULL     -- copied from the JWT exp claim; used for cleanup
);
CREATE INDEX IF NOT EXISTS idx_jwt_revocations_expires_at
    ON jwt_revocations (expires_at);
CREATE INDEX IF NOT EXISTS idx_jwt_revocations_user_id
    ON jwt_revocations (user_id);
```

### What's needed

**`backend/migrations/012_jwt_revocations.sql`** (new)
Schema above.

**`backend/src/models.rs`**
Add `jti: Option<String>` to `Claims` with `#[serde(default)]`. The `Option`
preserves backward compatibility — existing tokens without a `jti` field decode
as `None` and bypass the revocation check (they expire naturally).

**`backend/src/db/jwt_revocations.rs`** (new module)
Four functions:
- `create(pool, jti, user_id, expires_at)` — insert a new active JTI record.
  Called once per magic link exchange.
- `is_revoked(pool, jti)` → `AppResult<bool>` — single point-read by primary key.
  Returns `true` if the row exists AND `revoked_at IS NOT NULL`.
- `revoke_for_user(pool, user_id)` — sets `revoked_at = now()` on all active
  (non-expired, non-revoked) rows for a given `user_id`. Called by session
  termination endpoints.
- `delete_expired(pool)` — `DELETE FROM jwt_revocations WHERE expires_at < now()`.
  Called by the existing session cleanup loop.

Add `pub mod jwt_revocations;` to `db/mod.rs`.

**`backend/src/services/magic.rs`**
In `exchange_magic_link`, after encoding the JWT:
1. Generate a `jti = Uuid::new_v4().to_string()`.
2. Embed it in `Claims { ..., jti: Some(jti.clone()) }` before encoding.
3. Call `db::jwt_revocations::create(pool, &jti, &user.id, &expires_at_str)`.
4. Log `jti` alongside the existing success log line.

**`backend/src/middleware.rs`** — `AuthUser` extractor
After decoding claims, add:
```rust
if let Some(ref jti) = claims.jti {
    if db::jwt_revocations::is_revoked(pool, jti).await? {
        return Err(AppError::Unauthorized);
    }
}
```
`AuthUser` currently has no pool access — add `State(pool): State<SqlitePool>` to
the extractor (it already lives in `AppState`; `SqlitePool: FromRef<AppState>`
is already implemented).

**`backend/src/routes/admin.rs`** — extend `delete_client_sessions`
`DELETE /admin/clients/:id/sessions` currently revokes refresh token sessions only.
Extend it to also call `db::jwt_revocations::revoke_for_user(pool, client_id)`.
This makes the existing "kill all sessions" desk action fully effective — it now
also neutralises any outstanding magic-link JWTs, with no new endpoint needed.

**`backend/src/tasks.rs`** — session cleanup loop
Add `db::jwt_revocations::delete_expired(&pool).await` alongside
`db::sessions::delete_expired_and_revoked` in the daily cleanup loop and at startup.

### Security notes

- **Backward compatibility.** Tokens without `jti` (`None`) pass through unchanged.
  If the TTL is short (< 1 h), this is acceptable. If you reduce `SCOPED_TOKEN_TTL_SECS`
  to 1–4 h as a short-term mitigation, the exposure window is already small by the
  time revocation lands.
- **Performance.** One indexed primary-key lookup per request for magic-link sessions
  only. SQLite handles millions of point reads per second at this access pattern.
- **Revocation granularity.** `revoke_for_user` kills all outstanding JTIs for a
  user in one query — the right behaviour for "client reported device stolen" or
  any admin forced-logout action.
- **Cleanup.** The `expires_at` index ensures `delete_expired` is O(log n) and
  the table stays small — only active and recently-expired rows accumulate.
- **This does not fix M7 (CORS) or M4 (memory load).** Those remain separate backlog
  items.

---

## Security hardening backlog

From the Phase 2 OWASP review — findings not yet fixed:

| # | Severity | Finding |
|---|----------|---------|
| M4 | MEDIUM | Attachment download loads full file into memory; no download rate limit |
| M7 | MEDIUM | No CORS middleware — needed for non-same-origin deploys |
| ~~M8~~ | ~~MEDIUM~~ | ~~Magic-link JWTs non-revocable~~ ✅ Done — `jwt_revocations` table, jti claim, fail-closed AuthUser check, `f3e0cb1` |
| ~~H5~~ | ~~HIGH~~ | ~~No CI pipeline; `cargo audit` not automated~~ ✅ Done — `.github/workflows/ci.yml` |
| ~~L3~~ | ~~LOW~~ | ~~Email addresses in SMTP failure logs~~ ✅ Done — masked to `a***@example.com` |
