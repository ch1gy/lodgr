// useFlip.ts — FLIP (First-Last-Invert-Play) animation for list re-sorts (§6.11)
// Usage:
//   const { setRef, play } = useFlip();
//   // call play() BEFORE updating the list order in state
//   // setRef(id) as a ref callback on each item: ref={setRef(item.id)}

import { useCallback, useRef } from 'react';

export function useFlip(durationMs = 560) {
  const rects = useRef<Map<string, DOMRect>>(new Map());

  const capture = useCallback((id: string, el: HTMLElement | null) => {
    if (el) rects.current.set(id, el.getBoundingClientRect());
  }, []);

  const play = useCallback((getEl: (id: string) => HTMLElement | null, ids: string[]) => {
    // "Last" — measure new positions after DOM update
    for (const id of ids) {
      const el = getEl(id);
      const first = rects.current.get(id);
      if (!el || !first) continue;
      const last = el.getBoundingClientRect();
      const dy = first.top - last.top;
      const dx = first.left - last.left;
      if (Math.abs(dy) < 1 && Math.abs(dx) < 1) continue;
      // Invert + Play
      el.style.transition = 'none';
      el.style.transform = `translate(${dx}px, ${dy}px)`;
      requestAnimationFrame(() => {
        el.style.transition = `transform ${durationMs}ms cubic-bezier(0.16, 1, 0.30, 1)`;
        el.style.transform = '';
      });
    }
    rects.current.clear();
  }, [durationMs]);

  return { capture, play };
}
