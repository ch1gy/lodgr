// DraggableRow.tsx — swipe-to-triage wrapper (§6.10)
//
// Right swipe → Acknowledge: rubber-bands past threshold, springs back, toasts.
// Left  swipe → Resolve:     delays the API call 2.4s so Undo can cancel it.
// Click (no movement) → passes through to the inner Link.
//
// Uses refs (not state) for the drag offset — frame-synchronous, no re-render
// while dragging. touch-action:pan-y keeps vertical scrolling smooth on mobile.

import { ReactNode, useCallback, useEffect, useRef } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { tickets as ticketsApi } from '../api/tickets';
import { useToast } from './Toast';

const THRESHOLD = 130;
const RESISTANCE = 0.2;

interface Props {
  id: string;
  status: string;
  children: ReactNode;
}

export function DraggableRow({ id, status, children }: Props) {
  const wrapRef     = useRef<HTMLDivElement>(null);
  const startX      = useRef(0);
  const curDx       = useRef(0);
  const dragging    = useRef(false);
  const didDrag     = useRef(false);
  const closeTimer  = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { show } = useToast();
  const qc = useQueryClient();

  const ackMut = useMutation({
    mutationFn: () => ticketsApi.ack(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tickets'] }),
  });

  const closeMut = useMutation({
    mutationFn: () => ticketsApi.close(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tickets'] }),
  });

  useEffect(() => {
    return () => { if (closeTimer.current) clearTimeout(closeTimer.current); };
  }, []);

  const setTranslate = useCallback((x: number, animated: boolean) => {
    const el = wrapRef.current;
    if (!el) return;
    el.style.transition = animated
      ? `transform var(--dur-base) var(--ease-spring), opacity var(--dur-base) var(--ease-out)`
      : 'none';
    el.style.transform = x === 0 ? '' : `translateX(${x}px)`;
  }, []);

  const onPointerDown = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    if (e.pointerType === 'mouse' && e.button !== 0) return;
    startX.current = e.clientX;
    curDx.current = 0;
    dragging.current = true;
    didDrag.current = false;
    (e.currentTarget as HTMLDivElement).setPointerCapture(e.pointerId);
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragging.current) return;
    const dx = e.clientX - startX.current;
    if (Math.abs(dx) > 4) didDrag.current = true;
    curDx.current = dx;

    const sign = dx > 0 ? 1 : -1;
    const abs = Math.abs(dx);
    const effective = abs > THRESHOLD
      ? THRESHOLD + (abs - THRESHOLD) * RESISTANCE
      : abs;
    setTranslate(sign * effective, false);
  }, [setTranslate]);

  const onPointerUp = useCallback(() => {
    if (!dragging.current) return;
    dragging.current = false;
    const dx = curDx.current;

    if (dx > THRESHOLD && status === 'open') {
      // Acknowledge — spring back, fire mutation, toast
      setTranslate(0, true);
      ackMut.mutate();
      show('Acknowledged ✓');

    } else if (dx < -THRESHOLD && status !== 'closed') {
      // Resolve — fling left, delay API call so Undo can cancel it
      const el = wrapRef.current;
      if (el) {
        el.style.transition = `transform var(--dur-base) var(--ease-in-out), opacity var(--dur-base) var(--ease-out)`;
        el.style.transform = `translateX(-110vw)`;
        el.style.opacity = '0';
      }

      const undo = () => {
        if (closeTimer.current) clearTimeout(closeTimer.current);
        if (el) {
          el.style.transition = `transform var(--dur-base) var(--ease-spring), opacity var(--dur-fast) var(--ease-out)`;
          el.style.transform = '';
          el.style.opacity = '';
        }
      };

      show('Ticket resolved', { undo });

      // Fire the API call just before the toast auto-dismisses
      closeTimer.current = setTimeout(() => {
        closeMut.mutate();
      }, 2400);

    } else {
      // No action — snap back
      setTranslate(0, true);
    }
  }, [status, ackMut, closeMut, show, setTranslate]);

  const onClick = useCallback((e: React.MouseEvent) => {
    // If the pointer moved during drag, block navigation
    if (didDrag.current) {
      e.preventDefault();
      e.stopPropagation();
    }
  }, []);

  return (
    <div
      className="lg-drag-wrap"
      onClick={onClick}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      style={{ touchAction: 'pan-y', userSelect: 'none', position: 'relative', overflow: 'hidden' }}
    >
      <div className="lg-drag-rail lg-drag-rail--ack" aria-hidden>✓ Acknowledge</div>
      <div className="lg-drag-rail lg-drag-rail--res" aria-hidden>× Resolve</div>
      <div ref={wrapRef} style={{ position: 'relative', zIndex: 1 }}>
        {children}
      </div>
    </div>
  );
}
