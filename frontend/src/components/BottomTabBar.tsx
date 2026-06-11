// ─────────────────────────────────────────────────────────────────────────────
// BottomTabBar.tsx — Mobile-only bottom navigation tab bar (≤ 540px).
//
// 4 tabs per design.md §10.6: Queue / Reports / Clients / Settings.
// Invoices is intentionally absent — desk-only secondary screen, reached
// from the masthead or links, not the primary mobile nav.
// ─────────────────────────────────────────────────────────────────────────────

import { Link } from 'react-router-dom';
import { useAuth } from '../auth/AuthContext';

interface Props {
  active: 'tickets' | 'clients' | 'reports' | 'settings';
}

export function BottomTabBar({ active }: Props) {
  const { isDesk } = useAuth();

  return (
    <nav className="lg-tabbar" aria-label="Main navigation">
      <Link to="/tickets" className={`lg-tabbar__t${active === 'tickets' ? ' active' : ''}`}>
        <span className="lg-tabbar__ic">≡</span>
        <span>Queue</span>
      </Link>
      {isDesk && (
        <Link to="/reports" className={`lg-tabbar__t${active === 'reports' ? ' active' : ''}`}>
          <span className="lg-tabbar__ic">↗</span>
          <span>Reports</span>
        </Link>
      )}
      {isDesk && (
        <Link to="/clients" className={`lg-tabbar__t${active === 'clients' ? ' active' : ''}`}>
          <span className="lg-tabbar__ic">◎</span>
          <span>Clients</span>
        </Link>
      )}
      <Link to="/settings" className={`lg-tabbar__t${active === 'settings' ? ' active' : ''}`}>
        <span className="lg-tabbar__ic">⚙</span>
        <span>Settings</span>
      </Link>
    </nav>
  );
}
