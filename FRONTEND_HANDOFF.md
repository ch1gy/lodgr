# Lodgr — Frontend Handoff

> **This is the single source of truth.** Read it fully before touching any file.

---

## Project overview

Lodgr is a self-hosted support desk. Rust/Axum + SQLite backend (complete, do not touch). React 18 + TypeScript + Vite frontend. Your job is the frontend only.

**Repo:** `ch1gy/lodgr` on GitHub, `master` branch  
**Dev server:** `cd frontend && npm run dev` — runs at `http://localhost:5173`, proxies API to `:3000`  
**Backend:** `cargo run` from repo root — runs at `http://localhost:3000`  
**TypeScript:** `cd frontend && npx tsc --noEmit` — must stay clean before every commit  
**Tests:** Never run `cargo test` locally (broken on Windows). CI only.  
**Push rule:** Never `git push` without the user saying "push!" Commit freely, wait for approval.

---

## Design system — match this exactly

Every screen uses the same editorial magazine vocabulary. No exceptions.

### Tokens (all in `frontend/src/styles/tokens.css`)
```
Light:  --cream: #f2ede4  --ink: #0d0d0d  --red: #c8322a  --mid: #6a6560  --rule: #c8c2b8
Dark:   --cream: #15110d  --ink: #ede8df  --red: #e0564a
Accent: --amber: #b88a2c (acknowledged/high)  --slate: #6a7585 (pending)  --green: #4f6f4a
```

**Never use raw hex values. Always `var(--*)`.**

### Fonts
- `var(--serif)` = DM Serif Display italic — headlines, wordmark, big numbers
- `var(--mono)` = DM Mono — labels, IDs, timestamps, meta, buttons
- `var(--sans)` = Archivo Narrow — body text, descriptions

### Buttons — always use `.lg-bt` system (in `buttons.css`)
```
.lg-bt--solid   → ink background, cream text (primary actions)
.lg-bt--ghost   → transparent + ink border (secondary)
.lg-bt--danger  → transparent + red border
.lg-bt--text    → no border, underline on hover (tertiary)
.lg-bt--icon    → 36×36 square
```
Add `is-loading` for pending state (spinner appears). No one-off button styling.

### Layout vocabulary
- **Masthead** — top bar on every auth'd page: hamburger (mobile) · Lodgr. · nav · avatar
- **Drawer** — slide-out left-side nav on mobile, triggered by hamburger
- **No bottom tab bar** — retired. `tokens.css` has `.lg-tabbar { display: none !important }`. Do not re-add it anywhere.
- **Cards / rows** — editorial grid, big italic serif numbers, mono labels
- **Modals** — centered overlay with `.lg-mdl` system from `v2.css`

---

## File map

