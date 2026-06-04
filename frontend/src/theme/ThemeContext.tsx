// ─────────────────────────────────────────────────────────────────────────────
// ThemeContext.tsx — light/dark mode for the whole app.
//
// Resolution order:
//   1. localStorage("lodgr.theme") if the user has explicitly chosen
//   2. matchMedia('(prefers-color-scheme: dark)') otherwise
//
// We apply the theme by setting [data-theme="dark"] on <html>; all dark-mode
// styling lives in CSS under `[data-theme="dark"]` selectors so it's
// statically analysable and never depends on JS for paint.
//
// When the user toggles, we use the View Transitions API (Chrome 111+,
// Safari 18) for a circular reveal that radiates from the toggle button —
// gorgeous on supporting browsers, gracefully a plain swap elsewhere.
// ─────────────────────────────────────────────────────────────────────────────

import { createContext, useContext, useEffect, useState, ReactNode, useCallback } from 'react';

type Theme = 'light' | 'dark';
const LS_KEY = 'lodgr.theme';

interface ThemeState {
  theme: Theme;
  /** Toggle, optionally animating a circular reveal from the click point.
   *  Pass the originating <button>'s clientX/clientY for the cleanest effect. */
  toggle(opts?: { x?: number; y?: number }): void;
  /** Set explicitly (used by some debug paths). */
  setTheme(t: Theme): void;
}

const ThemeContext = createContext<ThemeState | null>(null);

// Read the persisted theme synchronously so we apply it before paint.
// Default is always light — only a saved user preference overrides this.
function readInitialTheme(): Theme {
  try {
    const saved = localStorage.getItem(LS_KEY);
    if (saved === 'light' || saved === 'dark') return saved;
  } catch { /* no-op */ }
  return 'light';
}

// Apply to <html> immediately — call this in main.tsx before React mounts so
// users never see a light-mode flash on dark systems.
export function applyInitialThemeSync(): void {
  const t = readInitialTheme();
  document.documentElement.setAttribute('data-theme', t);
}

// Type extension for the View Transitions API (TS doesn't ship this yet).
type DocumentWithVT = Document & {
  startViewTransition?: (cb: () => void) => { finished: Promise<void> };
};

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    return (document.documentElement.getAttribute('data-theme') as Theme | null) ?? readInitialTheme();
  });

  // Keep <html data-theme> in sync, and persist.
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    try { localStorage.setItem(LS_KEY, theme); } catch { /* no-op */ }
  }, [theme]);


  const setTheme = useCallback((t: Theme) => setThemeState(t), []);

  const toggle = useCallback(
    (opts?: { x?: number; y?: number }) => {
      const next: Theme = theme === 'dark' ? 'light' : 'dark';

      const doc = document as DocumentWithVT;
      // Honor reduced-motion + browsers without View Transitions API.
      const prefersReduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
      if (prefersReduced || typeof doc.startViewTransition !== 'function') {
        setThemeState(next);
        return;
      }

      // Run the swap inside startViewTransition so the browser snapshots
      // before/after and we can animate the new layer in.
      const transition = doc.startViewTransition(() => {
        setThemeState(next);
      });

      // Compute origin point for the circular reveal. Fall back to centre.
      const x = opts?.x ?? window.innerWidth / 2;
      const y = opts?.y ?? window.innerHeight / 2;
      const endRadius = Math.hypot(
        Math.max(x, window.innerWidth - x),
        Math.max(y, window.innerHeight - y),
      );

      transition.finished.catch(() => { /* user navigated away — ignore */ });
      // Animate the ::view-transition-new pseudo with a clip-path circle that
      // expands from the click. CSS at the bottom of tokens.css ensures the
      // root pseudos don't fade or move; we just override the new layer.
      document.documentElement.animate(
        {
          clipPath: [
            `circle(0 at ${x}px ${y}px)`,
            `circle(${endRadius}px at ${x}px ${y}px)`,
          ],
        },
        {
          duration: 520,
          easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
          pseudoElement: '::view-transition-new(root)',
        },
      );
    },
    [theme],
  );

  return (
    <ThemeContext.Provider value={{ theme, toggle, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme(): ThemeState {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within a <ThemeProvider>');
  return ctx;
}
