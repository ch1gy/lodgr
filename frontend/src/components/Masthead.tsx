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

import { Link } from 'react-router-dom';
import { useAuth } from '../auth/AuthContext';
import { useTheme } from '../theme/ThemeContext';

interface Props {
  /** Which link should render as active. Use this everywhere a desk page
   *  mounts the masthead so the nav reflects the route. */
  active?: 'tickets' | 'clients' | 'reports' | 'settings';
}

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
  const { user, profile, isDesk, logout } = useAuth();
  const { theme, toggle } = useTheme();

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

  return (
    <header className="lg-mast">
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
    </header>
  );
}
