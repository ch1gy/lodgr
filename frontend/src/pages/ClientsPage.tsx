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

import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { PasswordGenerator } from '../components/PasswordGenerator';
import { MagicLinkModal } from '../components/MagicLinkModal';
import { ConfirmModal } from '../components/ConfirmModal';
import type { ConfirmOptions } from '../components/ConfirmModal';
import { admin } from '../api/admin';
import { downloadBlob } from '../utils/format';
import type { Client } from '../api/types';
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


// ── Per-row ───────────────────────────────────────────────────────────────────
interface RowProps {
  client: Client;
  onAction: (action: string, id: string) => void;
  /** Disable all action buttons while a mutation is in flight. */
  disabled?: boolean;
}

function ClientRow({ client, onAction, disabled = false }: RowProps) {
  const status     = clientStatus(client);
  const isArchived = status === 'archived';
  const isLocked   = status === 'locked';

  const metaLine = isLocked
    ? `Locked · ${client.failed_attempts} failed attempt${client.failed_attempts !== 1 ? 's' : ''}`
    : isArchived
    ? `Archived ${client.deleted_at?.slice(0, 10) ?? ''}`
    : client.failed_attempts > 0
    ? `${client.failed_attempts} failed attempt${client.failed_attempts !== 1 ? 's' : ''}`
    : `id · ${client.id.slice(0, 8)}`;

  return (
    <div className={`lg-cl-row${isArchived ? ' deleted' : isLocked ? ' locked' : ''}`}>
      <div className="av">{clientInitials(client.name)}</div>

      <div className="name-blk">
        <div className="nm">{client.name}</div>
        <div className="em">{client.email}</div>
      </div>

      <div className="meta-blk">
        <b>{client.id.slice(0, 8)}</b><br />
        {metaLine}
      </div>

      {/* Ticket count not exposed by /admin/clients yet — placeholder */}
      <div className="stat-blk">
        <div className="v">—</div>
        <div className="lbl">tickets</div>
      </div>

      <div className={`status-blk ${status}`}>
        <span className="dot" />{status}
      </div>

      <div className="acts">
        {isArchived ? (
          <>
            <button type="button" className="a" disabled={disabled} onClick={() => onAction('restore', client.id)}>
              Restore ⟲
            </button>
            <button type="button" className="a" disabled={disabled} onClick={() => onAction('export', client.id)}>
              Export ↓
            </button>
            <button type="button" className="a danger" disabled={disabled} onClick={() => onAction('hard-delete', client.id)}>
              Hard delete ✕
            </button>
          </>
        ) : (
          <>
            <button type="button" className="a primary" disabled={disabled} onClick={() => onAction('magic-link', client.id)}>
              Magic link ↗
            </button>
            {isLocked && (
              <button type="button" className="a" disabled={disabled} onClick={() => onAction('unlock', client.id)}>
                Unlock ⊙
              </button>
            )}
            <button type="button" className="a" disabled={disabled} onClick={() => onAction('revoke', client.id)}>
              Revoke sessions
            </button>
            <button type="button" className="a" disabled={disabled} onClick={() => onAction('export', client.id)}>
              Export ↓
            </button>
            <button type="button" className="a danger" disabled={disabled} onClick={() => onAction('archive', client.id)}>
              Archive ⌫
            </button>
          </>
        )}
      </div>
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

  const createM = useMutation({
    mutationFn: () => admin.createClient({ name: name.trim(), email: email.trim(), password }),
    onSuccess: () => {
      onCreated();
      onClose();
    },
  });

  const err = createM.error
    ? ((createM.error as { response?: { data?: { error?: string } } })
        .response?.data?.error ?? 'Failed to create client. Try again.')
    : null;

  const canSubmit =
    name.trim().length > 0 &&
    email.trim().includes('@') &&
    password.length >= 8 &&
    !createM.isPending;

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

  const allClients = clientsQ.data ?? [];

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
        magicM.mutate(id);
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
          <h1>The roster<span className="count">/ {String(counts.all).padStart(2, '0')}</span></h1>
          <div className="sub">
            {counts.active} active · {counts.locked} locked · {counts.archived} archived
          </div>
        </div>
        <div className="right">
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
          <ClientRow key={c.id} client={c} onAction={handleAction} disabled={anyPending} />
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

      <BottomTabBar active="clients" />
    </div>
  );
}
