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
  sub_client_id: string | null;
  sub_client_name: string | null;
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
  /** Desk only: set the initial status at creation time (defaults to 'open'). */
  initial_status?: TicketStatus;
  /** Desk only: tag this ticket with a sub-client (must belong to the selected client). */
  sub_client_id?: string;
}

// ── Sub-clients ──────────────────────────────────────────────────────────────
export interface SubClient {
  id: string;
  client_id: string;
  name: string;
  created_at: string;
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
  failed_attempts: number;
  locked_until: string | null;
  address_line1: string | null;
  address_line2: string | null;
  pin_number: string | null;
  contact_person: string | null;
}

// ── Invoices ─────────────────────────────────────────────────────────────────
export type InvoiceStatus = 'draft' | 'sent' | 'paid';
export type RecurInterval = 'monthly' | 'quarterly' | 'yearly';

export interface InvoiceItem {
  name: string;
  sub?: string;
  qty: number;
  rate: number;
}

export interface InvoiceNote {
  k: string;
  v: string;
}

export interface InvoiceResponse {
  id: string;
  client_id: string;
  number: string;
  status: InvoiceStatus;
  currency: string;
  terms: string;
  issued_date: string;
  due_date: string;
  project_type: string;
  project_location: string;
  billed_to_name: string;
  billed_to_role: string;
  billed_to_addr1: string;
  billed_to_addr2: string;
  billed_to_pin: string;
  items: InvoiceItem[];
  notes: InvoiceNote[];
  editor_note: string;
  kra_number: string | null;
  recurring: boolean;
  recur_interval: RecurInterval | null;
  next_recur_date: string | null;
  created_at: string;
}

export interface CreateInvoicePayload {
  number?: string;
  client_id: string;
  currency?: string;
  terms?: string;
  issued_date: string;
  due_date: string;
  project_type?: string;
  project_location?: string;
  billed_to_name: string;
  billed_to_role?: string;
  billed_to_addr1?: string;
  billed_to_addr2?: string;
  billed_to_pin?: string;
  items: InvoiceItem[];
  notes?: InvoiceNote[];
  editor_note?: string;
  kra_number?: string;
  recurring?: boolean;
  recur_interval?: RecurInterval;
  next_recur_date?: string;
}

export type UpdateInvoicePayload = Partial<CreateInvoicePayload> & { status?: InvoiceStatus };

export interface CreateClientPayload {
  name: string;
  email: string;
  password: string;
  address_line1?: string;
  address_line2?: string;
  pin_number?: string;
  contact_person?: string;
}

export interface ExportResponse {
  export_id: string;
  download_url: string;
}

// ── Desk profile ────────────────────────────────────────────────────────────
export interface DeskProfile {
  name: string;
  tagline: string;
  email: string;
  website: string;
  city: string;
  phone: string;
  vat_number: string;
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
