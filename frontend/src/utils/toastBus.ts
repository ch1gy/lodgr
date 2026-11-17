// A tiny emitter so code outside React (the QueryClient's global error handler)
// can raise a toast. ToastProvider subscribes while it is mounted.

export interface ToastRequest {
  text: string;
  kind: 'info' | 'error';
}

type Listener = (t: ToastRequest) => void;

const listeners = new Set<Listener>();

export function subscribeToasts(fn: Listener): () => void {
  listeners.add(fn);
  return () => { listeners.delete(fn); };
}

export function emitToast(t: ToastRequest): void {
  listeners.forEach((fn) => fn(t));
}
