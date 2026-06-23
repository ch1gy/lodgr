// ─────────────────────────────────────────────────────────────────────────────
// AuthContext.tsx — global auth state for the Lodgr frontend.
//
// What it owns:
//   • The current access token (mirrored from the in-memory tokenStore).
//   • The decoded JWT payload (role, session_type, ticket_scope, exp).
//   • The profile fetched from GET /auth/me (name, email) — the JWT itself
//     does not carry email, so this is the authoritative source for display.
//   • login / logout / redeemMagicLink helpers.
//
// Profile fetch:
//   Any time the access token changes from null → value, GET /auth/me is called
//   and the result cached in state. This covers login, silent refresh, and
//   magic link exchange. The fetch is non-fatal — if it fails, profile is null
//   and components fall back to '—'.
// ─────────────────────────────────────────────────────────────────────────────

import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  useCallback,
  useRef,
  ReactNode,
} from 'react';
import { jwtDecode } from 'jwt-decode';
import { tokenStore } from '../api/client';
import { auth } from '../api/auth';
import type { JwtPayload, MeResponse } from '../api/types';

interface AuthState {
  /** Raw access token, or null when signed out. */
  token: string | null;
  /** Decoded JWT payload, or null. Do NOT trust client-side; server enforces. */
  user: JwtPayload | null;
  /** Server-side profile: name, email. Null until GET /auth/me resolves. */
  profile: Pick<MeResponse, 'id' | 'name' | 'email'> | null;
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
  /** Re-fetch GET /auth/me — call after the user edits their own profile. */
  refreshProfile(): Promise<void>;
}

const AuthContext = createContext<AuthState | null>(null);

export function safeDecode(token: string | null): JwtPayload | null {
  if (!token) return null;
  try {
    return jwtDecode<JwtPayload>(token);
  } catch {
    return null;
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [token, setTokenState] = useState<string | null>(() => tokenStore.get());
  const [loading, setLoading] = useState<boolean>(true);
  const [profile, setProfile] = useState<Pick<MeResponse, 'id' | 'name' | 'email'> | null>(null);

  // Subscribe to tokenStore so background refreshes propagate here.
  useEffect(() => {
    const unsub = tokenStore.subscribe((t) => setTokenState(t));
    return () => { unsub(); };
  }, []);

  // Fetch /auth/me whenever the token changes to a non-null value.
  // Non-fatal: if the request fails, profile stays null and UI shows '—'.
  useEffect(() => {
    if (!token) {
      setProfile(null);
      return;
    }
    let cancelled = false;
    auth.me()
      .then((data) => {
        if (!cancelled) setProfile({ id: data.id, name: data.name, email: data.email });
      })
      .catch(() => {
        // Token may be expired or /auth/me unavailable — non-fatal.
      });
    return () => { cancelled = true; };
  }, [token]);

  // ── Silent refresh on first mount ─────────────────────────────────────────
  // Refresh tokens rotate on every call — the old one is revoked the instant
  // a new one is issued. StrictMode double-invokes this effect in dev, which
  // without a guard fires two concurrent /auth/refresh requests; the second
  // can race ahead of the browser applying the first response's Set-Cookie
  // and replay the now-revoked old cookie, which the backend treats as token
  // theft and wipes every session. The ref ensures only one request per app
  // load actually fires.
  // The ref (not a per-call `cancelled` flag) is what guards against the
  // duplicate request — StrictMode's synthetic unmount still runs this
  // effect's cleanup even though the fetch we kept is still in flight, so
  // gating on a closure-local `cancelled` here would discard its own result.
  const refreshStarted = useRef(false);
  useEffect(() => {
    if (refreshStarted.current) return;
    refreshStarted.current = true;
    (async () => {
      try {
        const r = await fetch('/auth/refresh', {
          method: 'POST',
          credentials: 'include',
        });
        if (r.ok) {
          const j = (await r.json()) as { access_token: string };
          tokenStore.set(j.access_token);
        }
      } catch {
        /* no refresh cookie / network — stay signed out */
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    await auth.login(email, password);
  }, []);

  const redeemMagicLink = useCallback((magicToken: string): Promise<string> => {
    return auth.magic(magicToken);
  }, []);

  const logout = useCallback(async () => {
    await auth.logout();
    setProfile(null);
  }, []);

  const refreshProfile = useCallback(async () => {
    const data = await auth.me();
    setProfile({ id: data.id, name: data.name, email: data.email });
  }, []);

  const user = useMemo(() => safeDecode(token), [token]);

  const value = useMemo<AuthState>(
    () => ({
      token,
      user,
      profile,
      isDesk: user?.role === 'desk',
      isScoped: user?.session_type === 'scoped',
      loading,
      login,
      redeemMagicLink,
      refreshProfile,
      logout,
    }),
    [token, user, profile, loading, login, redeemMagicLink, refreshProfile, logout]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within an <AuthProvider>');
  return ctx;
}
