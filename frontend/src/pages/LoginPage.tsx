// ─────────────────────────────────────────────────────────────────────────────
// LoginPage.tsx — SIMPLE sign-in. Logo + credentials, nothing else.
// Look ported 1:1 from the approved prototype `login/Simple Login.html`
// (design.md §4.1). The old two-column editorial login was retired.
//
// Password ⇄ magic-link is one inline switch (not two tabs):
//   • Password : email + password → POST /auth/login (via useAuth().login).
//   • Magic    : email-only → POST /auth/magic-request. Always shows the same
//                "if registered, a link has been sent" message — enumeration-safe.
//
// Auth/error behaviour is unchanged from before:
//   401 → inline "credentials didn't work"
//   429 + Retry-After → lockout countdown ; 429 w/o header → permanent lock
//   else → error.error body string
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import type { AxiosError } from 'axios';
import { useAuth } from '../auth/AuthContext';
import { useTheme } from '../theme/ThemeContext';
import { auth as authApi } from '../api/auth';
import type { ApiError } from '../api/types';
import '../styles/login.css';

type Mode = 'password' | 'magic';

function fmtCountdown(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

export function LoginPage() {
  const { token, login } = useAuth();
  const { theme, toggle } = useTheme();
  const nav = useNavigate();
  const loc = useLocation();

  const from = (loc.state as { from?: { pathname?: string } } | null)?.from?.pathname ?? '/tickets';

  useEffect(() => {
    if (token) nav(from, { replace: true });
  }, [token, from, nav]);

  const [mode, setMode] = useState<Mode>('password');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [magicSent, setMagicSent] = useState(false);
  const [error, setError] = useState<{ title: string; body: string } | null>(null);
  const [lockoutUntil, setLockoutUntil] = useState<number | null>(null);

  // Lockout countdown tick.
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

  function swapMode(next: Mode) {
    setMode(next);
    setError(null);
    setMagicSent(false);
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (submitting || locked) return;
    setError(null);

    if (mode === 'magic') {
      setSubmitting(true);
      try {
        await authApi.requestMagicLink(email);
        setMagicSent(true);
        setError(null);
      } catch {
        // Show the generic message even on network error — don't leak info.
        setMagicSent(true);
        setError(null);
      } finally {
        setSubmitting(false);
      }
      return;
    }

    setSubmitting(true);
    try {
      await login(email, password);
      // The effect above navigates when `token` flips.
    } catch (err) {
      const ax = err as AxiosError<ApiError>;
      const status = ax.response?.status;

      if (status === 429) {
        const ra = ax.response?.headers?.['retry-after'];
        const secs = typeof ra === 'string' ? parseInt(ra, 10) : NaN;
        if (Number.isFinite(secs) && secs > 0) {
          setLockoutUntil(Date.now() + secs * 1000);
          setError({ title: 'Too many attempts', body: 'The form re-enables when the lockout lifts.' });
        } else {
          setError({
            title: 'Account locked',
            body: ax.response?.data?.error ?? 'Account is permanently locked. Contact support to unlock.',
          });
        }
      } else if (status === 401) {
        setError({ title: 'Those credentials didn\u2019t work', body: 'Check the email and password, or use a magic link.' });
      } else {
        setError({ title: 'Something went wrong', body: ax.response?.data?.error ?? ax.message ?? 'Unknown error' });
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="lg-login grain">
      <button
        className="lg-login__theme"
        aria-label="Toggle theme"
        onClick={(e) => toggle({ x: e.clientX, y: e.clientY })}
      >
        <span className="knob" />
      </button>

      <div className="lg-login__watermark" aria-hidden>L</div>

      <form className="lg-login__card" onSubmit={handleSubmit} noValidate>
        <div className="lg-login__brand">
          <div className="lg-login__wm"><i>Lodgr</i><span className="dot">.</span></div>
          <div className="lg-login__sub">— Self-hosted support desk —</div>
        </div>

        <div className="lg-login__rule" />

        <div className="lg-login__swap" key={mode}>
          {error && (
            <div className="lg-login__error" role="alert">
              <b>{error.title}</b>
              {error.body}
              {locked && <> · retry in {fmtCountdown(lockoutSec)}</>}
            </div>
          )}

          <div className="lg-login__field">
            <div className="lg-login__lab"><span>Email</span></div>
            <input
              className="lg-login__inp"
              type="email"
              autoComplete="username"
              value={email}
              onChange={(ev) => { setEmail(ev.target.value); setMagicSent(false); }}
              placeholder={mode === 'magic' ? 'you@company.com' : 'desk@local'}
              disabled={locked}
              required
            />
            <span className="lg-login__underline" />
          </div>

          {mode === 'password' ? (
            <div className="lg-login__field">
              <div className="lg-login__lab">
                <span>Password</span>
                <button type="button" className="lg-login__forgot" onClick={() => swapMode('magic')}>Forgot</button>
              </div>
              <input
                className="lg-login__inp"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(ev) => setPassword(ev.target.value)}
                disabled={locked}
                required
              />
              <span className="lg-login__underline" />
            </div>
          ) : magicSent ? (
            <div className="lg-login__helper">
              <b>Check your inbox.</b> If this email is registered, a sign-in link has been sent. It expires in <b>15 minutes</b> and works once.
            </div>
          ) : (
            <div className="lg-login__helper">
              We&rsquo;ll email you a one-time sign-in link. It expires in <b>15 minutes</b> and works once.
            </div>
          )}

          <button type="submit" className="lg-login__signin" disabled={submitting || locked || magicSent}>
            {mode === 'password'
              ? (submitting ? 'Signing in…' : 'Sign in')
              : (submitting ? 'Sending…' : magicSent ? 'Link sent' : 'Send the link')}{' '}
            <span className="arr">&rarr;</span>
          </button>
        </div>

        <div className="lg-login__alt">
          {mode === 'password' ? (
            <>Client? <em onClick={() => swapMode('magic')}>Email me a link</em> instead</>
          ) : (
            <>Prefer a <em onClick={() => swapMode('password')}>password</em>?</>
          )}
        </div>
      </form>

      <div className="lg-login__foot">
        <span>v0.1.0 · self-hosted</span>
        <span>Built in <b>Nairobi</b></span>
      </div>

      {/* theme state is read so the toggle re-renders with it */}
      <span hidden>{theme}</span>
    </div>
  );
}
