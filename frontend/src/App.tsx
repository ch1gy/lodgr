// ─────────────────────────────────────────────────────────────────────────────
// App.tsx — provider stack + route table.
//
// Provider order (outermost → innermost):
//   • <BrowserRouter>          — so child providers can use useNavigate hooks.
//   • <QueryClientProvider>    — TanStack Query for all server state.
//   • <ThemeProvider>          — light/dark mode + View-Transitions reveal.
//   • <AuthProvider>           — access-token state + silent refresh on boot.
//
// Routes:
//   /login                — public sign-in.
//   /magic                — one-shot token exchange landing (kept off /auth/*
//                           so the dev proxy doesn't forward this GET to the
//                           backend's POST-only /auth/magic exchange route).
//   /tickets              — list (desk + client, both shapes).
//   /tickets/:id          — detail + thread + composer.
//   /clients              — admin client roster (desk only).
//   /reports              — monthly PDF reports (desk only).
//   /settings             — password change + account (both roles).
//   *                     — fallback redirect.
// ─────────────────────────────────────────────────────────────────────────────

import { BrowserRouter, Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { useEffect, useRef, useState } from 'react';
import { ErrorBoundary } from './components/ErrorBoundary';
import { ToastProvider } from './components/Toast';
import './styles/buttons.css';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider } from './auth/AuthContext';
import { ThemeProvider } from './theme/ThemeContext';
import { MorphProvider } from './theme/MorphContext';
import { CommandPaletteProvider } from './components/CommandPalette';
import { ProtectedRoute } from './components/ProtectedRoute';
import { LoginPage } from './pages/LoginPage';
import { MagicLandingPage } from './pages/MagicLandingPage';
import { TicketListPage } from './pages/TicketListPage';
import { TicketDetailPage } from './pages/TicketDetailPage';
import { ClientsPage } from './pages/ClientsPage';
import { InvoicesPage } from './pages/InvoicesPage';
import { ReportsPage } from './pages/ReportsPage';
import { SettingsPage } from './pages/SettingsPage';

// Single QueryClient instance for the app lifetime.
//
// Defaults chosen for a low-frequency support desk:
//   • staleTime: 10s — quick re-fetch on remount, avoid hammering the API
//                      when you bounce between list/detail.
//   • retry: 1     — don't spin on a flaky network forever.
//   • refetchOnWindowFocus: false — desk users tab away constantly; let
//                      page-level polling drive freshness instead.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 10_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

// Cursor lighting — soft specular highlight follows the cursor (§6.12)
function CursorLight() {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    function onMove(e: MouseEvent) {
      el!.style.setProperty('--mx', `${e.clientX}px`);
      el!.style.setProperty('--my', `${e.clientY}px`);
    }
    document.addEventListener('mousemove', onMove, { passive: true });
    return () => document.removeEventListener('mousemove', onMove);
  }, []);
  return <div ref={ref} className="sig-light" aria-hidden />;
}

// Page slide transition — wraps every routed page.
// Slides right→left on forward nav, left→right on back.
function PageTransition({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  const prevPath = useRef<string>(location.pathname);
  const [cls, setCls] = useState('lg-page lg-page--entered');

  useEffect(() => {
    if (prevPath.current === location.pathname) return;
    const entering = 'lg-page lg-page--entering';
    setCls(entering);
    const raf = requestAnimationFrame(() => {
      setCls('lg-page lg-page--entered');
    });
    prevPath.current = location.pathname;
    return () => cancelAnimationFrame(raf);
  }, [location.pathname]);

  return <div className={cls}>{children}</div>;
}

export function App() {
  return (
    <ErrorBoundary>
    <CursorLight />
    <BrowserRouter>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <AuthProvider>
            <MorphProvider>
            <CommandPaletteProvider>
            <ToastProvider>
            <Routes>
              {/* Public */}
              <Route path="/login" element={<LoginPage />} />
              <Route path="/magic" element={<MagicLandingPage />} />

              {/* Authenticated */}
              <Route
                path="/tickets"
                element={
                  <ProtectedRoute>
                    <PageTransition><TicketListPage /></PageTransition>
                  </ProtectedRoute>
                }
              />
              <Route
                path="/tickets/:id"
                element={
                  <ProtectedRoute>
                    <PageTransition><TicketDetailPage /></PageTransition>
                  </ProtectedRoute>
                }
              />

              <Route
                path="/clients"
                element={
                  <ProtectedRoute deskOnly>
                    <PageTransition><ClientsPage /></PageTransition>
                  </ProtectedRoute>
                }
              />
              <Route
                path="/invoices"
                element={
                  <ProtectedRoute>
                    <PageTransition><InvoicesPage /></PageTransition>
                  </ProtectedRoute>
                }
              />
              <Route
                path="/reports"
                element={
                  <ProtectedRoute deskOnly>
                    <PageTransition><ReportsPage /></PageTransition>
                  </ProtectedRoute>
                }
              />
              <Route
                path="/settings"
                element={
                  <ProtectedRoute>
                    <PageTransition><SettingsPage /></PageTransition>
                  </ProtectedRoute>
                }
              />

              {/* Root + 404 — bounce to the queue. ProtectedRoute will send
                  unsigned users to /login. */}
              <Route path="/" element={<Navigate to="/tickets" replace />} />
              <Route path="*" element={<Navigate to="/tickets" replace />} />
            </Routes>
            </ToastProvider>
            </CommandPaletteProvider>
            </MorphProvider>
          </AuthProvider>
        </ThemeProvider>
      </QueryClientProvider>
    </BrowserRouter>
    </ErrorBoundary>
  );
}
