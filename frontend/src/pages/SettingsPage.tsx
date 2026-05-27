// ─────────────────────────────────────────────────────────────────────────────
// SettingsPage.tsx — /settings
//
// Desk: full password-change form with the PasswordGenerator on the right.
// Client: placeholder (self-serve password change is planned, see PLANNED.md).
//
// PATCH /auth/password already validates current password and returns a fresh
// access token, which the auth API wires into tokenStore automatically.
// ─────────────────────────────────────────────────────────────────────────────

import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { PasswordGenerator } from '../components/PasswordGenerator';
import { useAuth } from '../auth/AuthContext';
import { auth as authApi } from '../api/auth';
import '../styles/v2.css';

type NavItem = 'password' | 'profile' | 'sessions' | 'notifications' | 'danger';

const NAV_ITEMS: Array<{ key: NavItem; label: string; sub: string }> = [
  { key: 'password',      label: '— Password',       sub: 'Change your passphrase' },
  { key: 'profile',       label: '— Profile',        sub: 'Name and contact email' },
  { key: 'sessions',      label: '— Sessions',       sub: 'Active device list' },
  { key: 'notifications', label: '— Notifications',  sub: 'Email and in-app alerts' },
  { key: 'danger',        label: '— Sign out everywhere', sub: 'Revoke every other session' },
];

export function SettingsPage() {
  const { user, isDesk } = useAuth();
  const [section, setSection] = useState<NavItem>('password');

  return (
    <div className="lg-v2">
      <Masthead active="settings" />
      <div className="lg-set">
        {/* ── Nav ─────────────────────────────────────────────────── */}
        <nav className="lg-set__nav">
          <div className="lg-set__eye">— The desk · settings</div>
          <div className="lg-set__h1">Your<br /><i>account.</i></div>
          {NAV_ITEMS.map((item) => (
            <button
              key={item.key}
              type="button"
              className={`lg-set__item${section === item.key ? ' on' : ''}`}
              onClick={() => setSection(item.key)}
              style={{ background: 'none', border: 'none', textAlign: 'left', width: '100%' }}
            >
              <span className="k" style={item.key === 'danger' ? { color: 'var(--red)' } : undefined}>
                {item.label}
              </span>
              <span className="v">{item.sub}</span>
            </button>
          ))}
        </nav>

        {/* ── Body ────────────────────────────────────────────────── */}
        <div className="lg-set__body">
          {section === 'password' && (
            isDesk
              ? <PasswordSection userEmail={user?.email} />
              : <PlaceholderSection title="Password" note="Client self-serve password change is coming in a future update. Use the magic link your desk sends you to sign in." />
          )}
          {section === 'profile' && (
            <PlaceholderSection title="Profile" note="Profile editing is coming soon. Contact the desk to update your name or email." />
          )}
          {section === 'sessions' && (
            <PlaceholderSection title="Sessions" note="Active session management is coming soon." />
          )}
          {section === 'notifications' && (
            <PlaceholderSection title="Notifications" note="Notification preferences are coming soon." />
          )}
          {section === 'danger' && <DangerSection />}
        </div>
      </div>
      <BottomTabBar active="settings" />
    </div>
  );
}