```
frontend/src/
├── main.tsx                  entry — applies theme sync before React mounts, imports buttons.css
├── App.tsx                   route table + provider stack (BrowserRouter → QueryClient → Theme → Auth → …)
├── auth/
│   └── AuthContext.tsx        token, user, profile, isDesk, isScoped, loading, login/logout/redeemMagicLink
├── theme/
│   ├── ThemeContext.tsx       light/dark, localStorage, system prefers-color-scheme, View Transitions reveal
│   └── MorphContext.tsx       shared-element morph context for row→detail transition
├── api/
│   ├── client.ts             axios instance + tokenStore + silent-refresh interceptor
│   ├── tickets.ts            all ticket endpoints
│   ├── admin.ts              clients, invoices, reports, desk-profile endpoints
│   ├── auth.ts               login, logout, magic, me, changePassword, sessions
│   └── types.ts              ALL TypeScript types — authoritative, do not duplicate
├── components/
│   ├── Masthead.tsx           hamburger + drawer (mobile) / full nav (desktop)
│   ├── DraggableRow.tsx       ⛔ DO NOT TOUCH — iOS swipe-to-triage, axis-lock, haptics
│   ├── Dropdown.tsx          portalled fixed-position dropdown (escapes overflow containers)
│   ├── PriorityBars.tsx       4-bar priority chip (low/medium/high/urgent)
│   ├── StatusPill.tsx         open/ack/pending/closed pill with animated ring on "open"
│   ├── SlaOdometer.tsx        rolling digit countdown to due_date
│   ├── Toast.tsx              <ToastProvider> + useToast().show('msg')
│   ├── Segmented.tsx          sliding-pip segmented control
│   ├── PasswordGenerator.tsx  passphrase generator
│   ├── ConfirmModal.tsx        generic confirm dialog
│   ├── CreateTicketModal.tsx  create ticket form (uses filing.css stamp animation)
│   ├── CommandPalette.tsx     ⌘K palette (desk only)
│   ├── ProtectedRoute.tsx     auth gate, `deskOnly` prop
│   ├── ErrorBoundary.tsx      top-level React error catch
│   └── BottomTabBar.tsx       DEAD — display:none in CSS. Ignore it.
├── pages/
│   ├── LoginPage.tsx          centered card, password↔magic-link inline swap
│   ├── MagicLandingPage.tsx   /auth/magic?token= one-shot exchange
│   ├── TicketListPage.tsx     ⚠ see §Queue below — has fixes, DO NOT REWRITE
│   ├── TicketDetailPage.tsx   3-column: queue rail · article · props rail
│   ├── ClientsPage.tsx        client roster with Dropdown actions
│   ├── InvoicesPage.tsx       invoices list + create/edit modal
│   ├── ReportsPage.tsx        monthly PDF download
│   └── SettingsPage.tsx       password change + desk profile + sessions
└── styles/
    ├── tokens.css    design tokens, masthead, hamburger drawer, dark theme, animations
    ├── buttons.css   .lg-bt system + consistency bridge for legacy button classes
    ├── list.css      ticket queue — row layout, Draft A mobile cards, FAB, swipe rails
    ├── detail.css    3-column detail layout, queue/props rails, mobile sheet
    ├── login.css     sign-in page + magic landing
    ├── v2.css        admin pages (clients, invoices, reports, settings) + modals
    ├── filing.css    stamp-press + fly-up animation for CreateTicketModal
    ├── dropdown.css  dropdown menu
    ├── segmented.css segmented control
    ├── sla.css       SLA odometer rolling digits
    └── palette.css   ⌘K command palette
```

---

## Auth model

Two roles: `desk` (admin/operator) and `client` (end user).  
Two session types: `full` (password login) and `scoped` (magic link scoped to one ticket).

`useAuth()` returns:
| Field | Type | Meaning |
|-------|------|---------|
| `token` | `string\|null` | raw JWT |
| `user` | `JwtPayload\|null` | decoded JWT (sub, role, session_type, ticket_scope, exp) |
| `profile` | `{id,name,email}\|null` | from GET /auth/me — has the display name and email |
| `isDesk` | `boolean` | true when role === 'desk' |
| `isScoped` | `boolean` | true when session_type === 'scoped' (magic link) |
| `loading` | `boolean` | true while silent refresh is in flight on first mount |

---

## Complete API reference

**Base URL:** `http://localhost:3000` (proxied by Vite in dev — use relative paths)  
**Auth header:** `Authorization: Bearer <token>` (handled automatically by axios interceptor)  
**Refresh:** `POST /auth/refresh` — httpOnly cookie, called automatically by interceptor on 401

### Auth endpoints
| Method | Path | Body | Response |
|--------|------|------|----------|
| POST | `/auth/login` | `{email, password}` | `{access_token}` |
| POST | `/auth/refresh` | — (uses cookie) | `{access_token}` |
| POST | `/auth/logout` | — | 204 (clears cookie) |
| GET | `/auth/me` | — | `{id, name, email, role, created_at}` |
| POST | `/auth/magic` | `{token}` | `{access_token}` (one-shot, destroys token) |
| PATCH | `/auth/password` | `{current_password, new_password}` | `{access_token}` |
| GET | `/auth/sessions` | — | `SessionResponse[]` |
| DELETE | `/auth/sessions/:id` | — | 204 |

