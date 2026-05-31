// Segmented control with a single sliding highlight pip.
//
// The pip is a single absolutely-positioned span that moves under the active
// option instead of each cell toggling its own background — this gives a
// smooth spring transition between options.
//
// Pip position is recomputed on `value` change AND after fonts load, because
// DM Mono metrics shift once webfonts swap in, which moves offsetLeft.
//
// Usage:
//   <Segmented
//     options={['Low', 'Medium', 'High', 'Urgent']}
//     value={priority}
//     onChange={setPriority}
//     redValue="Urgent"   // optional: pip turns red for this value
//   />

import { useLayoutEffect, useRef } from 'react';
import '../styles/segmented.css';

interface Props {
  options: string[];
  value: string;
  onChange: (v: string) => void;
  /** If the active value matches this, the pip colour becomes var(--red). */
  redValue?: string;
  className?: string;
}

export function Segmented({ options, value, onChange, redValue, className = '' }: Props) {
  const rootRef = useRef<HTMLDivElement>(null);
  const pipRef  = useRef<HTMLSpanElement>(null);

  function updatePip() {
    const root = rootRef.current;
    const pip  = pipRef.current;
    if (!root || !pip) return;
    const active = root.querySelector<HTMLElement>(`[data-v="${value}"]`);
    if (!active) return;
    pip.style.left  = `${active.offsetLeft}px`;
    pip.style.width = `${active.offsetWidth}px`;
    pip.style.background = value === redValue ? 'var(--red)' : 'var(--ink)';
  }

  useLayoutEffect(() => {
    updatePip();
    // Recompute once webfonts are ready — mono metrics shift on swap-in.
    document.fonts.ready.then(updatePip);
  }, [value, redValue]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div ref={rootRef} className={`seg ${className}`}>
      {options.map((o) => (
        <button
          key={o}
          type="button"
          data-v={o}
          className={`seg__o ${o === value ? 'on' : ''}`}
          onClick={() => onChange(o)}
        >
          {o}
        </button>
      ))}
      <span ref={pipRef} className="seg__pip" aria-hidden />
    </div>
  );
}
