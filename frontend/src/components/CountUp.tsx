// CountUp.tsx — KPI number that rolls up on mount (§6.9)
// Uses the same odometer digit logic as SlaOdometer but animates 0→N on first render.

import { useEffect, useState } from 'react';

interface Props {
  value: number;
  /** Duration in ms (default 900) */
  duration?: number;
  /** Pad to N digits with leading zeros */
  pad?: number;
  className?: string;
}

export function CountUp({ value, duration = 900, pad = 2, className = '' }: Props) {
  const [displayed, setDisplayed] = useState(0);

  useEffect(() => {
    if (value === 0) return;
    const start = performance.now();
    let raf: number;
    function tick(now: number) {
      const progress = Math.min((now - start) / duration, 1);
      // ease-out-quart
      const eased = 1 - Math.pow(1 - progress, 4);
      setDisplayed(Math.round(eased * value));
      if (progress < 1) raf = requestAnimationFrame(tick);
    }
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [value, duration]);

  const str = String(displayed).padStart(pad, '0');
  return <span className={className}>{str}</span>;
}
