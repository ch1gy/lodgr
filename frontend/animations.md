# Lodgr — Animation Implementation Spec

> Handoff doc for Claude Code. Every motion in the **Motion Lab** (`motion/Motion Lab.html`)
> mapped onto the **real** `frontend/` codebase: React 18 + TS + Vite + react-router,
> plain CSS modules under `src/styles/`, tokens in `src/styles/tokens.css`.
>
> **Golden rule:** do not invent new easing/duration values. The tokens already
> exist in `tokens.css` — use them verbatim. If a value below isn't a token, it's
> a one-off and is called out as such.

---

## 0. Tokens you already have (do not redefine)

From `src/styles/tokens.css`:

```css
--ease-out:    cubic-bezier(0.16, 1.00, 0.30, 1.00);  /* entering UI — fast in, soft settle */
--ease-in-out: cubic-bezier(0.65, 0.00, 0.35, 1.00);  /* color cross-fades, theme */
--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1.00);  /* tactile, slight overshoot */
--dur-xfast: 120ms;   --dur-fast: 200ms;
--dur-base:  320ms;   --dur-slow: 520ms;
```

**Mapping note:** the Motion Lab prototype used ad-hoc names (`--d-fast 160ms`,
`--d-med 240ms`, `--d-slow 360ms`). Translate them on the way in:

| Lab value      | Use this token | Notes |
|----------------|----------------|-------|
| `--d-fast 160` | `--dur-fast`   | press states, hovers |
| `--d-med 240`  | `--dur-base`   | most transitions |
| `--d-slow 360` | `--dur-base`   | overlays use base; theme stays `--dur-slow` |
| `ease (.2,.7,.2,1)` | `--ease-out` | the lab curve was a near-twin |

### Global rules — add once, near the top of `tokens.css`

```css
/* Reduced motion: kill everything, no exceptions. */
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.001ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.001ms !important;
    scroll-behavior: auto !important;
  }
}
```

---

## 1. Theme transition — ALREADY DONE, just finish the token cross-fade

`ThemeContext.tsx` already runs the View Transitions circular reveal from the
toggle's click point (520ms, `cubic-bezier(0.16,1,0.3,1)`) with a reduced-motion
+ unsupported-browser fallback. **Do not touch that logic.**

What's missing: on browsers *without* View Transitions, the fallback is an instant
swap. Add a token cross-fade so the fallback is still smooth. Put this in `tokens.css`:

```css
/* Color-only cross-fade. NEVER transition transform/opacity globally — those
   belong to individual components. Scope to color-ish properties only. */
:root, [data-theme] {
  /* applied to elements, not :root itself; see selector below */
}
body, .mast, .ticket-row, .pill, .btn, .field, input, .card, .modal, a, h1, h2, h3, p, span, svg {
  transition:
    background-color var(--dur-slow) var(--ease-in-out),
    color            var(--dur-slow) var(--ease-in-out),
    border-color     var(--dur-slow) var(--ease-in-out),
    fill             var(--dur-slow) var(--ease-in-out),
    stroke           var(--dur-slow) var(--ease-in-out);
}
```

> When the View Transitions path runs, it snapshots before/after so this
> cross-fade is harmless there; on the fallback path it's what makes it fluid.
> Keep the property list color-only — a global `transition: all` will fight every
> other animation in this doc.

The toggle component itself (the knob that springs across): apply to the
`Masthead.tsx` theme button.

```css
.theme-toggle .knob {
  transition: transform var(--dur-slow) var(--ease-spring), background var(--dur-base);
}
[data-theme="dark"] .theme-toggle .knob { transform: translateX(28px); }
```

---

## 2. Buttons — `src/styles/` (shared `buttons.css`, import in `tokens.css` or App)

Every interactive button gets the same transition set + press compression.

```css
.btn {
  /* …existing layout… */
  transition:
    transform        var(--dur-fast) var(--ease-out),
    background-color var(--dur-base) var(--ease-out),
    color            var(--dur-base) var(--ease-out),
    border-color     var(--dur-base) var(--ease-out),
    box-shadow       var(--dur-base) var(--ease-out);
}
.btn:active { transform: scale(0.97); }          /* tactile down-state */

.btn .arr { transition: transform var(--dur-base) var(--ease-out); }
.btn:hover .arr { transform: translateX(3px); }  /* arrow nudge */

.btn--solid:hover {
  background: color-mix(in oklab, var(--ink) 88%, transparent);
  box-shadow: 0 4px 16px -8px color-mix(in oklab, var(--ink) 50%, transparent);
}

/* text-link variant: wiping underline */
.btn--text { position: relative; }
.btn--text::after {
  content: ''; position: absolute; left: 0; bottom: 10px;
  height: 1px; width: 0; background: var(--ink);
  transition: width var(--dur-base) var(--ease-out);
}
.btn--text:hover::after { width: 100%; }
```

