// SlaOdometer.tsx — live SLA countdown chip (§6.8)
// Digits animate via CSS translateY — each digit slot shows a 0-9 strip
// shifted to the current digit. < 24h ticks every second; ≥ 24h is static.

import { useEffect, useRef, useState } from 'react';
import '../styles/sla.css';

interface Props {
  /** ISO date string, or null if no due date */
  dueDate: string | null;
  /** Optional: estimated_completion also accepted */
  estimatedCompletion?: string | null;
  /** Size variant */
  size?: 'sm' | 'md';
}

type Bucket = 'hot' | 'warn' | 'ok' | 'over';

function getBucket(msLeft: number): Bucket {
  if (msLeft < 0) return 'over';
  if (msLeft < 3_600_000) return 'hot';      // < 1 hour
  if (msLeft < 21_600_000) return 'warn';    // < 6 hours
  return 'ok';
}

// Format milliseconds into display parts
function formatMs(ms: number): { display: string; tick: boolean } {
  const abs = Math.abs(ms);
  const totalSec = Math.floor(abs / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;

  if (abs < 86_400_000) {
    // < 24h: HH:MM:SS
    const hh = String(h).padStart(2, '0');
    const mm = String(m).padStart(2, '0');
    const ss = String(s).padStart(2, '0');
    return { display: `${hh}:${mm}:${ss}`, tick: true };
  }
  // ≥ 24h: Nd HHh
  const days = Math.floor(abs / 86_400_000);
  return { display: `${days}d ${String(h % 24).padStart(2, '0')}h`, tick: false };
}

// A single rolling digit slot
function OdDigit({ digit }: { digit: string }) {
  const isNum = /^\d$/.test(digit);
  if (!isNum) {
    return <span className="sla-od__sep">{digit}</span>;
  }
  const d = parseInt(digit, 10);
  return (
    <span className="sla-od__slot" aria-hidden>
      <span className="sla-od__strip" style={{ transform: `translateY(${-d * 10}%)` }}>
        {Array.from({ length: 10 }, (_, i) => (
          <span key={i} className="sla-od__digit">{i}</span>
        ))}
      </span>
    </span>
  );
}

export function SlaOdometer({ dueDate, estimatedCompletion, size = 'sm' }: Props) {
  const target = dueDate ?? estimatedCompletion ?? null;
  const [now, setNow] = useState(() => Date.now());
  const tickRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const ms = target ? new Date(target).getTime() - now : null;
  const bucket = ms !== null ? getBucket(ms) : 'ok';
  const { display, tick } = ms !== null ? formatMs(ms) : { display: '—', tick: false };
  const overdue = ms !== null && ms < 0;

  useEffect(() => {
    if (!target || !tick) { if (tickRef.current) clearInterval(tickRef.current); return; }
    tickRef.current = setInterval(() => setNow(Date.now()), 1000);
    return () => { if (tickRef.current) clearInterval(tickRef.current); };
  }, [target, tick]);

  if (!target) return null;

  return (
    <span
      className={`sla-od sla-od--${bucket} sla-od--${size}`}
      aria-label={overdue ? `SLA breached ${display} ago` : `${display} remaining`}
    >
      {display.split('').map((ch, i) => (
        <OdDigit key={i} digit={ch} />
      ))}
      {overdue && <span className="sla-od__breach" aria-hidden>▲</span>}
    </span>
  );
}
