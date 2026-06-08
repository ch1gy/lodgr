import { api, tokenStore } from './client';
import type { AccessTokenResponse, MeResponse, SessionResponse } from './types';

export const auth = {
  async login(email: string, password: string): Promise<string> {
    const r = await api.post<AccessTokenResponse>('/auth/login', { email, password });
    tokenStore.set(r.data.access_token);
    return r.data.access_token;
  },

  async logout(): Promise<void> {
    try { await api.post('/auth/logout'); } catch { /* ignore */ }
    tokenStore.set(null);
  },

  /** Exchange a magic-link token for a (short-lived, stateless) access token. */
  async magic(token: string): Promise<string> {
    const r = await api.post<AccessTokenResponse>('/auth/magic', { token });
    tokenStore.set(r.data.access_token);
    return r.data.access_token;
  },

  /** Fetch the authenticated user's own profile (name, email, role). */
  async me(): Promise<MeResponse> {
    const r = await api.get<MeResponse>('/auth/me');
    return r.data;
  },

  async changePassword(current_password: string, new_password: string): Promise<string> {
    const r = await api.patch<AccessTokenResponse>('/auth/password', {
      current_password,
      new_password,
    });
    tokenStore.set(r.data.access_token);
    return r.data.access_token;
  },

  async listSessions(): Promise<SessionResponse[]> {
    const r = await api.get<SessionResponse[]>('/auth/sessions');
    return r.data;
  },

  async revokeSession(sessionId: string): Promise<void> {
    await api.delete(`/auth/sessions/${sessionId}`);
  },
};
