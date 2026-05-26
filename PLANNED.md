# Planned Features

## Multi-desk support (v1.x)

The current system is designed for a single desk agent (`desk@local`). Multi-desk
support is planned for a future version.

### What will be needed

**Schema changes**
- A `desk_accounts` table (or extending `users`) with individual desk credentials
- A `super_admin` role or separate admin bootstrap mechanism to create desk accounts
- Desk account creation restricted to `super_admin` — desk agents must not be able
  to create other desk agents

**Authorization changes**
- Client ownership scoping per desk: each client is assigned to a specific desk agent
  (or a desk team), and desk agents can only manage their own clients
- The `DeskUser` extractor will need to carry the desk agent's ID so service functions
  can enforce ownership
- All admin routes (list clients, export, hard delete, etc.) must filter by the
  requesting desk agent's assigned clients

**Security implications to design before implementation**
- Session isolation: a desk agent's sessions must not be accessible or revocable by
  another desk agent (only by super_admin)
- Magic link audit trail must record WHICH desk agent generated a link
- Super-admin account must follow the same hardened auth (argon2id, no default
  password, PATCH /auth/password enforcement) as desk accounts
- Super-admin must NOT be able to read message thread content (desk-agent-level
  access only) — separation of administrative and operational privilege
- Role escalation: there must be no code path that allows a `client` or `desk` role
  to create a `super_admin` account

### Migration path

Existing `desk@local` seeded account will be preserved and treated as a legacy
single-desk account. Before multi-desk is enabled in production, the operator must:
1. Create explicit desk accounts with real credentials
2. Assign each existing client to a desk account
3. Disable or rename `desk@local`

This section will be replaced by implementation notes once the feature is underway.

---

## Account lockout recovery via magic link (v1.x)

Account lockout is implemented (migration 010, `failed_attempts` + `locked_until`
columns on users, escalating backoff: 1 min → 5 min → 15 min → 1 hour → permanent).

Currently, a permanently locked account can only be recovered by direct DB access
(`UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE ...`).
This is acceptable for a single-operator deployment but does not scale.

### Planned behaviour

When an account is permanently locked:
- **Client locked** — desk triggers `POST /admin/clients/:id/unlock-link`, which
  sends a magic link to the client's registered email. Clicking the link
  authenticates them and resets the lockout counter.
- **Desk locked** — the server automatically sends a magic link to the desk
  account's registered email address. The link authenticates desk and resets
  the counter. Requires SMTP to be configured; if SMTP is not set up, DB
  recovery remains the fallback.

### What will be needed

- SMTP must be configured and working (`SMTP_HOST` set in `.env`)
- The existing magic link infrastructure handles delivery and exchange;
  the unlock flow adds a lockout-counter reset on successful exchange
- A new `POST /admin/clients/:id/unlock-link` route (desk only)
- Logic in `exchange_magic_link` to clear `failed_attempts` and `locked_until`
  for the user being authenticated
- For the desk self-unlock: trigger the magic link send automatically when
  permanent lockout is reached, rather than requiring a manual desk action

### Security notes

- Magic link exchange already marks the token as used (single-use) — no replay risk
- The link proves ownership of the registered email, which is the correct
  identity verification step for account recovery
- If the registered email is also compromised, this flow cannot help —
  DB recovery remains the break-glass option in that scenario
- Rate-limit the `unlock-link` endpoint to prevent it being used as an
  email-spam vector
