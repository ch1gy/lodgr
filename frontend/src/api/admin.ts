// ── Admin API — all endpoints under /admin ──────────────────────────────────
import { api } from './client';
import type {
  Client,
  CreateClientPayload,
  CreateInvoicePayload,
  DeskProfile,
  ExportResponse,
  InvoiceResponse,
  MagicLinkResponse,
  SubClient,
  UpdateInvoicePayload,
} from './types';

export interface DeleteSessionsResponse {
  deleted: number;
}

export const admin = {
  listClients(): Promise<Client[]> {
    return api.get<Client[]>('/admin/clients').then((r) => r.data);
  },

  createClient(payload: CreateClientPayload): Promise<Client> {
    return api.post<Client>('/admin/clients', payload).then((r) => r.data);
  },

  deleteClientSessions(id: string): Promise<DeleteSessionsResponse> {
    return api
      .delete<DeleteSessionsResponse>(`/admin/clients/${id}/sessions`)
      .then((r) => r.data);
  },

  softDeleteClient(id: string): Promise<void> {
    return api.post(`/admin/clients/${id}/soft-delete`).then(() => undefined);
  },

  restoreClient(id: string): Promise<void> {
    return api.post(`/admin/clients/${id}/restore`).then(() => undefined);
  },

  /** `confirm` must be the string `"DELETE"` to succeed server-side. */
  hardDeleteClient(id: string, confirm: string): Promise<void> {
    return api
      .delete(`/admin/clients/${id}`, { data: { confirm } })
      .then(() => undefined);
  },

  updateClient(id: string, payload: Partial<{ name: string; email: string; address_line1: string; address_line2: string; pin_number: string; contact_person: string; phone: string }>): Promise<Client> {
    return api.patch<Client>(`/admin/clients/${id}`, payload).then((r) => r.data);
  },

  unlockClient(id: string): Promise<void> {
    return api.post(`/admin/clients/${id}/unlock`).then(() => undefined);
  },

  exportClient(id: string): Promise<ExportResponse> {
    return api
      .post<ExportResponse>(`/admin/clients/${id}/export`)
      .then((r) => r.data);
  },

  /** Download the export file. Returns the JSON bytes as a Blob.
   *  The `downloadUrl` is the path from ExportResponse.download_url. */
  downloadExport(downloadUrl: string): Promise<Blob> {
    return api
      .get<Blob>(downloadUrl, { responseType: 'blob' })
      .then((r) => r.data);
  },

  createMagicLink(id: string): Promise<MagicLinkResponse> {
    return api
      .post<MagicLinkResponse>(`/admin/clients/${id}/magic-link`)
      .then((r) => r.data);
  },

  listSubClients(clientId: string): Promise<SubClient[]> {
    return api.get<SubClient[]>(`/admin/clients/${clientId}/sub-clients`).then((r) => r.data);
  },

  createSubClient(clientId: string, name: string): Promise<SubClient> {
    return api
      .post<SubClient>(`/admin/clients/${clientId}/sub-clients`, { name })
      .then((r) => r.data);
  },

  deleteSubClient(subClientId: string): Promise<void> {
    return api.delete(`/admin/sub-clients/${subClientId}`).then(() => undefined);
  },

  // ── Invoices ───────────────────────────────────────────────────────────────

  listInvoices(clientId?: string): Promise<InvoiceResponse[]> {
    return api
      .get<InvoiceResponse[]>('/admin/invoices', { params: clientId ? { client_id: clientId } : undefined })
      .then((r) => r.data);
  },

  getInvoice(id: string): Promise<InvoiceResponse> {
    return api.get<InvoiceResponse>(`/admin/invoices/${id}`).then((r) => r.data);
  },

  createInvoice(payload: CreateInvoicePayload): Promise<InvoiceResponse> {
    return api.post<InvoiceResponse>('/admin/invoices', payload).then((r) => r.data);
  },

  updateInvoice(id: string, payload: UpdateInvoicePayload): Promise<InvoiceResponse> {
    return api.patch<InvoiceResponse>(`/admin/invoices/${id}`, payload).then((r) => r.data);
  },

  deleteInvoice(id: string): Promise<void> {
    return api.delete(`/admin/invoices/${id}`).then(() => undefined);
  },

  /** Returns the URL to open in a new tab for print/PDF. */
  invoicePrintUrl(id: string): string {
    return `/admin/invoices/${id}/print`;
  },

  getDeskProfile(): Promise<DeskProfile> {
    return api.get<DeskProfile>('/admin/desk-profile').then((r) => r.data);
  },

  updateDeskProfile(payload: Partial<DeskProfile>): Promise<DeskProfile> {
    return api.put<DeskProfile>('/admin/desk-profile', payload).then((r) => r.data);
  },
};