### Ticket endpoints
| Method | Path | Notes |
|--------|------|-------|
| GET | `/tickets?page=1&limit=50` | `→ {tickets: TicketResponse[], total, page, limit}` |
| POST | `/tickets` | `CreateTicketPayload` → `{id}` |
| GET | `/tickets/:id` | `→ TicketWithThread` (includes `thread: ThreadEntry[]`) |
| PATCH | `/tickets/:id` | update priority/category/due_date/type/recurring |
| DELETE | `/tickets/:id` | desk only |
| PATCH | `/tickets/:id/ack` | desk: open OR pending → acknowledged |
| PATCH | `/tickets/:id/pend` | desk: open OR acknowledged → pending |
| PATCH | `/tickets/:id/close` | desk: acknowledged → closed |
| POST | `/tickets/:id/message` | multipart `body` + optional `file` → `{id}` |
| GET | `/tickets/:id/notes` | desk only → `InternalNote[]` |
| POST | `/tickets/:id/notes` | desk only `{body}` → `InternalNote` |
| POST | `/tickets/:id/magic-link` | desk only → `{url}` |

### Admin endpoints (desk only)
| Method | Path | Notes |
|--------|------|-------|
| GET | `/admin/clients` | `→ Client[]` (includes deleted/locked) |
| POST | `/admin/clients` | `CreateClientPayload` → `Client` |
| PATCH | `/admin/clients/:id` | partial update of name/email/address fields |
| DELETE | `/admin/clients/:id` | hard delete — requires body `{confirm: "DELETE"}` |
| POST | `/admin/clients/:id/soft-delete` | archive (sets deleted_at) |
| POST | `/admin/clients/:id/restore` | un-archive |
| POST | `/admin/clients/:id/unlock` | clear login lockout |
| DELETE | `/admin/clients/:id/sessions` | revoke all active sessions |
| POST | `/admin/clients/:id/export` | `→ {export_id, download_url}` |
| GET | `<download_url>` | blob download of export JSON |
| POST | `/admin/clients/:id/magic-link` | full-session client magic link → `{url}` |
| GET | `/admin/clients/:id/sub-clients` | `→ SubClient[]` |
| POST | `/admin/clients/:id/sub-clients` | `{name}` → `SubClient` |
| DELETE | `/admin/sub-clients/:id` | — |
| GET | `/admin/invoices?client_id=` | `→ InvoiceResponse[]` |
| POST | `/admin/invoices` | `CreateInvoicePayload` → `InvoiceResponse` |
| GET | `/admin/invoices/:id` | `→ InvoiceResponse` |
| PATCH | `/admin/invoices/:id` | `UpdateInvoicePayload` → `InvoiceResponse` |
| DELETE | `/admin/invoices/:id` | — |
| GET | `/admin/invoices/:id/print` | returns PDF — open with `window.open(url)` |
| GET | `/admin/desk-profile` | `→ DeskProfile` |
| PUT | `/admin/desk-profile` | `DeskProfile` → `DeskProfile` |
| GET | `/reports/monthly/:client_id/:year/:month` | PDF blob (month is 1-indexed) |

### Ticket status machine
```
open ──→ acknowledged ──→ closed
 │              ↑
 └──→ pending ──┘
```
`can.ack   = status === 'open' || status === 'pending'`  
`can.pend  = status === 'open' || status === 'acknowledged'`  
`can.close = status === 'acknowledged'`

---

## The ticket queue — what's already built (DO NOT REWRITE)

`TicketListPage.tsx` is the most complex page. These features are complete and working:

1. **Draft A stacked cards (mobile ≤540px)** — CSS grid with `grid-template-areas: "av nm sla" / "ttl ttl ttl" / "st st cat"`. The `display: contents` trick on `.lg-row__client` and `.lg-row__title-blk` dissolves wrapper divs so children become direct grid items.

