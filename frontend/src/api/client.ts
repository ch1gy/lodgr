// Lodgr API client — axios + auto-refresh interceptor.
//
// The backend uses TWO tokens:
//   • Access token: short-lived JWT (15 min), sent as Authorization: Bearer.
//   • Refresh token: 7-day httpOnly cookie, sent automatically by the browser
//     because we're same-origin via the Vite proxy in dev (and ServeDir in prod).
//
// Per FRONTEND_HANDOFF.md we keep the access token in MEMORY ONLY (never
// localStorage — XSS risk). On any 401, a single refresh lock prevents
// thundering-herd refresh calls, and the original request is retried once.

import axios, { AxiosError, AxiosRequestConfig } from 'axios';

// ── Token store (module-scoped, in memory) ─────────────────────────────────
let accessToken: string | null = null;
const subscribers = new Set<(t: string | null) => void>();

export const tokenStore = {
  get(): string | null { return accessToken; },
  set(t: string | null) {
    accessToken = t;
    subscribers.forEach((fn) => fn(t));
  },
  subscribe(fn: (t: string | null) => void): () => void {
    subscribers.add(fn);
    return () => subscribers.delete(fn);
  },
};

// ── Axios instance ─────────────────────────────────────────────────────────
export const api = axios.create({
  // Use same-origin paths so the Vite proxy / ServeDir picks them up.
  baseURL: '',
  withCredentials: true, // ensure refresh cookie is sent
});

api.interceptors.request.use((config) => {
  const t = tokenStore.get();
  if (t) {
    config.headers = config.headers ?? {};
    config.headers['Authorization'] = `Bearer ${t}`;
  }
  return config;
});

// ── Refresh lock — one in-flight refresh at a time ─────────────────────────
let refreshing: Promise<string | null> | null = null;

async function doRefresh(): Promise<string | null> {
  try {
    const r = await axios.post<{ access_token: string }>(
      '/auth/refresh',
      undefined,
      { withCredentials: true }
    );
    tokenStore.set(r.data.access_token);
    return r.data.access_token;
  } catch {
    tokenStore.set(null);
    return null;
  }
}

api.interceptors.response.use(
  (r) => r,
  async (error: AxiosError) => {
    const original = error.config as AxiosRequestConfig & { _retry?: boolean; _skipRefresh?: boolean };
    if (!original) return Promise.reject(error);

    // Don't recurse on the refresh endpoint itself, or on opt-out requests.
    if (original.url?.includes('/auth/refresh') || original._skipRefresh) {
      return Promise.reject(error);
    }

    if (error.response?.status === 401 && !original._retry) {
      original._retry = true;
      if (!refreshing) {
        refreshing = doRefresh().finally(() => { refreshing = null; });
      }
      const newToken = await refreshing;
      if (newToken) {
        original.headers = original.headers ?? {};
        (original.headers as Record<string, string>)['Authorization'] = `Bearer ${newToken}`;
        return api(original);
      }
      // Refresh failed — session is dead, send the user to login.
      window.location.href = '/login';
    }

    return Promise.reject(error);
  }
);
