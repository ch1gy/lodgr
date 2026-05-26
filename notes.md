# Lodgr — Dev Notes

---

## Next Session — Start Here

Two small remaining security items, then features.

1. **Export files plaintext on disk** — HIGH. Decide: encrypt at rest or download-and-delete. Detail below.
2. **cargo audit** — MEDIUM. One command, add to CI. Detail below.

After those: `GET /health` and file download endpoint are the highest-impact features.

---

## Security Posture

**Rating: 8.5 / 10** *(was 7.5 before account lockout)*

Ready to deploy to a private or internal URL. The two remaining items
below are worth fixing before any public exposure.

**What's strong**
- AES-256-GCM with per-entry nonces on all message bodies and internal notes
- Argon2id at correct parameters for both passwords and key derivation
- SHA-256 refresh token storage — DB breach doesn't yield usable tokens
- Refresh token rotation with theft detection (replay = all sessions wiped)
- Per-account lockout: 5 failures → exponential backoff → permanent (migration 010)
- Scoped magic link sessions, DeskUser requires full session
- Full security header stack (CSP, HSTS, Referrer-Policy, Permissions-Policy, etc.)
- Parameterized SQL throughout — no injection surface
- Structured audit logging on every auth and admin event
- Daily log rotation, 30-day retention
- Input validation: enums, lengths, date format, email format, common passwords
- Hard delete fully cascading in a single transaction

**What's still open**
- Export files are cleartext JSON on disk
- No cargo audit in CI

---

## Planned Features

Features worth building, roughly in order of impact.

### Short term — before frontend

| Feature | Why | Effort |
|---------|-----|--------|
| `GET /health` | Every deployment needs this — load balancers, uptime monitors, sanity checks | Trivial |
| File download endpoint | Attachments are uploaded but unserveable. Broken feature. | Small |
| Client password change | Clients are stuck with whatever the desk set. Extend existing `PATCH /auth/password`. | Small |
| Export download-and-delete | Removes plaintext files from disk after download | Small |
| WAL mode | One line. Concurrent read/write without blocking. | Trivial |

### Medium term — during or after frontend

| Feature | Why | Effort |
|---------|-----|--------|
| Ticket filtering & search | `?status=open&priority=high&q=login` — currently filter client-side | Medium |
| Unread message count | Minimum for a usable UI. Without it you have to poll every ticket. | Small |
| Paginate `GET /admin/clients` | Consistent with tickets. Needed at any real scale. | Small |
| Real-time updates (SSE) | Without it the desk and clients have to refresh manually | Medium |
| OpenAPI spec (`utoipa`) | Auto-generates TypeScript types. Works well with Axum. | Medium |

### Longer term — architecture

| Feature | Why | Effort |
|---------|-----|--------|
| Multi-desk support | See PLANNED.md — requires roles, client ownership scoping, super-admin | Large |
| Magic link account recovery | Unlock via email instead of DB access. See PLANNED.md. | Medium |
| Webhook support | POST events to a client-registered URL. Email is already the pattern. | Medium |
| spawn_blocking for PDF/export | Stop blocking the async executor on CPU work | Small |
| Background task restart logic | Tasks die silently on panic. Needs a respawn loop. | Small |
| Postgres migration | When SQLite's single-writer ceiling becomes a problem | Medium |

---

## Remaining Security Work

### ✅ Account Lockout — DONE

Migration 010. 5 failures → 1 min → 5 min → 15 min → 1 hour → permanent.
`POST /admin/clients/:id/unlock` for client accounts.
Desk recovery: `sqlite3 data/support.db "UPDATE users SET failed_attempts=0, locked_until=NULL WHERE email='desk@local'"`
Magic link recovery: see PLANNED.md.

---

### 1. Export Files Are Plaintext on Disk — HIGH
> Not fixed.

Every export writes decrypted JSON to `exports/<client_id>/<uuid>.json`.
The directory is in `.gitignore` and the download endpoint is desk-auth-gated,
but the files are cleartext on the filesystem. A misconfigured backup or
filesystem breach exposes every historical conversation.

**Options**

