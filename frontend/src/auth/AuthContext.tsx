// ─────────────────────────────────────────────────────────────────────────────
// AuthContext.tsx — global auth state for the Lodgr frontend.
//
// What it owns:
//   • The current access token (mirrored from the in-memory tokenStore so the
//     app re-renders when the token changes — e.g. after refresh).
//   • The decoded JWT payload, so any component can read role / session_type
//     and conditionally render desk-only chrome (transitions, internal notes,
//     admin actions, etc).
//   • login / logout / setMagicToken helpers used by LoginPage and MagicLandingPage.
//
// What it does NOT own:
//   • The refresh token. That's an httpOnly cookie the browser handles for us;
//     we just call POST /auth/refresh and the cookie is sent automatically.
//   • Persistence. Tokens live in memory only (XSS hygiene). On page reload we
//     try a silent refresh — the refresh cookie survives reload, the access
//     token does not.
//
// Usage pattern:
//   const { user, isDesk, login, logout } = useAuth();
//   if (isDesk) { /* show transition buttons */ }
// ─────────────────────────────────────────────────────────────────────────────

import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  useCallback,
  ReactNode,
} from 'react';
import { jwtDecode } from 'jwt-decode';
import { tokenStore } from '../api/client';
import { auth } from '../api/auth';
import type { JwtPayload } from '../api/types';

interface AuthState {
  /** Raw access token, or null when signed out. */
  token: string | null;
  /** Decoded JWT payload, or null. Do NOT trust client-side; server enforces. */
  user: JwtPayload | null;
  /** True if signed in as the desk agent. */
  isDesk: boolean;
  /** True if this session is scoped to a single ticket (magic-link share). */
  isScoped: boolean;
  /** True until the initial silent-refresh attempt finishes. */
  loading: boolean;
  /** Sign in with email/password. Throws on failure (caller catches). */
  login(email: string, password: string): Promise<void>;
  /** Exchange a magic-link `?token=…` for an access token. Throws on failure. */
  redeemMagicLink(magicToken: string): Promise<string>;
  /** Clear server-side refresh cookie + local state. */
  logout(): Promise<void>;
}

const AuthContext = createContext<AuthState | null>(null);

// Safely decode a JWT and return null on any failure. Never throws — a bad
// token is treated as "not signed in" rather than crashing. Exported so
// pages that need to peek at a fresh token (e.g. MagicLandingPage) can
// reuse the same null-safe wrapper without re-implementing it.
export function safeDecode(token: string | null): JwtPayload | null {
  if (!token) return null;
  try {
    return jwtDecode<JwtPayload>(token);
  } catch {
    return null;
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  // Mirror the in-memory token from api/client.ts. We subscribe so that
  // background refreshes (driven by the response interceptor) propagate here.
  const [token, setTokenState] = useState<string | null>(() => tokenStore.get());
  const [loading, setLoading] = useState<boolean>(true);

  // Subscribe to tokenStore changes so any refresh (incl. interceptor-driven)
  // updates the context and re-renders consumers.
  useEffect(() => {
    const unsub = tokenStore.subscribe((t) => setTokenState(t));
    return () => { unsub(); };
  }, []);

  // ── Silent refresh on first mount ─────────────────────────────────────────
  // If a refresh cookie is still valid we can recover the session without
  // showing a login screen. If it fails we just stay signed out.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        // POST /auth/refresh — the interceptor doesn't help here because
        // there's no current request to retry, so we call the endpoint
        // through `auth.ts` indirectly… actually we use axios directly via
        // the typed api wrapper. We rely on the cookie being present.
        // We hit the endpoint via plain fetch to avoid auth header logic.
        const r = await fetch('/auth/refresh', {
          method: 'POST',
          credentials: 'include',
        });
        if (!cancelled && r.ok) {
          const j = (await r.json()) as { access_token: string };
          tokenStore.set(j.access_token);
        }
      } catch {
        /* no refresh cookie / network — stay signed out */
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    await auth.login(email, password); // updates tokenStore on success
  }, []);

  const redeemMagicLink = useCallback((magicToken: string): Promise<string> => {
    return auth.magic(magicToken); // updates tokenStore on success, returns the token
  }, []);

  const logout = useCallback(async () => {
    await auth.logout(); // clears refresh cookie + tokenStore
  }, []);

  // Decode the token once; memoize the full context value so that consumers
  // only re-render when something they actually care about (token, loading,
  // etc.) changes — not on every AuthProvider render.
  const user = useMemo(() => safeDecode(token), [token]);

  const value = useMemo<AuthState>(
    () => ({
      token,
      user,
      isDesk: user?.role === 'desk',
      isScoped: user?.session_type === 'scoped',
      loading,
      login,
      redeemMagicLink,
      logout,
    }),
    [token, user, loading, login, redeemMagicLink, logout]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

/** Hook for accessing auth state. Throws if used outside <AuthProvider>. */
export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within an <AuthProvider>');
  return ctx;
}
