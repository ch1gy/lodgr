import { api } from './client';
import type {
  PaginatedTickets,
  TicketWithThread,
  InternalNote,
  CreateTicketPayload,
  PatchTicketPayload,
  MagicLinkResponse,
} from './types';

export const tickets = {
  list(page = 1, limit = 50): Promise<PaginatedTickets> {
    return api
      .get<PaginatedTickets>('/tickets', { params: { page, limit } })
      .then((r) => r.data);
  },

  get(id: string): Promise<TicketWithThread> {
    return api.get<TicketWithThread>(`/tickets/${id}`).then((r) => r.data);
  },

  create(payload: CreateTicketPayload): Promise<{ id: string }> {
    return api.post<{ id: string }>('/tickets', payload).then((r) => r.data);
  },

  patch(id: string, payload: PatchTicketPayload): Promise<void> {
    return api.patch<void>(`/tickets/${id}`, payload).then(() => undefined);
  },

  // ── Desk-only transitions ────────────────────────────────────────────────
  ack(id: string): Promise<void> {
    return api.patch<void>(`/tickets/${id}/ack`).then(() => undefined);
  },
  pend(id: string): Promise<void> {
    return api.patch<void>(`/tickets/${id}/pend`).then(() => undefined);
  },
  close(id: string): Promise<void> {
    return api.patch<void>(`/tickets/${id}/close`).then(() => undefined);
  },

  // ── Messages ────────────────────────────────────────────────────────────
  /** Post a message. `file` optional, multipart (max 100 MiB enforced server-side). */
  postMessage(id: string, body: string, file?: File): Promise<{ id: string }> {
    const fd = new FormData();
    fd.append('body', body);
    if (file) fd.append('file', file);
    return api.post<{ id: string }>(`/tickets/${id}/message`, fd).then((r) => r.data);
  },

  // ── Internal notes (desk only) ──────────────────────────────────────────
  listNotes(id: string): Promise<InternalNote[]> {
    return api.get<InternalNote[]>(`/tickets/${id}/notes`).then((r) => r.data);
  },
  addNote(id: string, body: string): Promise<InternalNote> {
    return api.post<InternalNote>(`/tickets/${id}/notes`, { body }).then((r) => r.data);
  },

  // ── Desk-only sharing ───────────────────────────────────────────────────
  magicLinkFor(id: string): Promise<MagicLinkResponse> {
    return api.post<MagicLinkResponse>(`/tickets/${id}/magic-link`).then((r) => r.data);
  },

  // ── Desk-only deletion ──────────────────────────────────────────────────
  delete(id: string): Promise<void> {
    return api.delete<void>(`/tickets/${id}`).then(() => undefined);
  },
};
