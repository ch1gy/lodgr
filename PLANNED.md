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

| Item | Why | Effort |
|------|-----|--------|
| Ticket server-side filtering | `?status=open&priority=high&q=login`. Currently fetch-all and filter client-side. | Medium |
| Unread message count | `unread_count` on `TicketResponse`. Without it every ticket has to be polled. | Small |
| SSE for real-time updates | Currently polls every 30 s. SSE would push updates instantly. | Medium |
| Paginate `GET /admin/clients` | Consistent with tickets. Needed at any real scale. | Small |

---

## `client_exports` FK schema fix (v1.x)

`client_exports.client_id` has a `REFERENCES users(id)` FK but no `ON DELETE CASCADE`.
The current code deletes `client_exports` rows explicitly in `cascade_delete_user_data`
(services/admin.rs) before deleting the user row. This works, but it means every future
addition of a child table requires updating the shared cascade function.

### The right fix

Migrate `client_exports` to use `ON DELETE CASCADE`:

```sql
CREATE TABLE client_exports_new (
    id         TEXT PRIMARY KEY,
    client_id  TEXT REFERENCES users(id) ON DELETE CASCADE,
    file_path  TEXT NOT NULL,
    created_at TEXT NOT NULL
);
INSERT INTO client_exports_new SELECT * FROM client_exports;
DROP TABLE client_exports;
ALTER TABLE client_exports_new RENAME TO client_exports;
```

Low-risk (small table, no external dependencies) but requires explicit human
review before running against a production database.

---

## Security hardening backlog

From the Phase 2 OWASP review:

| # | Severity | Finding |
|---|----------|---------|
| M4 | MEDIUM | Attachment download loads full file into memory; no download rate limit |
| M7 | MEDIUM | No CORS middleware — needed for non-same-origin deploys |
