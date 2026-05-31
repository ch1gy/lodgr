// Keep a component mounted for `ms` after `open` goes false so the CSS
// exit animation can play before the element is removed from the DOM.
//
// Usage:
//   const mounted = useMounted(open);
//   return mounted ? (
//     <div className={`lg-ov ${open ? 'is-open' : ''}`}>…</div>
//   ) : null;

import { useEffect, useState } from 'react';

export function useMounted(open: boolean, ms = 360): boolean {
  const [mounted, setMounted] = useState(open);

  useEffect(() => {
    if (open) {
      setMounted(true);
      return;
    }
    const t = setTimeout(() => setMounted(false), ms);
    return () => clearTimeout(t);
  }, [open, ms]);

  return mounted;
}
