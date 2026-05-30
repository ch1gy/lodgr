// Lodgr API — TypeScript types, copied from FRONTEND_HANDOFF.md

export type TicketStatus = 'open' | 'pending' | 'acknowledged' | 'closed';
export type TicketPriority = 'low' | 'medium' | 'high' | 'urgent';
export type TicketType = 'standard' | 'maintenance' | 'security_log';
export type SessionType = 'full' | 'scoped';

// ── Auth ────────────────────────────────────────────────────────────────────
export interface AccessTokenResponse {
  access_token: string;
}

export interface JwtPayload {
  sub: string;
  email?: string;
  role?: 'desk' | 'client';
  session_type: SessionType;
  ticket_scope: string | null;
  exp: number;
  iat: number;
}

// ── Tickets ─────────────────────────────────────────────────────────────────
export interface TicketResponse {
  id: string;
  title: string;
  description: string;
  status: TicketStatus;
  created_by: string;
  client_id: string;
  created_at: string;
  priority: TicketPriority;
  category: string | null;
  due_date: string | null;
  estimated_completion: string | null;
  ticket_type: TicketType;
  recurring: boolean;
  recurring_interval_days: number | null;
}

export interface PaginatedTickets {
  tickets: TicketResponse[];
  total: number;
  page: number;
  limit: number;
}

export interface ThreadEntry {
  id: string;
  ticket_id: string;
  sender_id: string;
  body: string;
  attachment_path: string | null;
  created_at: string;
}

export type TicketWithThread = TicketResponse & { thread: ThreadEntry[] };

export interface InternalNote {
  id: string;
  ticket_id: string;
  author_id: string;
  body: string;
  created_at: string;
}

export interface CreateTicketPayload {
  title: string;
  description: string;
  priority?: TicketPriority;
  ticket_type?: TicketType;
  category?: string;
  due_date?: string;
  recurring?: boolean;
  recurring_interval_days?: number;
  /** Desk only: file on behalf of a specific client UUID. */
  client_id?: string;
}

export interface PatchTicketPayload {
  priority?: TicketPriority;
  category?: string;
  due_date?: string;
  ticket_type?: TicketType;
  recurring?: boolean;
  recurring_interval_days?: number;
}

// ── Admin ───────────────────────────────────────────────────────────────────
export interface Client {
  id: string;
  name: string;
  email: string;
  deleted_at: string | null;
  /** Consecutive failed login attempts. 0 when not locked. */
  failed_attempts: number;
  /** RFC-3339 locked-until. null = not locked. "9999-…" = permanent. */
  locked_until: string | null;
}

export interface CreateClientPayload {
  name: string;
  email: string;
  password: string;
}

export interface ExportResponse {
  export_id: string;
  download_url: string;
}

export interface MagicLinkResponse {
  url: string;
}

// ── Me (GET /auth/me) ────────────────────────────────────────────────────────
export interface MeResponse {
  id: string;
  name: string;
  email: string;
  role: string;
  created_at: string;
}

// ── Error shape ─────────────────────────────────────────────────────────────
export interface ApiError {
  error: string;
}
