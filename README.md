# Ticket Support — Rust/Axum Backend

A ticket-support system with JWT authentication, refresh-token rotation,
per-IP rate limiting, and AES-256-GCM encryption of all message content.

---

## Stack

| Crate | Purpose |
|---|---|
| Axum 0.7 | HTTP framework |
| sqlx 0.8 | Async SQLite driver |
| Tokio 1 | Async runtime |
| jsonwebtoken 9 | JWT access tokens (HS256) |
| argon2 0.5 | Password hashing (argon2id) + key derivation |
| aes-gcm 0.10 | Thread body encryption (AES-256-GCM) |
| bcrypt 0.15 | Legacy hash verification only |
| rand 0.8 | Cryptographically secure RNG |

---

## Project Structure

```
ticket_support/
├── .env                   local secrets — NEVER commit (in .gitignore)
├── .env.example           template for all required variables
└── backend/
    ├── migrations/
    │   ├── 001_init.sql
    │   ├── 002_sessions_and_indexes.sql
    │   └── 003_thread_encryption.sql
    ├── static/index.html  minimal browser test UI
    └── src/
        ├── main.rs        startup: config, key derivation, router
        ├── config.rs      env var loading + validation
        ├── crypto.rs      AES-256-GCM + Argon2id key derivation
        ├── dto.rs         API response types (only place Serialize is derived)
        ├── error.rs       central AppError, IntoResponse
        ├── models.rs      DB row types — no Serialize
        ├── rate_limit.rs  per-IP token-bucket rate limiter
        ├── ticket_status.rs  isolated status-transition logic
        ├── middleware.rs  AuthUser, DeskUser, RefreshTokenCookie
        ├── auth.rs        login / refresh / logout handlers
        ├── notify.rs      notify() — stdout + DB insert
        ├── db/            repository layer — SQL only
        └── services/      business logic layer
            └── routes/    HTTP handlers — HTTP concerns only
```

---

## Setup

### 1. Configure `.env`

```bash
cp .env.example .env
```

Edit `.env` and fill in every value. Generate strong secrets:

```bash
openssl rand -hex 32   # → JWT_SECRET
openssl rand -hex 16   # → ENCRYPTION_SALT (set once, never change)
```

### 2. Run

```bash
cargo run -p backend
```

Server listens on `BIND_ADDR` (default `127.0.0.1:3000`).

On first run:
1. Creates `data/support.db` and runs migrations
2. Creates `uploads/` for file attachments
3. Derives the AES-256-GCM key (~2 s — Argon2id with 64 MiB)
4. Cleans up any expired/revoked sessions
5. Seeds `desk@local` / `changeme` (warns if still default)

---

## Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | no | `sqlite://data/support.db` | SQLite file path |
| `BIND_ADDR` | no | `127.0.0.1:3000` | TCP bind address. Set `0.0.0.0:3000` to expose on all interfaces (e.g. behind a reverse proxy). |
| `JWT_SECRET` | **yes** | — | HS256 signing key, ≥ 32 bytes. Server panics at startup if shorter. Generate: `openssl rand -hex 32` |
| `ACCESS_TOKEN_TTL_SECS` | no | `900` | Access token lifetime (15 min) |
| `REFRESH_TOKEN_TTL_SECS` | no | `604800` | Refresh token lifetime (7 days) |
| `COOKIE_SECURE` | no | `true` | Set `; Secure` flag on the refresh cookie. Set `false` only for plain-HTTP local dev. **Must be `true` in any HTTPS deployment.** |
| `ENCRYPTION_PASSPHRASE` | **yes** | — | Passphrase for AES key derivation (min 16 chars) |
| `ENCRYPTION_SALT` | **yes** | — | Hex-encoded random salt, **fixed for the lifetime of the database**. Generate once: `openssl rand -hex 16` |

---

## ⚠ Encryption Key Warning