### 2a. Loading state (React)

Drive with a boolean prop; CSS does the rest. Spinner is a one-off `0.9s linear`.

```tsx
<button className={`btn btn--solid ${loading ? 'is-loading' : ''}`} disabled={loading}>
  <span className="lbl">Open ticket</span><span className="arr">↗</span>
  <span className="spin" aria-hidden />
</button>
```
```css
.btn.is-loading { pointer-events: none; }
.btn.is-loading .lbl, .btn.is-loading .arr { opacity: 0.3; transition: opacity var(--dur-base); }
.btn .spin {
  position: absolute; left: 50%; top: 50%; width: 14px; height: 14px; margin: -7px 0 0 -7px;
  border: 1.5px solid currentColor; border-top-color: transparent; border-radius: 50%;
  opacity: 0; animation: spin 0.9s linear infinite; transition: opacity var(--dur-base);
}
.btn.is-loading .spin { opacity: 1; }
@keyframes spin { to { transform: rotate(360deg); } }
```

> Wire `loading` to the React Query mutation's `isPending` (create-ticket,
> change-password, generate-report all already use mutations).

### 2b. Copy-link success morph (React)

Used on the magic-link modal + anywhere a link/password is copied. Button
cross-fades to `--green`, a checkmark draws in via `stroke-dashoffset`.

```tsx
const [copied, setCopied] = useState(false);
function onCopy() {
  navigator.clipboard.writeText(url);
  setCopied(true);
  setTimeout(() => setCopied(false), 1800);
}
```
```css
.copy-btn { transition: background var(--dur-base) var(--ease-out), transform var(--dur-fast) var(--ease-out); }
.copy-btn:active { transform: scale(0.97); }
.copy-btn.is-copied { background: var(--green); }
.copy-btn .check { width: 0; overflow: hidden; transition: width var(--dur-base) var(--ease-out); }
.copy-btn.is-copied .check { width: 14px; }
.copy-btn svg path {
  stroke-dasharray: 28; stroke-dashoffset: 28;
  transition: stroke-dashoffset 320ms var(--ease-out) 80ms;  /* 80ms wait so fill lands first */
}
.copy-btn.is-copied svg path { stroke-dashoffset: 0; }
```
Checkmark SVG: `<svg viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`

---

## 3. Selection — segmented control, tabs, checkbox, switch

### 3a. Segmented control + tabs — the **shared sliding pip**

The key idea: do **not** animate each cell's background. Render ONE highlight
element and move it (`left` + `width`) under the active cell. New component
`components/Segmented.tsx`.

```tsx
function Segmented({ options, value, onChange, redValue }: {
  options: string[]; value: string; onChange: (v: string) => void; redValue?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const pipRef = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const root = ref.current, pip = pipRef.current; if (!root || !pip) return;
    const active = root.querySelector<HTMLElement>(`[data-v="${value}"]`); if (!active) return;
    pip.style.left = active.offsetLeft + 'px';
    pip.style.width = active.offsetWidth + 'px';
    pip.style.background = value === redValue ? 'var(--red)' : 'var(--ink)';
  }, [value, redValue]);
  return (
    <div className="seg" ref={ref}>
      {options.map(o => (
        <button key={o} data-v={o} className={`seg__o ${o === value ? 'on' : ''}`} onClick={() => onChange(o)}>{o}</button>
      ))}
      <span className="seg__pip" ref={pipRef} aria-hidden />
    </div>
  );
}
```
```css
.seg { position: relative; display: inline-flex; border: 1px solid var(--rule); width: fit-content; }
.seg__pip {
  position: absolute; top: 0; bottom: 0; z-index: 0;
  transition: left var(--dur-base) var(--ease-spring), width var(--dur-base) var(--ease-spring);
}
.seg__o {
  position: relative; z-index: 1; white-space: nowrap;
  padding: 10px 16px; border-right: 1px solid var(--rule);
  font: …mono 10px…; color: var(--mid);
  transition: color var(--dur-base) var(--ease-out);
}
.seg__o:last-child { border-right: none; }
.seg__o.on { color: var(--cream); }
```

