// MorphContext.tsx — shared state for the row→detail View Transitions morph (§6.2)
//
// morphId stores the ticket id that is currently being navigated to, so only
// that row's elements receive view-transition-name attributes for the snapshot.
// The detail page always has matching names — it just waits for the group to arrive.

import { createContext, useContext, useState, ReactNode } from 'react';

interface MorphState {
  morphId: string | null;
  setMorphId: (id: string | null) => void;
}

const MorphCtx = createContext<MorphState>({ morphId: null, setMorphId: () => {} });

export function MorphProvider({ children }: { children: ReactNode }) {
  const [morphId, setMorphId] = useState<string | null>(null);
  return <MorphCtx.Provider value={{ morphId, setMorphId }}>{children}</MorphCtx.Provider>;
}

export function useMorph(): MorphState {
  return useContext(MorphCtx);
}

// Type extension for the VT API (not yet in TypeScript's lib.dom.d.ts).
export type DocumentWithVT = Document & {
  startViewTransition?: (cb: () => void) => { finished: Promise<void> };
};
