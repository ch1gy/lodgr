// ─────────────────────────────────────────────────────────────────────────────
// Masthead.tsx — top editorial bar used on every signed-in page.
//
// Three slots:
//   • LEFT  : "Issue No. NNN · DD MMM" + role line ("Desk · Name" or
//             "Client · Name"). Hidden on mobile to save space.
//   • CENTER: the "Lodgr." wordmark (italic serif + red period).
//   • RIGHT : nav links (Tickets / Clients / Reports / Settings) + avatar.
//             For clients we hide the desk-only links (Clients, Reports).
//
// All styles live in tokens.css under .lg-mast*.
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useState, type CSSProperties } from 'react';
import { Link } from 'react-router-dom';
import { useAuth } from '../auth/AuthContext';
import { useTheme } from '../theme/ThemeContext';

interface Props {
  active?: 'tickets' | 'clients' | 'invoices' | 'reports' | 'settings';
}

type NavKey = Props['active'];
const NAV: { key: NavKey; to: string; label: string; deskOnly: boolean }[] = [
  { key: 'tickets',  to: '/tickets',  label: 'Tickets',  deskOnly: false },
  { key: 'clients',  to: '/clients',  label: 'Clients',  deskOnly: true },
  { key: 'invoices', to: '/invoices', label: 'Invoices', deskOnly: true },
  { key: 'reports',  to: '/reports',  label: 'Reports',  deskOnly: true },
  { key: 'settings', to: '/settings', label: 'Settings', deskOnly: false },
];

/** Format the masthead's issue label deterministically.
 *  We treat the issue number as DDD-of-the-year so it changes once a day. */
function issueLabel(now = new Date()): string {
  const start = new Date(now.getFullYear(), 0, 0);
  const diff = +now - +start;
  const doy = Math.floor(diff / 86_400_000);
  const day = now.toLocaleDateString('en-GB', { day: '2-digit', month: 'short' });
  return `Issue No. ${String(doy).padStart(3, '0')} · ${day}`;
}

export function Masthead({ active = 'tickets' }: Props) {
  const { profile, isDesk, logout } = useAuth();
  const { theme, toggle } = useTheme();
  const [menuOpen, setMenuOpen] = useState(false);

  // Lock body scroll + close on Escape while the drawer is open.
  useEffect(() => {
    if (!menuOpen) return;
    const prev = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setMenuOpen(false); };
    window.addEventListener('keydown', onKey);
    return () => { document.body.style.overflow = prev; window.removeEventListener('keydown', onKey); };
  }, [menuOpen]);

  // Use server-side profile for email — JWT does not carry the email field.
  const email = profile?.email ?? '';
  const displayName = profile?.name ?? email;
  const initials =
    (profile?.name ?? email)
      .split(/[\s@.\-_+]/)
      .map((s) => s[0]?.toUpperCase())
      .filter(Boolean)
      .slice(0, 2)
      .join('') || '—';

  const visibleNav = NAV.filter((n) => !n.deskOnly || isDesk);

  return (
    <header className="lg-mast">
      {/* Hamburger — mobile only (CSS hides it ≥769px). Opens the drawer. */}
      <button
        type="button"
        className="lg-mast-burger"
        aria-label="Open menu"
        aria-expanded={menuOpen}
        onClick={() => setMenuOpen(true)}
      >
        <i></i><i></i><i></i>
      </button>

      <div className="lg-mast-left">
        <span className="lg-mast-issue">{issueLabel()}</span>
        <span className="lg-mast-issue">
          {isDesk ? 'Desk' : 'Client'} · <b>{displayName || 'signed in'}</b>
        </span>
      </div>

      <Link to="/tickets" className="lg-mast-logo" style={{ textDecoration: 'none' }}>
        <i>Lodgr</i>
        <span className="dot">.</span>
      </Link>

      <div className="lg-mast-right">
        <Link to="/tickets" className={'lg-mast-link' + (active === 'tickets' ? ' active' : '')}>
          Tickets
        </Link>
        {isDesk && (
          <>
            <Link to="/clients" className={'lg-mast-link' + (active === 'clients' ? ' active' : '')}>
              Clients
            </Link>
            <Link to="/invoices" className={'lg-mast-link' + (active === 'invoices' ? ' active' : '')}>
              Invoices
            </Link>
            <Link to="/reports" className={'lg-mast-link' + (active === 'reports' ? ' active' : '')}>
              Reports
            </Link>
          </>
        )}
        <Link to="/settings" className={'lg-mast-link' + (active === 'settings' ? ' active' : '')}>
          Settings
        </Link>

        {/* Light / dark toggle. We pass the click coordinates so the View
            Transitions reveal radiates from the toggle itself. */}
        <button
          type="button"
          className="lg-theme-switch"
          aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
          aria-pressed={theme === 'dark'}
          onClick={(e) => toggle({ x: e.clientX, y: e.clientY })}
        >
          <span className="seg light">LGT</span>
          <span className="slash" aria-hidden>/</span>
          <span className="seg dark">DRK</span>
        </button>

        <button
          type="button"
          className="lg-mast-link"
          onClick={() => { void logout(); }}
          style={{ background: 'none', border: 'none' }}
          title="Sign out"
        >
          Sign out
        </button>
        <div className="lg-mast-user" title={email || undefined}>
          <span className="av">{initials}</span>
          <span className="nm">{displayName || '—'}</span>
        </div>
      </div>

      {/* ── Mobile drawer (hamburger menu) ─────────────────────────────── */}
      <div className={'lg-drawer' + (menuOpen ? ' is-open' : '')} aria-hidden={!menuOpen}>
        <button className="lg-drawer__scrim" aria-label="Close menu" tabIndex={menuOpen ? 0 : -1} onClick={() => setMenuOpen(false)} />
        <nav className="lg-drawer__panel" aria-label="Main navigation">
          <div className="lg-drawer__top">
            <button className="lg-drawer__x" aria-label="Close menu" onClick={() => setMenuOpen(false)}>←</button>
          </div>

          <div className="lg-drawer__links">
            {visibleNav.map((n, i) => (
              <Link
                key={n.to}
                to={n.to}
                className={'lg-drawer__link' + (active === n.key ? ' active' : '')}
                style={{ '--di': i } as CSSProperties}
                onClick={() => setMenuOpen(false)}
              >
                <span className="n">{String(i + 1).padStart(2, '0')}</span>
                {n.label}
              </Link>
            ))}
          </div>

          <div className="lg-drawer__foot">
            <button
              type="button"
              className="lg-drawer__theme"
              aria-pressed={theme === 'dark'}
              onClick={(e) => toggle({ x: e.clientX, y: e.clientY })}
            >
              {theme === 'dark' ? 'Darkroom — on' : 'Darkroom — off'}
            </button>
            <button
              type="button"
              className="lg-drawer__signout"
              onClick={() => { setMenuOpen(false); void logout(); }}
            >
              Sign out
            </button>
            <div className="lg-drawer__who">
              <span>{email || (isDesk ? 'Desk' : 'Client')}</span>
              <span className="status">● online</span>
            </div>
          </div>
        </nav>
      </div>
    </header>
  );
}