> **Recompute after fonts load.** Mono/serif metrics shift once webfonts swap in,
> which moves `offsetLeft`. Re-run the position calc in a
> `document.fonts.ready.then(...)` (one effect at the app root, or inside the
> component). The Motion Lab does this and it matters.

Tabs are the same pattern with a 1.5px bottom indicator instead of a filled pip —
use it for the ticket-list status filter (All / Open / Acknowledged / Closed) and
the settings nav.

### 3b. Checkbox (draws in) + switch (springs)

```css
.check__box { transition: background var(--dur-base) var(--ease-out); }
.check.on .check__box { background: var(--ink); }
.check__box svg path {
  fill: none; stroke: var(--cream); stroke-width: 2; stroke-linecap: round; stroke-linejoin: round;
  stroke-dasharray: 24; stroke-dashoffset: 24;
  transition: stroke-dashoffset var(--dur-base) var(--ease-out) 80ms;
}
.check.on .check__box svg path { stroke-dashoffset: 0; }

.switch__knob { transition: transform var(--dur-slow) var(--ease-spring), background var(--dur-base); }
.switch.on .switch__knob { transform: translateX(22px); background: var(--cream); }
```
Use the checkbox for the create-ticket "Recurring" field; the switch for settings
notification toggles.

---

## 4. Inputs — floating label + wiping underline

Applies to all `.field` / `input` in login, create-ticket, settings,
change-password. The label rises into an uppercase mono caption and reddens; the
underline wipes in from the left on focus.

```css
.field { position: relative; padding-top: 18px; }
.field input { border: none; padding: 10px 0; width: 100%; background: transparent; outline: none; }
.field::after  { content: ''; position: absolute; bottom: 0; left: 0; width: 100%; height: 1px; background: var(--rule); }
.field::before {
  content: ''; position: absolute; bottom: 0; left: 0; width: 0; height: 1.5px; background: var(--ink); z-index: 2;
  transition: width var(--dur-base) var(--ease-out);
}
.field:focus-within::before { width: 100%; }
.field__label {
  position: absolute; left: 0; top: 28px; color: var(--mid); pointer-events: none;
  font: …mono 13px…;
  transition: top var(--dur-base) var(--ease-out), font-size var(--dur-base) var(--ease-out),
              letter-spacing var(--dur-base) var(--ease-out), color var(--dur-base) var(--ease-out);
}
.field:focus-within .field__label,
.field.is-filled .field__label {
  top: 0; font-size: 9px; letter-spacing: 0.18em; text-transform: uppercase; color: var(--red);
}
```
React: toggle `.is-filled` when `value.length > 0` so the label stays lifted.

---

## 5. List rows — coordinated hover (`src/styles/list.css`)

On `.ticket-row` hover, four transitions fire on one curve: red rule grows from
the left edge, paper tints faintly red, the index numeral colours red, the
trailing arrow slides right.

```css
.ticket-row {
  position: relative;
  transition: background var(--dur-base) var(--ease-out);
}
.ticket-row::before {
  content: ''; position: absolute; left: 0; top: 0; bottom: 0; width: 0; background: var(--red);
  transition: width var(--dur-base) var(--ease-out);
}
.ticket-row:hover { background: color-mix(in oklab, var(--red) 4%, transparent); }
.ticket-row:hover::before { width: 3px; }
.ticket-row .num   { color: var(--rule); transition: color var(--dur-base) var(--ease-out); }
.ticket-row:hover .num { color: var(--red); }
.ticket-row .arr   { transition: transform var(--dur-base) var(--ease-out); }
.ticket-row:hover .arr { transform: translateX(4px); }
```

> The codebase already has `transition: width .25s ease` on the row bar — replace
> the raw `.25s ease` with `var(--dur-base) var(--ease-out)` for consistency.

---

## 6. Ambient — status ping + skeleton shimmer

### 6a. Open-ticket ping (replaces the current opacity blink)

`StatusPill.tsx` currently blinks opacity (`lg-blink`). For the **open** status,
swap to an expanding ring — calmer, more "live". Keep the other statuses static.

