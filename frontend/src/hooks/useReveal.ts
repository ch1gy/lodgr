// Adds `.is-in` to the referenced element when it enters the viewport.
// Use the `.reveal` + `.d2/.d3/.d4` CSS classes for the animation.
//
// Usage:
//   const ref = useReveal<HTMLDivElement>();
//   <div ref={ref} className="reveal">…</div>

import { useEffect, useRef } from 'react';

export function useReveal<T extends HTMLElement>(threshold = 0.2) {
  const ref = useRef<T>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(
      ([entry]) => el.classList.toggle('is-in', entry.isIntersecting),
      { threshold }
    );
    io.observe(el);
    return () => io.disconnect();
  }, [threshold]);

  return ref;
}
