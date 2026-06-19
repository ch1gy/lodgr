// ─────────────────────────────────────────────────────────────────────────────
// ClientsPage.tsx — /clients (desk only)
//
// Full CRUD admin panel for managing clients. Status is derived client-side:
//   • deleted_at != null  → 'archived'
//   • locked_until != null → 'locked'
//   • else                → 'active'
//
// Actions by status:
//   Active / Locked  — Magic link, Unlock (if locked), Revoke sessions,
//                       Export + download, Archive
//   Archived         — Restore, Export + download, Hard delete
// ─────────────────────────────────────────────────────────────────────────────

import { useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Masthead } from '../components/Masthead';
import { PasswordGenerator } from '../components/PasswordGenerator';
import { MagicLinkModal } from '../components/MagicLinkModal';
import { ConfirmModal } from '../components/ConfirmModal';
import type { ConfirmOptions } from '../components/ConfirmModal';
import { Dropdown } from '../components/Dropdown';
import type { DropdownItem } from '../components/Dropdown';
import { admin } from '../api/admin';
import { tickets as ticketsApi } from '../api/tickets';
import { downloadBlob, extractApiError } from '../utils/format';
import type { Client, SubClient } from '../api/types';
import '../styles/v2.css';

type ClientStatus = 'active' | 'locked' | 'archived';
type FilterTab = 'all' | ClientStatus;

function clientStatus(c: Client): ClientStatus {
  if (c.deleted_at != null) return 'archived';
  if (c.locked_until != null) return 'locked';
  return 'active';
}

function clientInitials(name: string): string {
  const parts = name.trim().split(/\s+/);
  return parts.slice(0, 2).map((w) => w[0] ?? '').join('').toUpperCase() || '??';
}



// ── Sub-clients panel ─────────────────────────────────────────────────────────
function SubClientsPanel({ clientId, enabled }: { clientId: string; enabled: boolean }) {
  const qc = useQueryClient();
  const [newName, setNewName] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  const scQ = useQuery({
    queryKey: ['sub-clients', clientId],
    queryFn: () => admin.listSubClients(clientId),
    enabled,
  });
  const subClients: SubClient[] = scQ.data ?? [];

  const createM = useMutation({
    mutationFn: (name: string) => admin.createSubClient(clientId, name),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['sub-clients', clientId] });
      setNewName('');
    },
  });

  const deleteM = useMutation({
    mutationFn: (id: string) => admin.deleteSubClient(id),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['sub-clients', clientId] }),
  });

  function handleAdd() {
    const n = newName.trim();
    if (!n) return;
    createM.mutate(n);
  }

  return (
    <div style={{ fontFamily: 'var(--mono)', fontSize: 10 }}>
      <div className="lg-cl-expand__section-label">End clients</div>
      {scQ.isLoading && (
        <div style={{ color: 'var(--mid)', fontStyle: 'italic', marginBottom: 8 }}>Loading…</div>
      )}
      {!scQ.isLoading && subClients.length === 0 && (
        <div style={{ color: 'var(--mid)', fontStyle: 'italic', marginBottom: 8 }}>None yet</div>
      )}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 10 }}>
        {subClients.map((sc) => (
          <span key={sc.id} style={{
            display: 'inline-flex', alignItems: 'center', gap: 5,
            border: '1px solid var(--rule)', padding: '2px 8px', color: 'var(--ink)',
          }}>
            {sc.name}
            <button
              type="button"
              onClick={() => deleteM.mutate(sc.id)}
              disabled={deleteM.isPending}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--mid)', fontSize: 10, padding: 0, lineHeight: 1 }}
              aria-label={`Remove ${sc.name}`}
            >✕</button>
          </span>
        ))}
      </div>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <input
          ref={inputRef}
          className="lg-cl-expand__inp"
          style={{ width: 160 }}
          placeholder="Add end client…"
          value={newName}
          maxLength={120}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') handleAdd(); }}
        />
        <button
          type="button"
          onClick={handleAdd}
          disabled={!newName.trim() || createM.isPending}
          style={{
            fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em',
            textTransform: 'uppercase', background: 'none',
            border: '1px solid var(--rule)', padding: '3px 10px', cursor: 'pointer',
            color: 'var(--ink)',
          }}
        >+ Add</button>
      </div>
    </div>
  );
}