```css
.pill--open .dot { position: relative; }
.pill--open .dot::before {
  content: ''; position: absolute; inset: -3px; border-radius: 50%;
  border: 1.5px solid var(--red);
  animation: ping 1.8s var(--ease-out) infinite;
}
@keyframes ping {
  0%   { transform: scale(0.6); opacity: 1; }
  100% { transform: scale(2.6); opacity: 0; }
}
```

### 6b. Skeleton shimmer — loading state for React Query `isLoading`

```css
.skel {
  background: linear-gradient(90deg, var(--rule) 0%,
    color-mix(in oklab, var(--rule) 50%, var(--cream)) 50%, var(--rule) 100%);
  background-size: 240% 100%;
  animation: shimmer 1.6s linear infinite;
}
@keyframes shimmer { 0% { background-position: 100% 0; } 100% { background-position: -100% 0; } }
```
Render skeleton `.skel` blocks (rows on the list, fields on detail) while
`query.isLoading`; swap to content when data arrives.

---

## 7. Overlays — modal, dropdown, toast

### 7a. Modal (create-ticket, new-client, magic-link, archive-confirm)

Backdrop blurs + fades; sheet rises 40px and scales up with the spring. Mount/unmount
needs a brief keep-alive so the exit animation can play — use a small `useMountTransition`
hook (or `framer-motion`'s `AnimatePresence` IF you decide to add it; otherwise CSS + timeout).

```css
.overlay {
  position: fixed; inset: 0; z-index: 100; display: grid; place-items: center;
  background: rgba(13,13,13,0.55); backdrop-filter: blur(2px);
  opacity: 0; pointer-events: none;
  transition: opacity var(--dur-base) var(--ease-out);
}
.overlay.is-open { opacity: 1; pointer-events: auto; }
.modal {
  transform: translateY(40px) scale(0.96); opacity: 0;
  transition: transform var(--dur-base) var(--ease-spring), opacity var(--dur-base) var(--ease-out);
}
.overlay.is-open .modal { transform: none; opacity: 1; }
```
```tsx
// Minimal keep-alive so exit plays. No new deps.
function useMounted(open: boolean, ms = 360) {
  const [mounted, setMounted] = useState(open);
  useEffect(() => {
    if (open) { setMounted(true); return; }
    const t = setTimeout(() => setMounted(false), ms);
    return () => clearTimeout(t);
  }, [open, ms]);
  return mounted;
}
// render: {mounted && <div className={`overlay ${open ? 'is-open' : ''}`}>…</div>}
// set `open` true on the NEXT frame after mount (requestAnimationFrame) so the
// transition has a 0→1 to animate.
```
Dismiss on backdrop click + Esc. Lock body scroll while open.

### 7b. Dropdown (row action menus on the clients roster)

Scales open from the top-left origin.

```css
.dropdown {
  position: absolute; top: calc(100% + 6px); left: 0; transform-origin: top left;
  opacity: 0; transform: translateY(-6px) scale(0.98); pointer-events: none;
  transition: opacity var(--dur-base) var(--ease-out), transform var(--dur-base) var(--ease-out);
}
.dropdown.is-open { opacity: 1; transform: none; pointer-events: auto; }
```
Close on outside-click (document listener) and Esc.

### 7c. Toast (ticket created, report generated, sessions revoked)

Springs up from bottom-center, auto-dismisses ~2.6s, exits with a fade-down.
Build a `ToastContext` + `useToast()` that pushes `{id, text}` and auto-removes.

```css
.toast {
  transform: translateY(40px); opacity: 0;
  animation: toast-in var(--dur-base) var(--ease-spring) forwards;
}
.toast.is-leaving { animation: toast-out var(--dur-base) var(--ease-out) forwards; }
@keyframes toast-in  { to { transform: none; opacity: 1; } }
@keyframes toast-out { to { transform: translateY(40px); opacity: 0; } }
```

---

## 8. Magic-link QR — staggered assemble

On `MagicLandingPage` / the magic-link modal, the QR modules pop in on a
3ms-per-cell stagger so the code "draws itself" over ~0.5s. The QR is rendered as
SVG `<rect>` cells; set `animation-delay` per cell by index.

```css
.qr-cell { animation: qr-pop 320ms var(--ease-spring) both; transform-origin: center; transform-box: fill-box; }
@keyframes qr-pop { from { opacity: 0; transform: scale(0); } to { opacity: 1; transform: scale(1); } }
```
```tsx
cells.map((c, i) => <rect key={i} {...c} className="qr-cell" style={{ animationDelay: `${i * 3}ms` }} />)
```
Finder squares (the 3 corners) should NOT animate — render them after, without
the class, so the code stays recognisable as it assembles.

---

## 9. Scroll reveal — IntersectionObserver

Elements lift 28px + fade in as they enter the viewport, staggered for cascades.
Best as a tiny reusable hook so any page section can opt in. Use on the **Reports**
recent-list and any future marketing/empty states (the working desk views should
NOT animate on every scroll — reserve this for first-paint of a section).

```css
.reveal { opacity: 0; transform: translateY(28px);
  transition: opacity 720ms var(--ease-out), transform 720ms var(--ease-out); }
.reveal.is-in { opacity: 1; transform: none; }
.reveal.d2 { transition-delay: 80ms; } .reveal.d3 { transition-delay: 160ms; } .reveal.d4 { transition-delay: 240ms; }
```
```tsx
function useReveal<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  useEffect(() => {
    const el = ref.current; if (!el) return;
    const io = new IntersectionObserver(
      ([e]) => el.classList.toggle('is-in', e.isIntersecting), { threshold: 0.2 });
    io.observe(el); return () => io.disconnect();
  }, []);
  return ref;
}
```

---

## 10. Page transitions — route slide (react-router)

Between desk sections (Tickets / Clients / Reports / Settings) the outgoing view
slides left + fades while the incoming eases in from the right; reverse direction
for back-nav so motion encodes direction.

react-router v6 has no built-in transition. Two acceptable routes:
1. **Lightweight, no deps:** wrap `<Outlet/>` in a component keyed on
   `useLocation().pathname`; track previous pathname to pick direction; apply the
   `.page--entering/.exiting/.entered` classes with a mount keep-alive like §7a.
2. **If you're already adding `framer-motion`:** `AnimatePresence mode="wait"` with
   an `x: 40 → 0 → -40` variants set. Don't add the dep solely for this, though.

```css
.page { transition: transform var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out); }
.page--entering { transform: translateX(40px);  opacity: 0; }   /* forward nav */
.page--back     { transform: translateX(-40px); opacity: 0; }   /* back nav */
.page--exiting  { transform: translateX(-40px); opacity: 0; }
.page--entered  { transform: none; opacity: 1; }
```

> Keep durations at `--dur-base` (320ms). Anything slower makes navigation feel
> laggy on a working tool. The theme reveal is the only thing that earns `--dur-slow`.

---

## 11. Accordion — fluid height (settings FAQ / help)

Animate height with the `grid-template-rows: 0fr → 1fr` trick — genuinely fluid
0→auto with no measured pixel heights. Plus rotates 45° into a cross and reddens.

```css
.acc__body { display: grid; grid-template-rows: 0fr; transition: grid-template-rows var(--dur-base) var(--ease-out); }
.acc.is-open .acc__body { grid-template-rows: 1fr; }
.acc__inner { overflow: hidden; }                     /* REQUIRED for the trick */
.acc__icon { transition: transform var(--dur-base) var(--ease-out); }
.acc.is-open .acc__icon { transform: rotate(45deg); color: var(--red); }
```

---

## Implementation order (suggested)

1. **§0 + §1** — tokens/reduced-motion + theme cross-fade fallback. Foundation; everything leans on it.
2. **§2 buttons + §4 inputs + §5 rows** — the highest-traffic surfaces, pure CSS, no React plumbing.
3. **§3 Segmented/Tabs** — new shared component; refactor the create-ticket priority + list filters to use it.
4. **§6 ambient** — swap StatusPill blink → ping; add skeletons to the Query loading states.
5. **§7 overlays** — modal keep-alive hook first, then dropdown + toast context.
6. **§8 QR, §9 reveal, §11 accordion** — self-contained set-pieces.
7. **§10 route transitions** — last; touches the router shell, easiest to regress.

## Acceptance checklist
- [ ] No `transition: all` anywhere. Property lists are explicit.
- [ ] No hard-coded easing/duration — all reference `--ease-*` / `--dur-*` tokens.
- [ ] `prefers-reduced-motion` zeroes every animation (test in DevTools rendering tab).
- [ ] Sliding pip/underline recomputes after `document.fonts.ready`.
- [ ] Modals/toasts play their **exit** animation (keep-alive works, not just enter).
- [ ] Theme flip stays smooth on a non-View-Transitions browser (Firefox) via the token cross-fade.
- [ ] Route slide reverses direction on browser back.
