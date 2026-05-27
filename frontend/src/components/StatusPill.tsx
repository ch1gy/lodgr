// ─────────────────────────────────────────────────────────────────────────────
// StatusPill.tsx — small status indicator for a ticket.
//
// `open` is the only status that pulses (animated dot). All other states are
// quiet — the pulse is our way of signalling "this needs a human".
// Styles are defined in tokens.css under .lg-status*.
// ─────────────────────────────────────────────────────────────────────────────

import type { TicketStatus } from '../api/types';

const LABEL: Record<TicketStatus, string> = {
  open: 'Open',
  acknowledged: 'Acknowledged',
  pending: 'Pending',
  closed: 'Closed',
};

const CLASS: Record<TicketStatus, string> = {
  open: 'open',
  acknowledged: 'ack',
  pending: 'pending',
  closed: 'closed',
};

export function StatusPill({ status }: { status: TicketStatus }) {
  return (
    <span className={`lg-status lg-status--${CLASS[status]}`}>
      <span className="dot" />
      {LABEL[status]}
    </span>
  );
}
