# Planned Features

---

## Client password self-service (v1.x)

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

## Auto-ticket on client lockout + login event tracking (v1.x)

### The flow

When a client hits permanent lockout (≥5 failed attempts reaching the permanent
tier), the backend should automatically open a `security_log` ticket so the desk
sees it in their queue without needing to check the admin panel.

```
client fails 5× login
  → account permanently locked
  → backend auto-creates security_log ticket (priority: urgent)
  → desk sees it in queue
  → desk opens ticket → generates magic link → QR code modal
  → client scans → authenticated + lockout counter reset
```

The ticket becomes the communication record for the event. The desk can add
internal notes (e.g. "confirmed client identity, issued new link"), and the
thread is the full audit trail.

### What's needed (backend)

**1. Auto-create a ticket on permanent lockout** (medium)

In `services/auth.rs`, where `compute_locked_until` returns the permanent sentinel
(`"9999-01-01T00:00:00+00:00"`), after writing the lockout to the DB, call
`db::tickets::create(...)` with:
- `title`: `"Account locked — repeated failed login attempts"`
- `description`: `"Client account {client_id} was permanently locked after {n} consecutive failed login attempts. Generate a magic link to restore access."`
- `ticket_type`: `security_log`
- `priority`: `urgent`
- `created_by`: the client's own ID (acceptable — lock was triggered by their account)

Only create the auto-ticket on the **first** permanent lockout — check if a recent
`security_log` ticket already exists for this client before creating, to prevent a
brute-force attacker flooding the queue.

**2. Login event audit table** (medium, lower priority)

Currently login events are in the tracing log files (structured JSON lines) but
not queryable from the app. An `auth_events` table would let the frontend show
a client's recent login history.

```sql
CREATE TABLE auth_events (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL,
    event_type  TEXT NOT NULL,  -- 'login_ok' | 'login_fail' | 'lockout' | 'magic_ok' | 'logout'
    ip_address  TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

Route: `GET /admin/clients/:id/auth-events` — desk only.

### What's needed (frontend)

- `ClientsPage` — the 🔒 badge and conditional "Unlock" prominence already work
  because `locked_until` is now in `ClientResponse` (already shipped in the DTO).
- The QR magic link modal is the recovery action — generate a magic link, show the
  QR, client scans, done. Already built.

### Security notes

- Auto-ticket creation is server-side only — no client-triggered endpoint.
- Sensitive detail (IP addresses, attempt counts) goes in an internal note, not the
  visible thread.
- Rate: only create the ticket on the first permanent lockout per client.

---

## Account lockout recovery via magic link — desk (v1.x)

When the desk account hits permanent lockout, the server should auto-send a magic
link to the desk's registered email. Requires SMTP to be configured.

### What's needed

- Logic in `exchange_magic_link` to clear `failed_attempts` and `locked_until` when
  the token is exchanged (already safe — token is single-use).
- Auto-send the magic link email when permanent lockout is reached for `desk@local`.
- Rate-limit magic link sends per account to prevent email-spam vectors.
- DB recovery remains the break-glass option if SMTP is not configured.

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

## Smaller backend items

| Item | Why | Effort | Status |
|------|-----|--------|--------|
| Ticket server-side filtering | `?status=open&priority=high&q=login`. Currently fetch-all and filter client-side. | Medium | Open |
| Unread message count | `unread_count` on `TicketResponse`. Without it every ticket has to be polled. | Small | Open |
| SSE for real-time updates | Currently polls every 30 s. SSE would push updates instantly. | Medium | Open |
| Paginate `GET /admin/clients` | Consistent with tickets. Needed at any real scale. | Small | Open |
| CI pipeline + `cargo audit` | No automated test/audit run on push. `cargo audit` currently manual. | Small | Open |
| ~~`GET /health`~~ | ~~Every load balancer needs this.~~ | ~~Trivial~~ | ✅ Done |
| ~~File download endpoint~~ | ~~Attachments not serveable.~~ | ~~Small~~ | ✅ Done |
| ~~`spawn_blocking` for PDF/export~~ | ~~Both blocked the async executor.~~ | ~~Small~~ | ✅ Done |
| ~~Background task restart logic~~ | ~~Both tasks died silently on panic.~~ | ~~Small~~ | ✅ Done |

---

## Security hardening backlog

From the Phase 2 OWASP review — findings not yet fixed:

| # | Severity | Finding |
|---|----------|---------|
| M4 | MEDIUM | Attachment download loads full file into memory; no download rate limit |
| M7 | MEDIUM | No CORS middleware — needed for non-same-origin deploys |
| M8 | MEDIUM | Magic-link JWTs non-revocable, 24-hour TTL |
| H5 | HIGH | No CI pipeline; `cargo audit` not automated |
| L3 | LOW | Email addresses in SMTP failure logs |
