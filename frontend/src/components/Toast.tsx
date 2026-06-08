// Toast notification system.
//
// Usage:
//   1. Wrap your app (or a subtree) in <ToastProvider>.
//   2. Call useToast().show('Message here') from anywhere inside it.
//   3. Toasts auto-dismiss after ~2.6 s and play an exit animation.
//
// The shelf is portalled to <body> via a fixed-position div in ToastProvider.

import { createContext, useCallback, useContext, useId, useState } from 'react';
import '../styles/v2.css';

interface ToastItem { id: string; text: string; leaving: boolean; undo?: () => void; }

interface ToastCtx {
  show: (text: string, opts?: { undo?: () => void }) => void;
}

const Ctx = createContext<ToastCtx>({ show: () => {} });

const DISMISS_MS = 2600;
const EXIT_MS    = 320; // must match CSS animation duration

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const baseId = useId();
  let counter = 0;

  const show = useCallback((text: string, opts?: { undo?: () => void }) => {
    const id = `${baseId}-${Date.now()}-${counter++}`;
    setToasts((prev) => [...prev, { id, text, leaving: false, undo: opts?.undo }]);

    // Mark leaving after DISMISS_MS so exit animation plays.
    setTimeout(() => {
      setToasts((prev) => prev.map((t) => t.id === id ? { ...t, leaving: true } : t));
      // Remove from DOM after exit animation.
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, EXIT_MS);
    }, DISMISS_MS);
  }, [baseId]); // eslint-disable-line react-hooks/exhaustive-deps

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.map((t) => t.id === id ? { ...t, leaving: true } : t));
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, EXIT_MS);
  }, []);

  return (
    <Ctx.Provider value={{ show }}>
      {children}
      <div className="lg-toast-shelf" aria-live="polite" aria-atomic>
        {toasts.map((t) => (
          <div key={t.id} className={`lg-toast${t.leaving ? ' is-leaving' : ''}`}>
            {t.text}
            {t.undo && (
              <button
                type="button"
                onClick={() => { t.undo!(); dismiss(t.id); }}
                style={{
                  fontFamily: 'var(--mono)',
                  fontSize: '9px',
                  letterSpacing: '.14em',
                  textTransform: 'uppercase',
                  color: 'var(--red)',
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  marginLeft: '16px',
                  padding: 0,
                }}
              >
                Undo
              </button>
            )}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

export function useToast(): ToastCtx {
  return useContext(Ctx);
}
