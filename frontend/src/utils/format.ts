import type { TicketType } from '../api/types';

// ── File download helper ──────────────────────────────────────────────────

/** Trigger a browser download from a Blob. */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

// ── Time formatters ────────────────────────────────────────────────────────

/** Human "x time ago" for a UTC ISO-8601 timestamp. */
export function timeAgo(iso: string): string {
  const ms = Date.now() - +new Date(iso);
  const m = Math.floor(ms / 60_000);
  if (m < 1) return 'just now';
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  if (d < 7) return `${d}d ago`;
  return new Date(iso).toLocaleDateString('en-GB', { day: '2-digit', month: 'short' });
}

/** Days until a YYYY-MM-DD date string (negative = overdue, 0 = today). */
export function daysUntil(dueDate: string | null): number | null {
  if (!dueDate) return null;
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const due = new Date(dueDate + 'T00:00:00');
  return Math.round((+due - +today) / 86_400_000);
}

/** Short locale date+time string for thread entry timestamps. */
export function fmtDateTime(iso: string): string {
  return new Date(iso).toLocaleString('en-GB', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

// ── Ticket type display labels ─────────────────────────────────────────────
// Typed record so a new backend value triggers a TS error rather than
// silently displaying a raw underscore-separated string at runtime.

export const TICKET_TYPE_LABEL: Record<TicketType, string> = {
  standard: 'Standard',
  maintenance: 'Maintenance',
  security_log: 'Security log',
};
