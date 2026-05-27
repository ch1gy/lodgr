// ── Admin API — all endpoints under /admin ──────────────────────────────────
import { api } from './client';
import type {
  Client,
  CreateClientPayload,
  ExportResponse,
  MagicLinkResponse,
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
};
