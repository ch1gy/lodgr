import { Component, ErrorInfo, ReactNode } from 'react';

interface Props { children: ReactNode; }
interface State { error: Error | null; }

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[Lodgr] Unhandled render error:', error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{
          minHeight: '100vh',
          background: '#f2ede4',
          display: 'grid',
          placeItems: 'center',
          fontFamily: '"DM Mono", monospace',
          textAlign: 'center',
          padding: 40,
          color: '#0d0d0d',
        }}>
          <div>
            <div style={{ fontSize: 11, letterSpacing: '.18em', textTransform: 'uppercase', color: '#c0392b', marginBottom: 20 }}>
              — Lodgr · something went wrong —
            </div>
            <h2 style={{ fontFamily: '"DM Serif Display", serif', fontStyle: 'italic', fontSize: 32, fontWeight: 400, marginBottom: 12 }}>
              Unexpected error.
            </h2>
            <p style={{ color: '#6a6560', fontSize: 13, marginBottom: 28, maxWidth: 420, lineHeight: 1.6 }}>
              {this.state.error.message}
            </p>
            <button
              onClick={() => window.location.reload()}
              style={{
                fontFamily: '"DM Mono", monospace', fontSize: 11, letterSpacing: '.14em',
                textTransform: 'uppercase', cursor: 'pointer', padding: '10px 24px',
                background: '#0d0d0d', color: '#f2ede4', border: 'none',
              }}
            >
              Reload the page
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
