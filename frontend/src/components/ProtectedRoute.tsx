// ─────────────────────────────────────────────────────────────────────────────
// ProtectedRoute.tsx — wraps any route that requires a signed-in session.
//
// While the initial silent-refresh is in flight we render a quiet splash so
// the user doesn't see the login screen flash on every reload. If after that
// settles there's still no token, we redirect to /login (preserving the
// originally-requested URL in state.from so we can bounce back after sign-in).
// ─────────────────────────────────────────────────────────────────────────────

import { ReactNode } from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { useAuth } from '../auth/AuthContext';

interface Props {
  children: ReactNode;
  /** Optional gate: only desk users may pass. Clients/scoped see the redirect. */
  deskOnly?: boolean;
}

export function ProtectedRoute({ children, deskOnly = false }: Props) {
  const { token, isDesk, loading } = useAuth();
  const location = useLocation();

  if (loading) {
    // Splash matches the cream background so there's no flash.
    return (
      <div
        style={{
          minHeight: '100vh',
          background: '#f2ede4',
          color: '#6a6560',
          display: 'grid',
          placeItems: 'center',
          fontFamily: '"DM Mono", monospace',
          fontSize: 11,
          letterSpacing: '.18em',
          textTransform: 'uppercase',
        }}
      >
        — Lodgr · loading the desk —
      </div>
    );
  }

  if (!token) {
    return <Navigate to="/login" replace state={{ from: location }} />;
  }

  if (deskOnly && !isDesk) {
    // Logged in but not desk — send to ticket list (clients can still use it).
    return <Navigate to="/tickets" replace />;
  }

  return <>{children}</>;
}
