// useMountTransition.ts — keep element mounted during exit animation
// Returns { mounted, show } where:
//   mounted = element should be in the DOM
//   show    = element should have its "entered" CSS class
//
// Usage:
//   const { mounted, show } = useMountTransition(isOpen, 320);
//   return mounted ? <div className={show ? 'is-open' : ''} /> : null;

import { useEffect, useState } from 'react';

export function useMountTransition(open: boolean, exitDurationMs: number) {
  const [mounted, setMounted] = useState(open);
  const [show, setShow] = useState(false);

  useEffect(() => {
    if (open) {
      setMounted(true);
      // One frame delay so the browser paints the initial (hidden) state
      // before we add the class that triggers the enter transition.
      const raf = requestAnimationFrame(() => setShow(true));
      return () => cancelAnimationFrame(raf);
    } else {
      setShow(false);
      const t = setTimeout(() => setMounted(false), exitDurationMs);
      return () => clearTimeout(t);
    }
  }, [open, exitDurationMs]);

  return { mounted, show };
}
