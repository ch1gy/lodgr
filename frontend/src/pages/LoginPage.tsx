// ─────────────────────────────────────────────────────────────────────────────
// LoginPage.tsx — sign-in for both desk and clients.
//
// Same door, same form. Two tabs:
//   • Password    : email + password, calls POST /auth/login.
//   • Magic link  : email-only, would call POST /auth/magic-request (TODO —
//                   backend doesn't expose this yet, see "Known gaps" in the
//                   handoff). For now the button is wired but shows a friendly
//                   "ask your desk to generate a link" message.
//
// After successful login we bounce to whatever route the user was trying to
// reach (preserved in location.state.from by ProtectedRoute), or /tickets.
//
// Errors:
//   • 401              — bad email/password, shown inline.
//   • 429 w/ Retry-After — lockout, show countdown.
//   • 429 w/o header   — permanent lockout, show static message.
//   • Anything else    — show the error.error string from the body.
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import type { AxiosError } from 'axios';
import { useAuth } from '../auth/AuthContext';
import type { ApiError } from '../api/types';
import '../styles/login.css';

type Tab = 'password' | 'magic';

// Format mm:ss for the lockout countdown.
function fmtCountdown(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

export function LoginPage() {
  const { token, login } = useAuth();
  const nav = useNavigate();
  const loc = useLocation();

  // Where to send the user after login. ProtectedRoute stashes this for us.
  const from = (loc.state as { from?: { pathname?: string } } | null)?.from?.pathname ?? '/tickets';

  // ── Already signed in? Bounce out. ──────────────────────────────────────
  useEffect(() => {
    if (token) nav(from, { replace: true });
  }, [token, from, nav]);

  // ── Form state ──────────────────────────────────────────────────────────
  const [tab, setTab] = useState<Tab>('password');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<{ title: string; body: string } | null>(null);
  const [lockoutUntil, setLockoutUntil] = useState<number | null>(null);

  // ── Tick the lockout countdown every second ─────────────────────────────
  const [nowMs, setNowMs] = useState(() => Date.now());
  useEffect(() => {
    if (!lockoutUntil) return;
    const i = setInterval(() => setNowMs(Date.now()), 1000);
    return () => clearInterval(i);
  }, [lockoutUntil]);
  const lockoutSec = useMemo(() => {
    if (!lockoutUntil) return 0;
    return Math.max(0, Math.ceil((lockoutUntil - nowMs) / 1000));
  }, [lockoutUntil, nowMs]);
  const locked = lockoutSec > 0;

  // ── Submit ──────────────────────────────────────────────────────────────
  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (submitting || locked) return;
    setError(null);

    if (tab === 'magic') {
      // Backend doesn't yet expose a "send magic link" endpoint for self-serve;
      // the handoff lists POST /admin/clients/:id/magic-link as desk-only.
      // Until that lands, surface a helpful message.
      setError({
        title: 'Magic link not yet available',
        body:
          'Ask the desk to generate one for you. Once the self-serve endpoint ships, this button will do the rest.',
      });
      return;
    }

    setSubmitting(true);
    try {
      await login(email, password);
      // Effect above will navigate when `token` flips.
    } catch (err) {
      const ax = err as AxiosError<ApiError>;
      const status = ax.response?.status;

      if (status === 429) {
        // Lockout. Server may include Retry-After (seconds).
        const ra = ax.response?.headers?.['retry-after'];
        const secs = typeof ra === 'string' ? parseInt(ra, 10) : NaN;
        if (Number.isFinite(secs) && secs > 0) {
          setLockoutUntil(Date.now() + secs * 1000);
          setError({
            title: 'Too many attempts',
            body: `Try again in a moment. We'll re-enable the form when the lockout lifts.`,
          });
        } else {
          setError({
            title: 'Account locked',
            body:
              ax.response?.data?.error ??
              'Account is permanently locked. Contact support to unlock.',
          });
        }
      } else if (status === 401) {
        setError({
          title: 'Those credentials didn\'t work',
          body: 'Check the email and password, or use a magic link.',
        });
      } else {
        setError({
          title: 'Something went wrong',
          body: ax.response?.data?.error ?? ax.message ?? 'Unknown error',
        });
      }
    } finally {
      setSubmitting(false);
    }
  }

  // ── Render ─────────────────────────────────────────────────────────────
  return (
    <div className="lg-login grain">
      <div className="lg-login__band">
        <span>
          <span className="dot" />
          lodgr · v0.1.0
        </span>
        <span>
          {new Date().toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: 'numeric' })}
        </span>
      </div>

      <div className="lg-login__core">
        {/* ── Editorial left ────────────────────────────────────────────── */}
        <section className="lg-login__left">
          <div className="lg-login__bg-num" aria-hidden>L</div>
          <div className="lg-login__left-top">
            <div className="lg-login__logo">
              <i>Lodgr</i>
              <span className="dot">.</span>
            </div>
            <div className="lg-login__meta">
              Self-hosted support deck
              <br />
              Vol. III — {new Date().getFullYear()}
              <br />
              <b style={{ color: 'var(--red)' }}>● Live</b>
            </div>
          </div>

          <h1 className="lg-login__pull">
            A quiet place
            <br />
            for <em>noisy</em> days.
            <span className="lg-login__pull-small">
              A self-hosted support deck where one agent and a handful of clients meet.
              No queues, no chatbots, no escalation matrix — just the conversation and
              the people in it.
            </span>
          </h1>

          <div className="lg-login__ftr">
            <span>Built in <b>Nairobi</b> · Rust + React</span>
            <span><b>—</b> {new Date().toLocaleDateString('en-GB', { weekday: 'long' })}</span>
          </div>
        </section>

        {/* ── Form right ────────────────────────────────────────────────── */}
        <section className="lg-login__right">
          <div className="lg-login__accent" aria-hidden />
          <div className="lg-login__eyebrow">Section 01 — Sign in</div>
          <h2 className="lg-login__h2">
            Welcome back<span className="mid">,<br />to Lodgr.</span>
          </h2>
          <p className="lg-login__lede">
            Whether you're running the desk or just checking on your tickets — same
            door, same sign-in.
          </p>

          <form onSubmit={handleSubmit} noValidate>
            <div className="lg-login__tabs" role="tablist">
              <button
                type="button"
                role="tab"
                className={'lg-login__tab' + (tab === 'password' ? ' is-active' : '')}
                onClick={() => { setTab('password'); setError(null); }}
              >
                Password
              </button>
              <button
                type="button"
                role="tab"
                className={'lg-login__tab' + (tab === 'magic' ? ' is-active' : '')}
                onClick={() => { setTab('magic'); setError(null); }}
              >
                Email me a link
              </button>
            </div>

            {error && (
              <div className="lg-login__error" role="alert">
                <b>{error.title}</b>
                {error.body}
                {locked && <> · <b>retry in {fmtCountdown(lockoutSec)}</b></>}
              </div>
            )}

            <div className="lg-login__field">
              <label className="lg-input-label" htmlFor="login-email">Email</label>
              <input
                id="login-email"
                className="lg-input"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                disabled={locked}
                required
              />
            </div>

            {tab === 'password' ? (
              <div className="lg-login__field">
                <label className="lg-input-label" htmlFor="login-password">Password</label>
                <input
                  id="login-password"
                  className="lg-input"
                  type="password"
                  autoComplete="current-password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={locked}
                  required
                />
              </div>
            ) : (
              <p className="lg-login__explainer">
                We'll send a one-time sign-in link to your inbox. The link expires in{' '}
                <b>15 minutes</b> and works once.
              </p>
            )}

            <div className="lg-login__actions">
              <button type="submit" className="lg-login__send" disabled={submitting || locked}>
                {tab === 'password' ? 'Sign in' : 'Send the link'}{' '}
                <span className="arr">→</span>
              </button>
              {tab === 'password' ? (
                <button type="button" className="lg-login__forgot" onClick={() => setTab('magic')}>
                  Forgot password
                </button>
              ) : (
                <button type="button" className="lg-login__forgot" onClick={() => setTab('password')}>
                  Use password
                </button>
              )}
            </div>
          </form>

          <div className="lg-login__helper">
            <span className="dot" />
            <span>
              {tab === 'magic'
                ? <>Best for <b>clients</b> · skip the password, jump straight to your tickets</>
                : <>Clients · check your inbox for a <b>magic link</b> if you'd rather skip the password</>}
            </span>
          </div>

          <div className="lg-login__legal">
            <span>Self-hosted</span>
            <span>Session · 15 min</span>
            <span>Lockout · 5 attempts</span>
          </div>
        </section>
      </div>
    </div>
  );
}