// ── Password section (desk only) ─────────────────────────────────────────────
function PasswordSection({ userEmail }: { userEmail?: string }) {
  const [current, setCurrent]   = useState('');
  const [newPw, setNewPw]       = useState('');
  const [confirm, setConfirm]   = useState('');
  const [done, setDone]         = useState(false);

  const changeM = useMutation({
    mutationFn: () => authApi.changePassword(current, newPw),
    onSuccess: () => {
      setDone(true);
      setCurrent('');
      setNewPw('');
      setConfirm('');
    },
  });

  const mismatch  = confirm.length > 0 && newPw !== confirm;
  const canSubmit = current.length > 0 && newPw.length >= 8 && newPw === confirm && !changeM.isPending;

  const err = changeM.error
    ? ((changeM.error as { response?: { data?: { error?: string } } })
        .response?.data?.error ?? 'Password change failed. Check your current password.')
    : null;

  return (
    <>
      <div className="lg-set__sec-eye">— Section 01 · Password</div>
      <h2 className="lg-set__h2">A new <em>passphrase.</em></h2>
      <div className="lg-set__dek">
        Eight to one-hundred-twenty-eight characters. Generate something strong on the right —
        it's stored as an argon2id hash server-side. You'll receive a fresh access token
        immediately after submitting. No re-login.
      </div>

      <div className="lg-set__split">
        {/* Left: form */}
        <div className="lg-set__pwblock">
          {done && (
            <div style={{ fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '.14em', textTransform: 'uppercase', color: 'var(--green, #4f6f4a)', padding: '10px 0', borderBottom: '1px solid var(--rule)', marginBottom: 8 }}>
              ✓ Password updated — new token issued
            </div>
          )}

          <div className="lg-f">
            <div className="lg-f__lbl"><span>Current password</span><span className="req">Required</span></div>
            <input
              className="lg-f__inp mono"
              type="password"
              placeholder="Your current password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
              autoComplete="current-password"
            />
            <span className="lg-f__hint">We re-prompt before every password change</span>
          </div>

          <div className="lg-f">
            <div className="lg-f__lbl"><span>New password</span><span className="req">Required · 8–128 chars</span></div>
            <input
              className="lg-f__inp mono"
              type="text"
              placeholder="Paste from the generator →"
              value={newPw}
              onChange={(e) => setNewPw(e.target.value)}
              autoComplete="new-password"
              maxLength={128}
            />
            <span className="lg-f__hint">Plain text — never logged, hashed server-side</span>
          </div>

          <div className="lg-f">
            <div className="lg-f__lbl"><span>Confirm new password</span></div>
            <input
              className="lg-f__inp mono"
              type="text"
              placeholder="Repeat the new password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              maxLength={128}
            />
            {mismatch && <span className="lg-f__err">Passwords don't match</span>}
            {!mismatch && confirm.length > 0 && newPw === confirm && (
              <span className="lg-f__hint" style={{ color: 'var(--green, #4f6f4a)' }}>✓ Matches</span>
            )}
          </div>

          {err && <div className="lg-f__err">{err}</div>}

          <div className="lg-set__row">
            <div className="left">
              Signed in as <b>{userEmail ?? '—'}</b><br />
              Hash <b>argon2id · m=64 MiB · t=3</b>
            </div>
            <button
              type="button"
              className="lg-bt lg-bt--solid"
              disabled={!canSubmit}
              onClick={() => changeM.mutate()}
            >
              {changeM.isPending ? 'Updating…' : 'Update password'} <span className="arr">↗</span>
            </button>
          </div>
        </div>

        {/* Right: generator */}
        <div>
          <PasswordGenerator
            mode="passphrase"
            onUse={(pw) => { setNewPw(pw); setConfirm(pw); }}
          />
        </div>
      </div>
    </>
  );
}

// ── Danger section ────────────────────────────────────────────────────────────
function DangerSection() {
  const { logout } = useAuth();
  return (
    <>
      <div className="lg-set__sec-eye">— Sign out everywhere</div>
      <h2 className="lg-set__h2">Sign out <em>everywhere.</em></h2>
      <div className="lg-set__dek">
        Revoke every active session on every device. You'll be signed out here too
        and taken to the login screen.
      </div>
      <button
        type="button"
        className="lg-bt lg-bt--danger"
        onClick={() => { if (confirm('Revoke all sessions and sign out everywhere?')) void logout(); }}
      >
        Sign out everywhere ✕
      </button>
    </>
  );
}

// ── Stub for unimplemented sections ──────────────────────────────────────────
function PlaceholderSection({ title, note }: { title: string; note: string }) {
  return (
    <>
      <div className="lg-set__sec-eye">— {title}</div>
      <h2 className="lg-set__h2"><em>{title}.</em></h2>
      <div className="lg-set__dek">{note}</div>
    </>
  );
}