> **`ENCRYPTION_PASSPHRASE` and `ENCRYPTION_SALT` are the only way to decrypt
> stored thread message bodies. If either value is lost or changed after the
> first run, every thread entry in the database is permanently unrecoverable.
> There is no recovery path.**
>
> - Back up `.env` securely and **separately** from the database file.
> - Never commit `.env` to version control (it is in `.gitignore`).
> - Never change `ENCRYPTION_SALT` after data has been written.
> - The `data/` directory and `uploads/` directory are also in `.gitignore` —
>   back them up separately.

---

## Auth Flow

```
POST /auth/login
  → { access_token }  +  Set-Cookie: refresh_token=…; HttpOnly; Path=/auth[; Secure]

POST /auth/refresh   (browser sends cookie automatically)
  → { access_token }  +  rotated refresh cookie
  → replayed rotated token → ALL sessions deleted (theft detection)

POST /auth/logout
  → 204  +  Max-Age=0 cookie (cleared)
```

Access tokens expire after `ACCESS_TOKEN_TTL_SECS`. Call `/auth/refresh` before
expiry. `/auth/login` and `/auth/refresh` are rate-limited to 5 req/s per IP
with a burst of 10.

---

## State Machine

```
open ──(PATCH /tickets/:id/ack)──▶ acknowledged ──(PATCH /tickets/:id/close)──▶ closed
```

All other transitions return `400`. Logic lives exclusively in
`src/ticket_status.rs::transition()`.

---

## Security Headers

Every response carries:
- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Content-Security-Policy: default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'`
- `Strict-Transport-Security: max-age=63072000; includeSubDomains`

---

## curl Examples

Replace `TOKEN` with the JWT from `/auth/login`.
Replace `TICKET_ID` and `CLIENT_ID` with real UUIDs.

### Auth

**Login**
```bash
curl -sc cookies.txt -X POST http://localhost:3000/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"desk@local","password":"changeme"}'
# → { "access_token": "eyJ..." }
```

**Refresh access token**
```bash
curl -sb cookies.txt -c cookies.txt -X POST http://localhost:3000/auth/refresh
```

**Logout**
```bash
curl -sb cookies.txt -X POST http://localhost:3000/auth/logout
```

---

### Admin (desk only)

**Create client**
```bash
curl -X POST http://localhost:3000/admin/clients \
  -H 'Authorization: Bearer TOKEN' \
  -H 'Content-Type: application/json' \
  -d '{"name":"Alice","email":"alice@example.com","password":"hunter2abc"}'
```

**Revoke all sessions for a client**
```bash
curl -X DELETE http://localhost:3000/admin/clients/CLIENT_ID/sessions \
  -H 'Authorization: Bearer TOKEN'
```

---

### Tickets

**List**
```bash
curl http://localhost:3000/tickets -H 'Authorization: Bearer TOKEN'
```

**Create**
```bash
curl -X POST http://localhost:3000/tickets \
  -H 'Authorization: Bearer TOKEN' \
  -H 'Content-Type: application/json' \
  -d '{"title":"Cannot log in","description":"Getting 401 every time."}'
```

**View ticket + thread** *(bodies are decrypted transparently)*
```bash
curl http://localhost:3000/tickets/TICKET_ID -H 'Authorization: Bearer TOKEN'
```

**Acknowledge** *(desk only — open → acknowledged)*
```bash
curl -X PATCH http://localhost:3000/tickets/TICKET_ID/ack \
  -H 'Authorization: Bearer TOKEN'
```

**Close** *(desk only — acknowledged → closed)*
```bash
curl -X PATCH http://localhost:3000/tickets/TICKET_ID/close \
  -H 'Authorization: Bearer TOKEN'
```

---

### Messages

**Post text message** *(body encrypted before storage)*
```bash
curl -X POST http://localhost:3000/tickets/TICKET_ID/message \
  -H 'Authorization: Bearer TOKEN' \
  -F 'body=Here is my reply.'
```

**Post with attachment** *(max 10 MiB)*
```bash
curl -X POST http://localhost:3000/tickets/TICKET_ID/message \
  -H 'Authorization: Bearer TOKEN' \
  -F 'body=See attached.' \
  -F 'file=@/path/to/screenshot.png'
```

Files are saved to `./uploads/<ticket_id>/`. The directory is not served.
