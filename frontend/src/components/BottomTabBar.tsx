// ─────────────────────────────────────────────────────────────────────────────
// BottomTabBar.tsx — Mobile-only bottom navigation tab bar.
//
// Shown only on screens ≤ 540px via CSS (display:none on desktop).
// The design spec (Lodgr/screens/list-responsive.jsx) defines a 4-tab bar
// with small serif icons and mono labels: Tickets / Clients / Reports / Settings.
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
        <span className="lg-tabbar__ic">T</span>
        <span>Tickets</span>
      </Link>
      {isDesk && (
        <Link to="/clients" className={`lg-tabbar__t${active === 'clients' ? ' active' : ''}`}>
          <span className="lg-tabbar__ic">C</span>
          <span>Clients</span>
        </Link>
      )}
      {isDesk && (
        <Link to="/reports" className={`lg-tabbar__t${active === 'reports' ? ' active' : ''}`}>
          <span className="lg-tabbar__ic">R</span>
          <span>Reports</span>
        </Link>
      )}
      <Link to="/settings" className={`lg-tabbar__t${active === 'settings' ? ' active' : ''}`}>
        <span className="lg-tabbar__ic">⚙</span>
        <span>Settings</span>
      </Link>
    </nav>
  );
}
