import type { RecurInterval, TicketType } from '../api/types';

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

/**
 * Add one recurrence interval to a YYYY-MM-DD date, clamping to the end of the
 * target month. Mirrors the backend's advance_recur_date. Returns '' for input
 * that is not a date.
 */
export function addInterval(isoDate: string, interval: RecurInterval): string {
  const [y, m, d] = isoDate.split('-').map(Number);
  if (!y || !m || !d) return '';
  const months = interval === 'monthly' ? 1 : interval === 'quarterly' ? 3 : 12;
  const lastDay = new Date(Date.UTC(y, m + months, 0)).getUTCDate();
  return new Date(Date.UTC(y, m - 1 + months, Math.min(d, lastDay))).toISOString().slice(0, 10);
}

/** Human wording for a link lifetime ("1 hour", "24 hours", "15 minutes"). Mirrors the backend. */
export function describeTtl(secs: number): string {
  const s = Math.max(secs, 60);
  const [n, unit] = s >= 3600 && s % 3600 === 0 ? [s / 3600, 'hour'] : [Math.ceil(s / 60), 'minute'];
  return `${n} ${unit}${n === 1 ? '' : 's'}`;
}

/**
 * Display name for a stored attachment path such as `/uploads/<ticket>/ab12cd34-report.pdf`:
 * the file name without the uniqueness prefix the server adds.
 */
export function attachmentName(path: string): string {
  const last = path.split('/').pop() ?? path;
  return last.replace(/^[0-9a-f]{8}-/i, '');
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

// ── API error extraction ───────────────────────────────────────────────────

/** Extract the server-side `error` string from an axios error response. */
export function extractApiError(err: unknown, fallback = 'Something went wrong'): string {
  return (err as { response?: { data?: { error?: string } } })?.response?.data?.error ?? fallback;
}

export const TICKET_TYPE_LABEL: Record<TicketType, string> = {
  standard: 'Standard',
  maintenance: 'Maintenance',
  security_log: 'Security log',
};