- **a) Encrypt at rest** — re-encrypt the JSON with AES-256-GCM before writing, decrypt on download. Clean, but the export is only useful while the encryption key is available.
- **b) Download-and-delete** — delete the file from disk after the desk downloads it. DB record stays; only the file is removed.

Option (b) is recommended: simpler, reduces the attack surface to the window between creation and download.

---

### 3. cargo audit — MEDIUM
> Not set up.

No automated CVE scanning on the dependency tree. A known vulnerability in any of the 20+ crates won't surface until manually checked.

```bash
cargo install cargo-audit
cargo audit
```

Add to CI/build pipeline. Can also add `cargo-deny` for licence and duplicate-dependency checks. Neither requires code changes.

---

---

## Code Quality & Dead Code

### Dead Code

**models.rs**
- `Notification` struct — never constructed. `notify()` uses a raw query; this struct is dead weight.
- `Session` — `.token_hash`, `.created_at`, `.replaced_by`, `.session_type`, `.scoped_ticket_id` are never read after mapping.
- `MagicLink` — `.token_hash`, `.created_at` never read after fetch.
- `ClientExport` — `.created_at` never read.
- `User` — `.created_at` never read.
- `Ticket` — `.last_recurred_at` stored and updated but never read anywhere.

**db/tickets.rs**
- `hard_delete()` — dead. `hard_delete_client` in admin.rs now uses inline transaction SQL.

**db/users.rs**
- `soft_delete()` and `restore()` — dead. Admin service was rewritten to use inline transaction SQL. `hard_delete()` is still alive (used in `tasks.rs`).

**db/tickets.rs**
- `soft_delete()` and `restore()` — same situation as above.

**db/exports.rs**
- `find_latest_for_client()` — never called.

**services/magic.rs**
- `MagicLinkOutput.raw_token` — populated but never accessed by any caller.

---

### Architecture & Best Practices

**Broken layering in services/admin.rs**
`soft_delete_client`, `restore_client`, and `hard_delete_client` now contain raw `sqlx::query` calls directly in service-layer code. Right call for atomicity, wrong layer. Two options:
- Move transactional versions into `db/` as functions accepting `&mut Transaction<'_, Sqlite>`
- Accept the inline SQL and delete the now-dead db functions

Second option is simpler — the code is correct, it just needs cleanup.

**Notification table grows forever**
No read endpoint, no cleanup, no pagination. `notify()` inserts a row on every ticket event. Either add a `GET /notifications` endpoint or drop the DB persistence and keep only the `tracing::info!` log line.

**PDF generation blocks the async thread**
`monthly_report` and large exports run synchronously on an async thread. Both should be wrapped in `tokio::task::spawn_blocking(|| { ... })`.

**Background tasks have no restart logic**
`recurring_tickets` and `hard_delete_expired_users` are spawned once in `main`. If either panics, the task stops silently with no log. Needs a respawn loop or at minimum a panic handler.

**COMMON_PASSWORDS uses linear scan**
`&[&str]` with `.contains()` is O(n). A `HashSet<&str>` or compile-time `phf::Set` is idiomatic for a lookup table.

**config.rs stores SMTP password as String**
`Config` is cloned into every handler via `FromRef`. The SMTP password is not zeroed on drop. Fine at this scale — worth noting for a future hardening pass.

---

### Scalability

**SQLite ceiling**
Single writer, serialised writes. Fine for one desk and dozens of daily tickets. Migration path to Postgres is one `Cargo.toml` change + query adjustments — nothing in the architecture prevents it.

**WAL mode not enabled**
Default journal mode blocks readers during writes. One line fix:
```rust
.journal_mode(SqliteJournalMode::Wal)
```
on `SqliteConnectOptions`. No migration needed.

**No connection pool tuning**
sqlx defaults to max 10 connections. Fine for now — worth setting explicitly so the limit is visible.

**In-memory rate limiter resets on restart**
An attacker who knows the deploy cadence can time attempts around it. Not exploitable at this scale.

**GET /admin/clients is unbounded**
Returns every client in one response. Tickets are paginated; clients aren't. Needs the same treatment at any real scale.

---

### Useful Features to Add

