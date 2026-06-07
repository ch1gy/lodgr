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
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { PasswordGenerator } from '../components/PasswordGenerator';
import { useAuth } from '../auth/AuthContext';
import { auth as authApi } from '../api/auth';
import { admin } from '../api/admin';
import '../styles/v2.css';

type NavItem = 'password' | 'desk-profile' | 'profile' | 'sessions' | 'notifications' | 'danger';

const NAV_ITEMS: Array<{ key: NavItem; label: string; sub: string }> = [
  { key: 'password',      label: '— Password',       sub: 'Change your passphrase' },
  { key: 'desk-profile',  label: '— Desk profile',   sub: 'Invoice "From" info' },
  { key: 'profile',       label: '— Profile',        sub: 'Name and contact email' },
  { key: 'sessions',      label: '— Sessions',       sub: 'Active device list' },
  { key: 'notifications', label: '— Notifications',  sub: 'Email and in-app alerts' },
  { key: 'danger',        label: '— Sign out everywhere', sub: 'Revoke every other session' },
];

export function SettingsPage() {
  const { user, profile } = useAuth();
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
            <PasswordSection userEmail={profile?.email ?? user?.email} />
          )}
          {section === 'desk-profile' && <DeskProfileSection />}
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
  const [showCurrent, setShowCurrent] = useState(false);
  const [showNew, setShowNew]         = useState(false);

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
            <div style={{ position: 'relative' }}>
              <input
                className="lg-f__inp mono"
                type={showCurrent ? 'text' : 'password'}
                placeholder="Your current password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
                autoComplete="current-password"
                style={{ paddingRight: 48 }}
              />
              <button
                type="button"
                onClick={() => setShowCurrent((v) => !v)}
                style={{ position: 'absolute', right: 10, top: '50%', transform: 'translateY(-50%)', background: 'none', border: 'none', cursor: 'pointer', fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', color: 'var(--mid)', textTransform: 'uppercase' }}
                aria-label={showCurrent ? 'Hide password' : 'Show password'}
              >
                {showCurrent ? 'hide' : 'show'}
              </button>
            </div>
            <span className="lg-f__hint">We re-prompt before every password change</span>
          </div>

          <div className="lg-f">
            <div className="lg-f__lbl"><span>New password</span><span className="req">Required · 8–128 chars</span></div>
            <div style={{ position: 'relative' }}>
              <input
                className="lg-f__inp mono"
                type={showNew ? 'text' : 'password'}
                placeholder="Paste from the generator →"
                value={newPw}
                onChange={(e) => setNewPw(e.target.value)}
                autoComplete="new-password"
                maxLength={128}
                style={{ paddingRight: 48 }}
              />
              <button
                type="button"
                onClick={() => setShowNew((v) => !v)}
                style={{ position: 'absolute', right: 10, top: '50%', transform: 'translateY(-50%)', background: 'none', border: 'none', cursor: 'pointer', fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', color: 'var(--mid)', textTransform: 'uppercase' }}
                aria-label={showNew ? 'Hide password' : 'Show password'}
              >
                {showNew ? 'hide' : 'show'}
              </button>
            </div>
            <span className="lg-f__hint">Hashed server-side · never logged</span>
          </div>

          <div className="lg-f">
            <div className="lg-f__lbl"><span>Confirm new password</span></div>
            <input
              className="lg-f__inp mono"
              type={showNew ? 'text' : 'password'}
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
  const [confirming, setConfirming] = useState(false);
  return (
    <>
      <div className="lg-set__sec-eye">— Sign out</div>
      <h2 className="lg-set__h2">Sign out <em>of this device.</em></h2>
      <div className="lg-set__dek">
        Revokes the refresh token for this browser session. You will be taken to the
        login screen. Other active sessions on other devices are not affected.
      </div>
      {confirming ? (
        <div style={{ display: 'flex', gap: 12, alignItems: 'center', marginTop: 8 }}>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--mid)' }}>
            Sign out of this device?
          </span>
          <button type="button" className="lg-bt lg-bt--danger" onClick={() => void logout()}>
            Yes, sign out ✕
          </button>
          <button type="button" className="lg-bt lg-bt--text" onClick={() => setConfirming(false)}>
            Cancel
          </button>
        </div>
      ) : (
        <button
          type="button"
          className="lg-bt lg-bt--danger"
          onClick={() => setConfirming(true)}
        >
          Sign out ✕
        </button>
      )}
    </>
  );
}

// ── Desk profile section ─────────────────────────────────────────────────────
function DeskProfileSection() {
  const qc = useQueryClient();
  const profileQ = useQuery({
    queryKey: ['desk-profile'],
    queryFn: () => admin.getDeskProfile(),
  });

  const [name, setName]           = useState('');
  const [tagline, setTagline]     = useState('');
  const [email, setEmail]         = useState('');
  const [website, setWebsite]     = useState('');
  const [city, setCity]           = useState('');
  const [phone, setPhone]         = useState('');
  const [vatNumber, setVatNumber] = useState('');
  const [seeded, setSeeded]       = useState(false);
  const [saved, setSaved]         = useState(false);

  // Seed form once data loads.
  if (profileQ.data && !seeded) {
    const p = profileQ.data;
    setName(p.name); setTagline(p.tagline); setEmail(p.email);
    setWebsite(p.website); setCity(p.city); setPhone(p.phone); setVatNumber(p.vat_number);
    setSeeded(true);
  }

  const updateM = useMutation({
    mutationFn: () => admin.updateDeskProfile({ name, tagline, email, website, city, phone, vat_number: vatNumber }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['desk-profile'] });
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    },
  });

  const err = updateM.error
    ? ((updateM.error as { response?: { data?: { error?: string } } }).response?.data?.error ?? 'Save failed.')
    : null;

  return (
    <>
      <div className="lg-set__sec-eye">— Section 02 · Desk profile</div>
      <h2 className="lg-set__h2">Invoice <em>"From"</em> info.</h2>
      <div className="lg-set__dek">
        This information appears in the "From" section of every printed invoice.
        It is never shown to clients inside the app.
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '22px 32px', maxWidth: 640, marginTop: 8 }}>
        <div className="lg-f" style={{ gridColumn: '1 / -1' }}>
          <div className="lg-f__lbl"><span>Display name</span></div>
          <input className="lg-f__inp" value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Acme Studio" />
        </div>
        <div className="lg-f" style={{ gridColumn: '1 / -1' }}>
          <div className="lg-f__lbl"><span>Tagline / role</span></div>
          <input className="lg-f__inp" value={tagline} onChange={(e) => setTagline(e.target.value)} placeholder="e.g. Independent · Software & Web" />
        </div>
        <div className="lg-f">
          <div className="lg-f__lbl"><span>Email</span></div>
          <input className="lg-f__inp" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="hello@yourdomain.com" />
        </div>
        <div className="lg-f">
          <div className="lg-f__lbl"><span>Website</span></div>
          <input className="lg-f__inp" value={website} onChange={(e) => setWebsite(e.target.value)} placeholder="yourdomain.com" />
        </div>
        <div className="lg-f">
          <div className="lg-f__lbl"><span>City</span></div>
          <input className="lg-f__inp" value={city} onChange={(e) => setCity(e.target.value)} placeholder="e.g. Nairobi" />
        </div>
        <div className="lg-f">
          <div className="lg-f__lbl"><span>Phone</span></div>
          <input className="lg-f__inp" value={phone} onChange={(e) => setPhone(e.target.value)} placeholder="+254 700 000 000" />
        </div>
        <div className="lg-f">
          <div className="lg-f__lbl"><span>VAT / KRA PIN</span></div>
          <input className="lg-f__inp mono" value={vatNumber} onChange={(e) => setVatNumber(e.target.value)} placeholder="e.g. P051234567X" />
        </div>
      </div>

      {err && <div className="lg-f__err" style={{ marginTop: 12 }}>{err}</div>}
      {saved && <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--green, #4f6f4a)', marginTop: 12 }}>✓ Saved</div>}

      <div style={{ marginTop: 24 }}>
        <button type="button" className="lg-bt lg-bt--solid" onClick={() => updateM.mutate()} disabled={updateM.isPending}>
          {updateM.isPending ? 'Saving…' : 'Save profile'} <span className="arr">↗</span>
        </button>
      </div>
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
