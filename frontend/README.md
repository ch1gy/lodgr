# Lodgr — Frontend

Editorial-magazine UI for the Lodgr self-hosted support deck. React + Vite + TypeScript + TanStack Query, wired to the Rust backend per `FRONTEND_HANDOFF.md`.

## Getting started

```bash
# 1. From the repo root, start the backend (see lodgr README)
cargo run -p backend
# → listening on http://localhost:3000

# 2. In another terminal, install + run the frontend
cd frontend
npm install
npm run dev
# → http://localhost:5173
```

The Vite dev server proxies `/auth`, `/tickets`, `/admin`, `/reports` to `http://localhost:3000`. This is essential — the backend uses `SameSite=Strict` cookies and no CORS headers, so cross-origin calls would be blocked.

In production, the backend's `ServeDir` serves the built frontend at `/`, so the same paths work without a proxy.

## File layout

```
frontend/
├── index.html              fonts + root mount
├── package.json
├── tsconfig.json
├── tsconfig.node.json
├── vite.config.ts          proxy config — read this if API calls 404
└── src/
    ├── main.tsx            apply theme then mount React
    ├── App.tsx             provider stack + route table
    │
    ├── api/
    │   ├── client.ts       axios instance + auto-refresh interceptor
    │   ├── auth.ts         login / logout / magic / change-password
    │   ├── tickets.ts      tickets + thread + notes + magic-link
    │   └── types.ts        TypeScript shapes mirrored from the handoff
    │
    ├── auth/
    │   └── AuthContext.tsx access-token state, silent refresh on boot,
    │                       JWT decode → { isDesk, isScoped }
    │
    ├── theme/
    │   └── ThemeContext.tsx light/dark + system preference + View
    │                        Transitions circular reveal
    │
    ├── components/
    │   ├── Masthead.tsx        editorial top bar w/ theme toggle
    │   ├── ProtectedRoute.tsx  route gate, optional deskOnly
    │   ├── StatusPill.tsx      status indicator (open pulses)
    │   └── PriorityBars.tsx    1-4 vertical bars, color shifts
    │
    ├── pages/
    │   ├── LoginPage.tsx        password + magic-link tabs, lockout countdown
    │   ├── MagicLandingPage.tsx /auth/magic?token=… exchange
    │   ├── TicketListPage.tsx   editorial queue, 30s polling
    │   └── TicketDetailPage.tsx article + thread + composer + rails
    │
    └── styles/
        ├── tokens.css      design tokens, atoms, theme toggle, motion
        ├── login.css       sign-in page
        ├── list.css        queue page
        └── detail.css      single ticket page
```

## Design system at a glance

- **Palette**: cream (`#f2ede4`) / ink (`#0d0d0d`) / red (`#c8322a`). Dark mode swaps to a near-black `oklch(0.10 0.01 60)` with off-white ink.
- **Fonts** (loaded in `index.html`): DM Serif Display italic (headings, "Lodgr." wordmark), DM Mono (labels, IDs, timestamps), Archivo Narrow 500 (body copy).
- **Editorial chrome**: every signed-in page mounts `<Masthead>` (issue-number-of-the-year, wordmark, nav, theme switch, sign out, avatar).
- **Status**: `open` is the only state that pulses (a `.dot` animation) — that's the desk's "needs a human" signal.

## Motion

All transitions use a small set of tokens defined in `tokens.css`:

- `--ease-out: cubic-bezier(0.16, 1, 0.30, 1)` — fast-start soft-settle (the workhorse).
- `--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1)` — for tactile button presses.
- `--dur-fast`: 200ms (hover), `--dur-base`: 320ms (entries), `--dur-slow`: 520ms (page-level fades).

Theme toggle uses the View Transitions API where available (Chrome 111+, Safari 18+) for a circular reveal radiating from the toggle button. Browsers without VT just swap.

`prefers-reduced-motion` is honored — animations collapse to ~0ms.

## Auth flow notes

- **Access token** — short-lived (15 min), in-memory only (XSS hygiene). Sent as `Authorization: Bearer <token>`.
- **Refresh token** — 7-day httpOnly cookie, browser sends automatically. On boot, `AuthProvider` makes a silent `POST /auth/refresh` so a tab reload doesn't bounce the user to login.
- On 401 the axios interceptor refreshes (single-flight lock prevents thundering herd), retries once, then propagates the error so a stale tab routes to `/login`.
- Magic-link landing (`/auth/magic?token=…`) calls `POST /auth/magic`, then if the JWT carries `ticket_scope` it routes straight to that ticket, otherwise to the queue.

## Role gating

`useAuth()` exposes `isDesk` and `isScoped`. The UI hides:

- **Internal-note tab + sidebar section + transitions + magic-link generator** for clients.
- **Queue rail** for scoped sessions (they only see one ticket anyway).
- **Clients / Reports** masthead links for clients.

The server enforces all of this; the client just avoids rendering dead controls.

## Known gaps (from the handoff)

1. **No attachment download route** — thread messages render an attachment pill but no download. Wait for the backend.
2. **No SSE / WebSocket** — pages poll every 30s (`refetchInterval` on the relevant queries).
3. **No client-side password change** — magic link is the recovery path. The "Forgot password" link on the login page switches to the magic-link tab.

## Building for prod

```bash
npm run build
# → outputs to dist/
```

The Rust backend serves `dist/` via `ServeDir` at `/`. Make sure your release build picks up the built assets per the lodgr repo instructions.