// ── Per-row ───────────────────────────────────────────────────────────────────
interface RowProps {
  client: Client;
  onAction: (action: string, id: string) => void;
  disabled?: boolean;
  counts?: { all: number; open: number };
}

function ClientRow({ client, onAction, disabled = false, counts }: RowProps) {
  const qc         = useQueryClient();
  const [expanded, setExpanded] = useState(false);
  const [profile, setProfile]   = useState({
    name:           client.name,
    contact_person: client.contact_person ?? '',
    pin_number:     client.pin_number     ?? '',
    address_line1:  client.address_line1  ?? '',
    address_line2:  client.address_line2  ?? '',
    phone:          client.phone          ?? '',
  });
  const [saving, setSaving] = useState(false);

  const status     = clientStatus(client);
  const isArchived = status === 'archived';
  const isLocked   = status === 'locked';
  const canExpand  = !isArchived;

  const metaLine = isLocked
    ? `Locked · ${client.failed_attempts} failed attempt${client.failed_attempts !== 1 ? 's' : ''}`
    : isArchived
    ? `Archived ${client.deleted_at?.slice(0, 10) ?? ''}`
    : client.failed_attempts > 0
    ? `${client.failed_attempts} failed attempt${client.failed_attempts !== 1 ? 's' : ''}`
    : `id · ${client.id.slice(0, 8)}`;

  async function saveProfile() {
    setSaving(true);
    try {
      await admin.updateClient(client.id, {
        name:           profile.name.trim()           || undefined,
        contact_person: profile.contact_person.trim() || undefined,
        pin_number:     profile.pin_number.trim()     || undefined,
        address_line1:  profile.address_line1.trim()  || undefined,
        address_line2:  profile.address_line2.trim()  || undefined,
        phone:          profile.phone.trim()          || undefined,
      });
      void qc.invalidateQueries({ queryKey: ['clients'] });
    } finally {
      setSaving(false);
    }
  }

  return (
    <div>
      <div
        className={[
          'lg-cl-row',
          isArchived ? 'deleted' : isLocked ? 'locked' : '',
          canExpand ? 'expandable' : '',
          expanded ? 'expanded' : '',
        ].filter(Boolean).join(' ')}
        onClick={() => canExpand && setExpanded((v) => !v)}
      >
        <div className="av">{clientInitials(client.name)}</div>

        <div className="name-blk">
          <div className="nm">
            {client.name}
            {canExpand && <span className="expand-arrow">▶</span>}
          </div>
          <div className="em">{client.email}</div>
        </div>

        <div className="meta-blk">
          <b>{client.id.slice(0, 8)}</b><br />
          {metaLine}
        </div>

        <div className="stat-blks">
          <div className="stat-blk"><div className="v">{String(counts?.all ?? 0).padStart(2, '0')}</div><div className="lbl">all-time</div></div>
          <div className="stat-blk"><div className="v red">{String(counts?.open ?? 0).padStart(2, '0')}</div><div className="lbl">open</div></div>
          <div className="stat-blk"><div className="v">{isLocked ? '⚠' : client.deleted_at ? '◌' : '✓'}</div><div className="lbl">{isLocked ? 'locked' : client.deleted_at ? 'archived' : 'active'}</div></div>
        </div>

        <div className={`status-blk ${status}`}>
          <span className="dot" />{status}
        </div>

        <div className="acts" onClick={(e) => e.stopPropagation()}>
          {isArchived ? (
            <>
              <button type="button" className="a primary" disabled={disabled} onClick={() => onAction('restore', client.id)}>Restore ⟲</button>
              <Dropdown
                align="right"
                trigger={<button type="button" className="a" disabled={disabled}>Actions ▾</button>}
                items={[
                  { label: 'Export ↓', onClick: () => onAction('export', client.id), disabled },
                  { label: 'Hard delete ✕', onClick: () => onAction('hard-delete', client.id), danger: true, disabled },
                ] satisfies DropdownItem[]}
              />
            </>
          ) : (
            <>
              <button type="button" className="a primary" disabled={disabled} onClick={() => onAction('magic-link', client.id)}>Magic link ↗</button>
              <Dropdown
                align="right"
                trigger={<button type="button" className="a" disabled={disabled}>Actions ▾</button>}
                items={[
                  ...(isLocked ? [{ label: 'Unlock ⊙', onClick: () => onAction('unlock', client.id), disabled }] : []),
                  { label: 'Revoke sessions', onClick: () => onAction('revoke', client.id), disabled },
                  { label: 'Export ↓', onClick: () => onAction('export', client.id), disabled },
                  { label: 'Archive ⌫', onClick: () => onAction('archive', client.id), danger: true, disabled },
                ] satisfies DropdownItem[]}
              />
            </>
          )}
        </div>
      </div>

      {canExpand && (
        <div className={`lg-cl-expand-wrap${expanded ? ' open' : ''}`}>
          <div>
            <div className="lg-cl-expand">
              {/* Billing profile */}
              <div>
                <div className="lg-cl-expand__section-label">Billing profile</div>
                <div className="lg-cl-expand__profile-grid">
                  <div style={{ gridColumn: 'span 2' }}>
                    <div className="lg-cl-expand__field-label">Company name</div>
                    <input className="lg-cl-expand__inp" value={profile.name} placeholder="e.g. Bahari Property Co."
                      onChange={(e) => setProfile((p) => ({ ...p, name: e.target.value }))} />
                  </div>
                  <div>
                    <div className="lg-cl-expand__field-label">Contact person</div>
                    <input className="lg-cl-expand__inp" value={profile.contact_person} placeholder="e.g. Jane Doe"
                      onChange={(e) => setProfile((p) => ({ ...p, contact_person: e.target.value }))} />
                  </div>
                  <div>
                    <div className="lg-cl-expand__field-label">PIN / KRA No.</div>
                    <input className="lg-cl-expand__inp" value={profile.pin_number} placeholder="e.g. P051234567X"
                      onChange={(e) => setProfile((p) => ({ ...p, pin_number: e.target.value }))} />
                  </div>
                  <div>
                    <div className="lg-cl-expand__field-label">Address line 1</div>
                    <input className="lg-cl-expand__inp" value={profile.address_line1} placeholder="Street address"
                      onChange={(e) => setProfile((p) => ({ ...p, address_line1: e.target.value }))} />
                  </div>
                  <div>
                    <div className="lg-cl-expand__field-label">Address line 2</div>
                    <input className="lg-cl-expand__inp" value={profile.address_line2} placeholder="City, Country"
                      onChange={(e) => setProfile((p) => ({ ...p, address_line2: e.target.value }))} />
                  </div>
                  <div>
                    <div className="lg-cl-expand__field-label">Phone</div>
                    <input className="lg-cl-expand__inp" value={profile.phone} placeholder="+254 700 000 000"
                      onChange={(e) => setProfile((p) => ({ ...p, phone: e.target.value }))} />
                  </div>
                  <div className="lg-cl-expand__save">
                    <button type="button" onClick={saveProfile} disabled={saving}>
                      {saving ? 'Saving…' : 'Save profile'}
                    </button>
                  </div>
                </div>
              </div>

              {/* Sub-clients */}
              <SubClientsPanel clientId={client.id} enabled={expanded} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── New client modal ──────────────────────────────────────────────────────────
interface NewClientModalProps {
  onClose: () => void;
  onCreated: () => void;
}

function NewClientModal({ onClose, onCreated }: NewClientModalProps) {
  const [name, setName]         = useState('');
  const [email, setEmail]       = useState('');
  const [password, setPassword] = useState('');
  const [profileOpen, setProfileOpen] = useState(false);
  const [profile, setProfile] = useState({
    contact_person: '',
    pin_number:     '',
    address_line1:  '',
    address_line2:  '',
    phone:          '',
  });

  const createM = useMutation({
    mutationFn: () => admin.createClient({
      name: name.trim(),
      email: email.trim(),
      password,
      contact_person: profile.contact_person.trim() || undefined,
      pin_number:     profile.pin_number.trim()     || undefined,
      address_line1:  profile.address_line1.trim()  || undefined,
      address_line2:  profile.address_line2.trim()  || undefined,
      phone:          profile.phone.trim()          || undefined,
    }),
    onSuccess: () => {
      onCreated();
      onClose();
    },
  });

  const err = createM.error
    ? extractApiError(createM.error, 'Failed to create client. Try again.')
    : null;

  const canSubmit =
    name.trim().length > 0 &&
    email.trim().includes('@') &&
    password.length >= 8 &&
    !createM.isPending;

  const monoInput: React.CSSProperties = {
    fontFamily: 'var(--mono)', fontSize: 10, background: 'none',
    border: 'none', borderBottom: '1px solid var(--rule)', outline: 'none',
    padding: '3px 0', color: 'var(--ink)', width: '100%',
  };

  return (
    <div className="lg-ov" role="dialog" aria-modal aria-label="Admit a new client">
      <div className="lg-mdl create" style={{ width: 'min(980px, 90vw)' }}>
        {/* ── Header ──────────────────────────────────────────────── */}
        <div className="lg-mdl__top">
          <span className="lg-mdl__eye">— Section 02 — Admit a new client</span>
          <span className="lg-mdl__no"><i>Lodgr</i><span className="dot">.</span></span>
          <button type="button" className="lg-mdl__x" onClick={onClose}>Close ✕</button>
        </div>

        {/* ── Body ────────────────────────────────────────────────── */}
        <div className="lg-mdl__body">
          <div className="lg-mdl__h1">A new name <em>on the roster.</em></div>
          <div className="lg-mdl__dek">
            Pick a strong password using the generator. Copy it before submitting —
            share it with the client out-of-band (SMS, video call, etc.).
            Only the argon2id hash is stored server-side, never the plaintext.
          </div>
          <div className="lg-mdl__rule" />

          <div className="lg-mdl-create-grid">
            {/* Left — form */}
            <div className="lg-f-grid">
              <div className="lg-f full">
                <div className="lg-f__lbl"><span>Client name</span><span className="req">Required</span></div>
                <input
                  className="lg-f__inp"
                  placeholder="e.g. Bahari Property Co."
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  maxLength={120}
                  autoFocus
                />
              </div>
              <div className="lg-f full">
                <div className="lg-f__lbl"><span>Contact email</span><span className="req">Required</span></div>
                <input
                  className="lg-f__inp mono"
                  type="email"
                  placeholder="office@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  maxLength={320}
                />
                <span className="lg-f__hint">Used for sign-in · magic-link recovery coming soon</span>
              </div>
              <div className="lg-f full">
                <div className="lg-f__lbl">
                  <span>Initial password</span>
                  <span className="req">Required · 8–128 chars</span>
                </div>
                <input
                  className="lg-f__inp mono"
                  type="text"
                  placeholder="Paste from the generator →"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  maxLength={128}
                  autoComplete="new-password"
                />
                <span className="lg-f__hint">Plain text — never logged, hashed argon2id server-side</span>
              </div>

              {/* ── Optional billing profile ─────────────────────── */}
              <div className="lg-f full" style={{ marginTop: 4 }}>
                <button
                  type="button"
                  onClick={() => setProfileOpen((v) => !v)}
                  style={{
                    background: 'none', border: 'none', cursor: 'pointer',
                    color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 10,
                    letterSpacing: '.14em', textTransform: 'uppercase', padding: 0,
                  }}
                >
                  {profileOpen ? '▾' : '▸'} Billing profile <span style={{ color: 'var(--mid)', fontStyle: 'italic', textTransform: 'none', letterSpacing: 0 }}>(optional)</span>
                </button>
                {profileOpen && (
                  <div style={{ marginTop: 10, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px 20px' }}>
                    <div>
                      <div style={{ color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase', marginBottom: 4 }}>Contact person</div>
                      <input style={monoInput} value={profile.contact_person} placeholder="e.g. Jane Doe"
                        onChange={(e) => setProfile((p) => ({ ...p, contact_person: e.target.value }))} />
                    </div>
                    <div>
                      <div style={{ color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase', marginBottom: 4 }}>PIN / KRA No.</div>
                      <input style={monoInput} value={profile.pin_number} placeholder="e.g. P051234567X"
                        onChange={(e) => setProfile((p) => ({ ...p, pin_number: e.target.value }))} />
                    </div>
                    <div>
                      <div style={{ color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase', marginBottom: 4 }}>Address line 1</div>
                      <input style={monoInput} value={profile.address_line1} placeholder="Street address"
                        onChange={(e) => setProfile((p) => ({ ...p, address_line1: e.target.value }))} />
                    </div>
                    <div>
                      <div style={{ color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase', marginBottom: 4 }}>Address line 2</div>
                      <input style={monoInput} value={profile.address_line2} placeholder="City, Country"
                        onChange={(e) => setProfile((p) => ({ ...p, address_line2: e.target.value }))} />
                    </div>
                    <div>
                      <div style={{ color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase', marginBottom: 4 }}>Phone</div>
                      <input style={monoInput} value={profile.phone} placeholder="+254 700 000 000"
                        onChange={(e) => setProfile((p) => ({ ...p, phone: e.target.value }))} />
                    </div>
                  </div>
                )}
              </div>

              {err && <div className="lg-f__err">{err}</div>}
            </div>

            {/* Right — generator */}
            <PasswordGenerator onUse={(pw) => setPassword(pw)} />
          </div>
        </div>

        {/* ── Footer ──────────────────────────────────────────────── */}
        <div className="lg-mdl__foot">
          <span className="meta">Argon2id · m=64 MiB · t=3 · password hashed server-side, never logged</span>
          <div className="lg-mdl__btns">
            <button type="button" className="lg-bt lg-bt--text" onClick={onClose}>Cancel</button>
            <button
              type="button"
              className="lg-bt lg-bt--solid"
              disabled={!canSubmit}
              onClick={() => createM.mutate()}
            >
              {createM.isPending ? 'Admitting…' : 'Admit client'} <span className="arr">↗</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────
export function ClientsPage() {
  const qc = useQueryClient();
  const [filter, setFilter]         = useState<FilterTab>('all');
  const [search, setSearch]         = useState('');
  const [newClientOpen, setNewClientOpen] = useState(false);
  const [magicLink, setMagicLink]   = useState<{ url: string; clientId: string } | null>(null);
  const [confirmOpts, setConfirmOpts] = useState<(ConfirmOptions & { onConfirm: () => void }) | null>(null);

  function showConfirm(opts: ConfirmOptions, onConfirm: () => void) {
    setConfirmOpts({ ...opts, onConfirm });
  }

  const clientsQ = useQuery({
    queryKey: ['clients'],
    queryFn: () => admin.listClients(),
  });

  const ticketsQ = useQuery({
    queryKey: ['tickets', 'all-for-counts'],
    queryFn: () => ticketsApi.list(1, 500),
    staleTime: 5 * 60_000,
  });

  const countsByClient = useMemo(() => {
    const m: Record<string, { all: number; open: number }> = {};
    for (const t of ticketsQ.data?.tickets ?? []) {
      const c = (m[t.client_id] ??= { all: 0, open: 0 });
      c.all++;
      if (t.status === 'open' || t.status === 'acknowledged' || t.status === 'pending') c.open++;
    }
    return m;
  }, [ticketsQ.data]);

  const allClients = useMemo(() => clientsQ.data ?? [], [clientsQ.data]);

  const counts = useMemo(() => {
    const c = { all: 0, active: 0, locked: 0, archived: 0 };
    for (const client of allClients) {
      c.all++;
      c[clientStatus(client)]++;
    }
    return c;
  }, [allClients]);

  const visible = useMemo(() => {
    let list = allClients;
    if (filter !== 'all') list = list.filter((c) => clientStatus(c) === filter);
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter((c) =>
        c.name.toLowerCase().includes(q) || c.email.toLowerCase().includes(q)
      );
    }
    return list;
  }, [allClients, filter, search]);

  // ── Shared invalidation ───────────────────────────────────────────────
  const invalidate = () => void qc.invalidateQueries({ queryKey: ['clients'] });

  // ── Mutations ─────────────────────────────────────────────────────────
  const unlockM  = useMutation({ mutationFn: admin.unlockClient,          onSuccess: invalidate });
  const revokeM  = useMutation({ mutationFn: admin.deleteClientSessions,  onSuccess: invalidate });
  const archiveM = useMutation({ mutationFn: admin.softDeleteClient,      onSuccess: invalidate });
  const restoreM = useMutation({ mutationFn: admin.restoreClient,         onSuccess: invalidate });
  const hardDelM = useMutation({
    mutationFn: ({ id, email }: { id: string; email: string }) =>
      admin.hardDeleteClient(id, `permanently delete ${email}`),
    onSuccess: invalidate,
  });
  const exportM  = useMutation({
    mutationFn: async (id: string) => {
      const res  = await admin.exportClient(id);
      const blob = await admin.downloadExport(res.download_url);
      downloadBlob(blob, `lodgr-export-${id.slice(0, 8)}.json`);
    },
  });
  const magicM   = useMutation({
    mutationFn: admin.createMagicLink,
    onSuccess: (data, id) => {
      setMagicLink({ url: data.url, clientId: id });
    },
  });

  const anyPending =
    unlockM.isPending || revokeM.isPending || archiveM.isPending ||
    restoreM.isPending || hardDelM.isPending || exportM.isPending || magicM.isPending;

  function handleAction(action: string, id: string) {
    const client = allClients.find((c) => c.id === id);
    switch (action) {
      case 'unlock':
        unlockM.mutate(id);
        break;
      case 'revoke':
        showConfirm(
          { title: 'Revoke sessions?', body: 'All active sessions for this client will be invalidated. They will need to sign in again.', confirmLabel: 'Revoke', danger: false },
          () => { revokeM.mutate(id); setConfirmOpts(null); }
        );
        break;
      case 'archive':
        showConfirm(
          { title: 'Archive this client?', body: 'They will no longer be able to sign in. You can restore them within 30 days.', confirmLabel: 'Archive', danger: false },
          () => { archiveM.mutate(id); setConfirmOpts(null); }
        );
        break;
      case 'restore':
        restoreM.mutate(id);
        break;
      case 'export':
        exportM.mutate(id);
        break;
      case 'magic-link':
        showConfirm(
          {
            title: 'Generate full-session link?',
            body: 'Anyone who opens this link is signed in as this client for 24h. Only share it with the client themselves.',
            confirmLabel: 'Generate link',
            danger: false,
          },
          () => { magicM.mutate(id); setConfirmOpts(null); }
        );
        break;
      case 'hard-delete': {
        const email = client?.email ?? '';
        showConfirm(
          {
            title: 'Permanently delete?',
            body: `All data for ${email || id.slice(0, 8)} will be erased and cannot be recovered. An export must have been created first.`,
            confirmLabel: 'Delete permanently',
            danger: true,
          },
          () => { hardDelM.mutate({ id, email }); setConfirmOpts(null); }
        );
        break;
      }
    }
  }

  return (
    <div className="lg-v2">
      <Masthead active="clients" />

      {/* ── KPI header ──────────────────────────────────────────────── */}
      <div className="lg-cl-head grain">
        <div>
          <div className="lg-cl-eye">— Roster · {String(counts.all).padStart(2, '0')}</div>
          <h1>The roster<span className="count">/ {String(counts.all).padStart(2, '0')}</span></h1>
          <div className="sub">
            {counts.active} active · {counts.locked} locked · {counts.archived} archived
          </div>
        </div>
        <div className="right lg-cl-kpis">
          <div className="kpi">
            <div className="v">{String(counts.active).padStart(2, '0')}</div>
            <div className="lbl">Active</div>
          </div>
          <div className="kpi">
            <div className={`v${counts.locked > 0 ? ' amber' : ''}`}>
              {String(counts.locked).padStart(2, '0')}
            </div>
            <div className="lbl">Locked</div>
          </div>
          <div className="kpi">
            <div className="v">{String(counts.archived).padStart(2, '0')}</div>
            <div className="lbl">Archived</div>
          </div>
        </div>
      </div>

      {/* ── Filter bar ──────────────────────────────────────────────── */}
      <div className="lg-cl-filt">
        <div className="group">
          {(['all', 'active', 'locked', 'archived'] as FilterTab[]).map((f) => (
            <button
              key={f}
              type="button"
              className={`tab${filter === f ? ' on' : ''}`}
              onClick={() => setFilter(f)}
            >
              {f[0].toUpperCase() + f.slice(1)} <b>{counts[f]}</b>
            </button>
          ))}
        </div>
        <div className="spacer" />
        <input
          style={{
            fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '.12em',
            background: 'none', border: 'none',
            borderBottom: '1px dashed var(--rule)', outline: 'none',
            padding: '4px 0', color: 'var(--mid)', width: 180,
          }}
          placeholder="/ search clients"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label="Search clients"
        />
        <button
          type="button"
          className="new"
          style={{ marginLeft: 24 }}
          onClick={() => setNewClientOpen(true)}
        >
          + New client
        </button>
      </div>

      {/* ── Client rows ─────────────────────────────────────────────── */}
      <div className="lg-cl-rows">
        {clientsQ.isLoading && (
          <div style={{
            padding: '48px 40px', fontFamily: 'var(--mono)', fontSize: 11,
            letterSpacing: '.14em', textTransform: 'uppercase', color: 'var(--mid)',
          }}>
            — Loading clients —
          </div>
        )}
        {clientsQ.isError && (
          <div style={{ padding: '48px 40px', fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--red)' }}>
            Failed to load clients. Try refreshing.
          </div>
        )}
        {!clientsQ.isLoading && !clientsQ.isError && visible.length === 0 && (
          <div style={{
            padding: '48px 40px',
            fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 22, color: 'var(--mid)',
          }}>
            {filter !== 'all'
              ? `No ${filter} clients.`
              : search
              ? 'No clients match that search.'
              : 'No clients yet — admit the first one above.'}
          </div>
        )}
        {visible.map((c) => (
          <ClientRow key={c.id} client={c} onAction={handleAction} disabled={anyPending} counts={countsByClient[c.id]} />
        ))}
      </div>

      {/* ── Confirm modal ───────────────────────────────────────────── */}
      {confirmOpts && (
        <ConfirmModal
          {...confirmOpts}
          onConfirm={confirmOpts.onConfirm}
          onCancel={() => setConfirmOpts(null)}
        />
      )}

      {/* ── Modals ──────────────────────────────────────────────────── */}
      {newClientOpen && (
        <NewClientModal
          onClose={() => setNewClientOpen(false)}
          onCreated={invalidate}
        />
      )}

      {magicLink && (
        <MagicLinkModal
          url={magicLink.url}
          scope="full"
          onClose={() => setMagicLink(null)}
          onRegenerate={async () => {
            const data = await admin.createMagicLink(magicLink.clientId);
            setMagicLink({ url: data.url, clientId: magicLink.clientId });
          }}
        />
      )}

    </div>
  );
}