2. **DraggableRow** — iOS-safe touch swipe. Left → ACK (if open/pending), right → CLOSE (if ack). Has axis-lock at 6px delta, 33% width threshold, haptic vibration at swipe threshold, `try/catch` on `setPointerCapture`. **Do not touch this component or list.css swipe styles.**

3. **clientMap** — fetches `GET /admin/clients` and builds `Record<clientId, clientName>`. Rows show "Brand Bistro" not a UUID. Uses `staleTime: 5 * 60_000`.

4. **Mobile header** — eyebrow `"TUESDAY · ISSUE 042"` (day-of-year counter), swipe hint shows on mobile (`.lg-list__swipe-hint`).

5. **Filter tabs** — ALL / OPEN / ACK / PENDING / CLOSED. "ACKNOWLEDGED" shortened to "ACK". CLOSED tab has `.lg-list__filt-tab--desk` class (hidden on mobile in CSS).

6. **FAB** — `<button className="lg-fab">+</button>` at bottom of component, before `{createOpen && <CreateTicketModal>}`. CSS hides it on desktop, shows `position: fixed` on mobile.

---

## Bugs to fix (confirmed, in working tree but not committed)

### BUG-1: Login form fields invisible
**File:** `frontend/src/styles/login.css` line ~103  
**Problem:** `.lg-login__swap` overrides the parent `lg-rise` animation but was missing `forwards` fill-mode, so after 420ms the swap div snaps back to `opacity: 0`.  
**Fix:** Already applied in working tree:
```css
.lg-login__swap { animation: lg-swapin 420ms cubic-bezier(0.16, 1, 0.3, 1) forwards; }
```

### BUG-2: Invoice status button shows as solid black rectangle
**File:** `frontend/src/pages/InvoicesPage.tsx` line ~681  
**Problem:** `color: var(--paper)` — `--paper` is not a defined token, renders as transparent/empty, text invisible on `var(--ink)` background.  
**Fix:** Already applied in working tree:
```
color: invoice.status === s ? 'var(--cream)' : 'var(--mid)',
```

### BUG-3: Clients ACTIONS dropdown hidden behind overflow container
**File:** `frontend/src/components/Dropdown.tsx`  
**Problem:** `.lg-cl-rows` has `overflow-y: auto` which clips the absolutely-positioned dropdown menu even when it has `z-index: 300`.  
**Fix:** Already applied in working tree — Dropdown now uses `ReactDOM.createPortal` to render at `document.body` with `position: fixed`, tracking trigger position via `getBoundingClientRect()`.

### BUG-4: FAB floating too high on mobile
**File:** `frontend/src/styles/list.css` lines 444–453  
**Problem:** FAB `bottom` is `max(80px, calc(60px + env(safe-area-inset-bottom, 0px)))` — was calculated to clear the old BottomTabBar (60px tall). Tab bar is retired, so FAB sits 80px above the floor instead of near the bottom edge.  
**Fix:** Change the FAB bottom value:
```css
.lg-fab {
  position: fixed;
  right: max(20px, env(safe-area-inset-right, 0px));
  bottom: max(28px, env(safe-area-inset-bottom, 16px));
  /* rest unchanged */
}
```

---

## Pages — what each should look like

All screenshots to match are the mockups that design Claude delivered in the `new/` folder. If you need a visual reference for any page, read:
- `new/frontend/src/pages/<PageName>.tsx` — the reference implementation
- `new/frontend/src/styles/v2.css` — the admin page layout spec
- `new/frontend/src/styles/list.css` — the queue layout spec (this is the canonical version — copy it to `frontend/src/styles/list.css` exactly)

### Login (`/login`)
Centered card, max-width 360px. Wordmark top, horizontal rule, email + password fields with animated red underline on focus, solid ink "Sign in →" button. "Client? Email me a link instead" at the bottom swaps to magic-link mode. Theme toggle in top-right corner. See `login.css`.

