// ─────────────────────────────────────────────────────────────────────────────
// PriorityBars.tsx — 1..4 vertical bars (low → urgent), color shifts with prio.
//
// Visual model lifted from cellular-signal indicators. The text label sits
// next to the bars; colors come from tokens.css (.lg-prio--*).
// ─────────────────────────────────────────────────────────────────────────────

import type { TicketPriority } from '../api/types';

const FILLED: Record<TicketPriority, number> = {
  low: 1,
  medium: 2,
  high: 3,
  urgent: 4,
};

export function PriorityBars({ priority }: { priority: TicketPriority }) {
  const filled = FILLED[priority];
  return (
    <span className={`lg-prio lg-prio--${priority}`}>
      <span className="bars">
        {[0, 1, 2, 3].map((i) => (
          <span key={i} className={i < filled ? '' : 'off'} />
        ))}
      </span>
      {priority}
    </span>
  );
}