| Priority | Feature | Effort | Notes |
|----------|---------|--------|-------|
| High | `GET /health` | Trivial | DB ping + uptime. Every load balancer needs this. Currently no way to check server health without an authed request. |
| High | File download endpoint | Small | Attachments are stored but unserveable. This is a broken feature. |
| High | Client password change | Small | Clients can't change their own password. Desk-only `PATCH /auth/password` already exists — extend it. |
| Medium | Ticket filtering & search | Medium | `?status=open&priority=high&q=login`. Currently fetch-everything-filter-client-side. |
| Medium | Unread message count | Small | `unread_count` on `TicketResponse` or `GET /tickets/unread`. Minimum for a usable UI. |
| Medium | WAL mode | Trivial | One line. Immediate improvement. |
| Low | Webhook support | Medium | POST events to a registered URL per client. Email infrastructure is already the pattern. |
| Low | Paginate GET /admin/clients | Small | Consistent with tickets. |
| Low | Background task restart logic | Small | Respawn loop or panic handler in `tasks.rs`. |
| Low | spawn_blocking for PDF/export | Small | Stop blocking the async executor. |

---

---

## Frontend Integration (React + Vite)

**Rating: 7/10** — clean API, one blocker to handle first.

---

### The Blocker — CORS

Vite runs on `localhost:5173`, backend on `localhost:3000`. The browser blocks every API call. The `SameSite=Strict` refresh cookie also won't transmit cross-origin.

Fix with a Vite proxy — no backend changes needed:

```ts
// vite.config.ts
export default defineConfig({
  server: {
    proxy: {
      '/auth':    'http://localhost:3000',
      '/tickets': 'http://localhost:3000',
      '/admin':   'http://localhost:3000',
      '/reports': 'http://localhost:3000',
    }
  }
})
```

Do this first or nothing else works. In production the frontend is served from the same origin via `ServeDir` and the problem disappears entirely.

---

### Auth Flow — Medium complexity

Three things needed:

1. Store the access token in **memory**, not localStorage (XSS risk)
2. Interceptor that retries on 401 by hitting `/auth/refresh` first
3. Handle the race condition where multiple requests 401 at the same time — only one refresh should fire

```ts
axios.interceptors.response.use(null, async (error) => {
  if (error.response?.status === 401 && !error.config._retry) {
    error.config._retry = true
    await axios.post('/auth/refresh')  // cookie sent automatically
    error.config.headers['Authorization'] = `Bearer ${getNewToken()}`
    return axios(error.config)
  }
  return Promise.reject(error)
})
```

30–50 lines total. Standard work, can't skip it.

---

### Everything Else

| Feature | Effort | Notes |
|---------|--------|-------|
| Login / logout | Easy | Standard POST, store returned token |
| Ticket list | Easy | `?page=1&limit=50`, response includes `total` |
| Create ticket | Easy | JSON body |
| Post message | Easy | `FormData` with `body` + optional `file` |
| File upload | Easy | Standard multipart — browser `FormData` handles it |
| PDF report download | Easy | `fetch` → blob → `URL.createObjectURL` |
| Magic link exchange | Easy | Read `?token=` from URL, POST to `/auth/magic` |
| Error handling | Easy | Every error is `{ "error": "..." }` — one global handler covers all |
| Change password | Easy | Two fields, standard PATCH |
| Internal notes | Easy | Desk-only, standard GET/POST |

---

### What's Missing

**No real-time updates.** No WebSockets, SSE, or long-polling. If a client sends a message the desk screen won't update. Options: poll every 30s (ugly but works) or add SSE to the backend.

**No TypeScript types.** No OpenAPI spec, no generated types. Hand-write interfaces per request/response. Consider `utoipa` on the backend later — it generates an OpenAPI spec and works well with Axum.

**No attachment download.** Files upload fine, paths are returned in threads, but there's no download route. The frontend can display the path but can't fetch the file.

---

### Realistic Timeline

| Phase | Time |
|-------|------|
| Vite proxy + axios interceptors + auth flow | Half day |
| Ticket list, create, view, transitions | 1–2 days |
| Message thread + file upload UI | 1 day |
| Admin panel (clients, export, magic links) | 1 day |
| Desk internal notes | Half day |
| Reports (PDF download button) | 2 hours |
| Polish, error states, loading states | 1–2 days |
| **Total** | **~1 week** |