### Ticket queue (`/tickets`)
- Desktop: full-width rows, 6 columns (number · client avatar · title block · status · due · chevron)
- Mobile ≤540px: Draft A stacked cards (3 rows: top=avatar+name+SLA / middle=title / bottom=status+category)
- Filter bar scrolls horizontally on mobile
- FAB (ink square, `+`) fixed bottom-right on mobile

### Ticket detail (`/tickets/:id`)
Three-column layout: left rail (queue list, collapsible to 56px), center (article with thread + composer), right rail (props/controls, collapsible). Below 1280px: drop left rail. Below 1024px: drop right rail, show "Controls" button that opens a bottom sheet. Mobile bottom sheet slides up with spring animation.

### Clients (`/clients`) — desk only
Editorial header "The roster / 01" (count in red mono). Stat row (active/locked/archived counts). Filter tabs ALL / ACTIVE / LOCKED / ARCHIVED. Search input + "+ New client" button. Each client is a `.lg-cl-row`: avatar square + name/email + stat blocks + MAGIC LINK button + ACTIONS dropdown. On mobile ≤540px the row collapses: avatar + name+email stacked + buttons below full-width.

### Invoices (`/invoices`) — desk only
Header "Invoices / 001" style. Filter tabs ALL / DRAFT / SENT / PAID. Each invoice is a collapsible row: number · client · due · amount · status · action buttons. Expanded shows status toggle (draft/sent/paid using `.lg-bt` classes), line items table, recurring info. "+ New invoice" opens a modal.

### Reports (`/reports`) — desk only
Two-column editorial: left is the big headline + instructions, right is the form (client picker → year → month → Download PDF button). Button: `.lg-bt lg-bt--solid`.

### Settings (`/settings`)
Left nav with serif section headings. Right body changes per section: Password (form + PasswordGenerator), Desk profile (grid of fields), Sessions (list with Revoke buttons), Sign out (confirm pattern). All buttons use `.lg-bt` system.

---

## Design reference files in `new/`

`new/frontend/` is design Claude's reference implementations. Use them as visual spec. **Do not blindly copy them into `frontend/` wholesale** — our versions of the following are ahead and must not be overwritten:
- `frontend/src/components/DraggableRow.tsx` ← DO NOT overwrite (iOS fixes)
- `frontend/src/styles/list.css` ← the `new/` version IS the canonical spec, already applied
- `frontend/src/pages/TicketListPage.tsx` ← DO NOT overwrite (has clientMap, Draft A, swipe)

Everything else in `new/frontend/src/` can be used as reference or copied if it improves things.

---

## Known gaps — do NOT implement client-side stubs

- `POST /auth/magic-request` — self-serve magic link request by clients. Backend not built yet. `LoginPage` shows "ask the desk" message when magic mode is used.
- Attachment download — `ThreadEntry.attachment_path` exists but backend has no download route. Show filename as text with a "(download unavailable)" note.

---

## Current git state

Commits on master (latest first):
```
fd34fc1  feat(ui): apply design Claude's final handoff — hamburger nav, new login, detail page, button system
36905ae  fix(queue): resolve client name showing as UUID; add mobile eyebrow + swipe hint
b8c47cf  feat(design): integrate design Claude's updated CSS and components
4fc057b  fix(mobile): add comprehensive 540px overrides for settings, reports, invoices, pw-gen
aa5fce3  feat(mobile): implement Draft A stacked card queue layout (§10.2)
```

Working tree has 4 uncommitted bug fixes (BUG-1 through BUG-4 above). Run `git diff` to see them. TypeScript is clean. Commit these first, then work on remaining issues.

---

## How to start

```bash
# 1. Run the backend
cargo run
# → listening on :3000

# 2. Run the frontend
cd frontend
npm run dev
# → http://localhost:5173

# 3. TypeScript check (run before every commit)
npx tsc --noEmit

# 4. Default desk login
email:    admin@lodgr.local
password: changeme
```

---

## Git rules

1. Never `git push` without user saying "push!"
2. Commit in logical batches after `npx tsc --noEmit` passes
3. Never add `#[allow(...)]` Clippy suppression — fix the code
4. Never run `cargo test` locally — CI only
