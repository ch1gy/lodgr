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
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
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
    await auth.login(email, password);
  }, []);

  const redeemMagicLink = useCallback((magicToken: string): Promise<string> => {
    return auth.magic(magicToken);
  }, []);

  const logout = useCallback(async () => {
    await auth.logout();
    setProfile(null);
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
      logout,
    }),
    [token, user, profile, loading, login, redeemMagicLink, logout]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within an <AuthProvider>');
  return ctx;
}
