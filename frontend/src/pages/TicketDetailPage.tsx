// ─────────────────────────────────────────────────────────────────────────────
// TicketDetailPage.tsx — the article + composer + collapsible rails.
//
// Data:
//   • GET /tickets/:id          (with thread, polled every 30s)
//   • GET /tickets/:id/notes    (desk only, lazy)
//   • GET /tickets              (for the left "queue" rail)
//
// Mutations:
//   • POST /tickets/:id/message     reply (with optional attachment)
//   • POST /tickets/:id/notes       internal note (desk only)
//   • PATCH /tickets/:id/ack|pend|close   transitions (desk only)
//   • POST /tickets/:id/magic-link  share with a client (desk only)
//
// Role gating:
//   • Internal-note tab + the entire "Internal note" sidebar section and the
//     transition buttons are conditional on isDesk.
//
// Collapsible rails:
//   • The body grid uses CSS custom properties; we just toggle classes on
//     .lg-detail__body. Persisted to localStorage so the user's preferred
//     "working vs reading" mode survives reloads.
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { tickets as ticketsApi } from '../api/tickets';
import { admin } from '../api/admin';
import type {
  InternalNote,
  TicketResponse,
  TicketWithThread,
  ThreadEntry,
} from '../api/types';
import { Masthead } from '../components/Masthead';
import { StatusPill } from '../components/StatusPill';
import { PriorityBars } from '../components/PriorityBars';
import { useAuth } from '../auth/AuthContext';
import '../styles/detail.css';

type ComposerTab = 'reply' | 'note';

// ── localStorage keys for rail collapse state ────────────────────────────
const LS_QUEUE = 'lodgr.detail.queueCollapsed';
const LS_PROPS = 'lodgr.detail.propsCollapsed';

function readBool(key: string): boolean {
  try { return localStorage.getItem(key) === '1'; } catch { return false; }
}
function writeBool(key: string, v: boolean) {
  try { localStorage.setItem(key, v ? '1' : '0'); } catch { /* private mode etc */ }
}

// ── Small format helpers ─────────────────────────────────────────────────
function fmtDateTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString('en-GB', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
function timeAgo(iso: string): string {
  const ms = Date.now() - +new Date(iso);
  const m = Math.floor(ms / 60_000);
  if (m < 1) return 'just now';
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  if (d < 7) return `${d}d ago`;
  return new Date(iso).toLocaleDateString('en-GB', { day: '2-digit', month: 'short' });
}
function daysUntil(date: string | null): number | null {
  if (!date) return null;
  const t = new Date(); t.setHours(0, 0, 0, 0);
  const d = new Date(date + 'T00:00:00');
  return Math.round((+d - +t) / 86_400_000);
}

function clientInitials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length >= 2) return (words[0][0] + words[1][0]).toUpperCase();
  return name.replace(/[^a-z0-9]/gi, '').slice(0, 2).toUpperCase() || '??';
}

export function TicketDetailPage() {
  const { id = '' } = useParams<{ id: string }>();
  const { user, isDesk, isScoped } = useAuth();
  const nav = useNavigate();
  const qc = useQueryClient();

  // ── Rail collapse state ───────────────────────────────────────────────
  const [queueCollapsed, setQueueCollapsed] = useState(() => readBool(LS_QUEUE));
  const [propsCollapsed, setPropsCollapsed] = useState(() => readBool(LS_PROPS));
  useEffect(() => writeBool(LS_QUEUE, queueCollapsed), [queueCollapsed]);
  useEffect(() => writeBool(LS_PROPS, propsCollapsed), [propsCollapsed]);

  // Mobile sheet (separate from desktop collapse; only used < 1024px).
  const [sheetOpen, setSheetOpen] = useState(false);

  // ── Queries ──────────────────────────────────────────────────────────
  const ticketQ = useQuery({
    queryKey: ['ticket', id],
    queryFn: () => ticketsApi.get(id),
    enabled: !!id,
    refetchInterval: 30_000,
    refetchOnWindowFocus: false,
  });

  // The queue rail wants the same list the list page uses — share the cache
  // key so we don't double-fetch.
  const queueQ = useQuery({
    queryKey: ['tickets', 1, 50],
    queryFn: () => ticketsApi.list(1, 50),
    // Scoped sessions can only see one ticket, so /tickets would 404/empty —
    // don't bother asking.
    enabled: !isScoped,
    refetchInterval: 60_000,
    refetchOnWindowFocus: false,
  });

  const notesQ = useQuery({
    queryKey: ['notes', id],
    queryFn: () => ticketsApi.listNotes(id),
    enabled: !!id && isDesk,
    refetchOnWindowFocus: false,
  });

  const clientsQ = useQuery({
    queryKey: ['clients'],
    queryFn: () => admin.listClients(),
    enabled: isDesk,
    staleTime: 5 * 60_000,
  });

  const clientMap = useMemo(() => {
    const m: Record<string, string> = {};
    for (const c of clientsQ.data ?? []) m[c.id] = c.name;
    return m;
  }, [clientsQ.data]);

  // ── Mutations ────────────────────────────────────────────────────────
  const replyM = useMutation({
    mutationFn: (vars: { body: string; file?: File }) =>
      ticketsApi.postMessage(id, vars.body, vars.file),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['ticket', id] });
    },
  });

  const noteM = useMutation({
    mutationFn: (body: string) => ticketsApi.addNote(id, body),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['notes', id] });
    },
  });

  const transitionM = useMutation({
    mutationFn: (kind: 'ack' | 'pend' | 'close' | 'reopen') =>
      kind === 'ack'    ? ticketsApi.ack(id) :
      kind === 'pend'   ? ticketsApi.pend(id) :
      kind === 'reopen' ? ticketsApi.reopen(id) :
      ticketsApi.close(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['ticket', id] });
      void qc.invalidateQueries({ queryKey: ['tickets', 1, 50] });
    },
  });

  const magicM = useMutation({
    mutationFn: () => ticketsApi.magicLinkFor(id),
    onSuccess: (data) => {
      // Copy URL to clipboard for desk convenience.
      try { void navigator.clipboard?.writeText(data.url); } catch { /* fine */ }
      alert(`Magic link copied to clipboard:\n${data.url}`);
    },
  });

  // ── Composer state ────────────────────────────────────────────────────
  const [composerTab, setComposerTab] = useState<ComposerTab>('reply');
  const [composerBody, setComposerBody] = useState('');
  const fileRef = useRef<HTMLInputElement>(null);
  const [pendingFile, setPendingFile] = useState<File | null>(null);

  async function submitComposer(e: React.FormEvent) {
    e.preventDefault();
    const body = composerBody.trim();
    if (!body) return;
    if (composerTab === 'reply') {
      await replyM.mutateAsync({ body, file: pendingFile ?? undefined });
    } else {
      await noteM.mutateAsync(body);
    }
    setComposerBody('');
    setPendingFile(null);
    if (fileRef.current) fileRef.current.value = '';
  }

  // ── Derived ──────────────────────────────────────────────────────────
  const ticket: TicketWithThread | undefined = ticketQ.data;
  const thread: ThreadEntry[] = ticket?.thread ?? [];
  const notes: InternalNote[] = notesQ.data ?? [];

  // What transitions are legal from the current status?
  const can = useMemo(() => {
    const s = ticket?.status;
    return {
      ack:    s === 'open' || s === 'pending',
      pend:   s === 'open' || s === 'acknowledged',
      close:  s === 'acknowledged',
      reopen: s === 'closed',
    };
  }, [ticket?.status]);

  const due = daysUntil(ticket?.due_date ?? null);
  const dueLbl = due === null ? '—' : due < 0 ? `${Math.abs(due)}d overdue` : due === 0 ? 'today' : `${due}d`;
  const clientName = ticket ? (clientMap[ticket.client_id] ?? `#${ticket.client_id.slice(0, 8)}`) : '—';
  const createdByLabel = isDesk ? 'the desk' : clientName;

  // ── Render ───────────────────────────────────────────────────────────
  if (ticketQ.isLoading) {
    return (
      <div className="lg-detail grain">
        <Masthead active="tickets" />
        <div style={{ padding: 64, textAlign: 'center', fontFamily: 'var(--mono)', color: 'var(--mid)', letterSpacing: '.18em', textTransform: 'uppercase', fontSize: 11 }}>
          — Loading ticket —
        </div>
      </div>
    );
  }
  if (ticketQ.isError || !ticket) {
    return (
      <div className="lg-detail grain">
        <Masthead active="tickets" />
        <div style={{ padding: 64, textAlign: 'center' }}>
          <h3 style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 36, marginBottom: 8 }}>
            Couldn't load that ticket.
          </h3>
          <p style={{ fontFamily: 'var(--sans)', fontWeight: 500, fontSize: 16, color: 'var(--mid)' }}>
            {(ticketQ.error as Error)?.message ?? 'It may have been closed or moved.'}
          </p>
          <Link to="/tickets" style={{ display: 'inline-block', marginTop: 16, fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '.14em', textTransform: 'uppercase', color: 'var(--ink)', borderBottom: '1px solid var(--ink)', paddingBottom: 2, textDecoration: 'none' }}>
            ← Back to the queue
          </Link>
        </div>
      </div>
    );
  }

  // The queue rail respects the user's mode but stays hidden for scoped users.
  const showQueueRail = !isScoped;

  return (
    <div className="lg-detail grain">
      <Masthead active="tickets" />

      {/* ── Breadcrumb + mode toggles ─────────────────────────────────── */}
      <div className="lg-detail__bread">
        <button type="button" className="lg-detail__back" onClick={() => nav('/tickets')}>← Tickets</button>
        <span>/</span>
        <b>{ticket.id.slice(0, 8)}</b>
        <span className="lg-detail__bread-client" style={{ color: 'var(--mid)' }}>· {clientName}</span>
        <div className="lg-detail__bread-spacer" />

        {showQueueRail && (
          <div className="lg-detail__modes">
            <button
              type="button"
              className={'lg-detail__mode' + (!queueCollapsed && !propsCollapsed ? ' is-active' : '')}
              onClick={() => { setQueueCollapsed(false); setPropsCollapsed(false); }}
              title="Show both rails"
            >
              ⇄ Working
            </button>
            <button
              type="button"
              className={'lg-detail__mode' + (queueCollapsed && propsCollapsed ? ' is-active' : '')}
              onClick={() => { setQueueCollapsed(true); setPropsCollapsed(true); }}
              title="Hide both rails"
            >
              ━ Reading
            </button>
          </div>
        )}

        <button
          type="button"
          className="lg-sheet-trigger"
          onClick={() => setSheetOpen(true)}
          aria-label="Open ticket controls"
        >
          Controls
        </button>
      </div>

      {/* ── Three-column body ────────────────────────────────────────── */}
      <div
        className={
          'lg-detail__body' +
          (queueCollapsed ? ' is-queue-collapsed' : '') +
          (propsCollapsed ? ' is-props-collapsed' : '')
        }
      >
        {/* ── Queue rail ───────────────────────────────────────────── */}
        {showQueueRail && (
          <aside className="lg-queue">
            <div className="lg-queue__h">
              <span>Queue</span>
              <b>{queueQ.data?.total ?? '—'} tickets</b>
              <button
                type="button"
                className="lg-queue__toggle"
                onClick={() => setQueueCollapsed((v) => !v)}
                aria-label={queueCollapsed ? 'Expand queue' : 'Collapse queue'}
              >
                {queueCollapsed ? '›' : '‹'}
              </button>
            </div>

            {/* Vertical strip shown only when collapsed. */}
            <div className="lg-queue__collapsed">
              <span className="badge">
                <span className="red">{String(queueQ.data?.tickets.filter((t) => t.priority === 'urgent' && t.status !== 'closed').length ?? 0).padStart(2, '0')}</span>
                /{queueQ.data?.total ?? 0}
              </span>
              <span className="vrt">Queue · expand</span>
            </div>

            {/* Item list. */}
            {(queueQ.data?.tickets ?? []).slice(0, 12).map((t: TicketResponse) => {
              const cls =
                'lg-queue__item' + (t.id === ticket.id ? ' is-active' : '');
              const dotCls = t.status === 'open' ? 'open' : t.status === 'acknowledged' ? 'ack' : '';
              return (
                <Link key={t.id} to={`/tickets/${t.id}`} className={cls}>
                  <div className="lg-queue__id">
                    <b>{t.id.slice(0, 8)}</b>
                    <span>{timeAgo(t.created_at)}</span>
                  </div>
                  <div className="lg-queue__ttl">{t.title}</div>
                  <div className="lg-queue__meta">
                    <span className={'dot ' + dotCls} />
                    <span>{t.status}</span>
                    {t.priority === 'urgent' && t.status !== 'closed' && (
                      <span style={{ color: 'var(--red)' }}>· urgent</span>
                    )}
                  </div>
                </Link>
              );
            })}
          </aside>
        )}

        {/* ── Article (center) ─────────────────────────────────────── */}
        <article className="lg-article">
          <div className="lg-article__above">
            <div className="lg-article__above-l">
              {ticket.category ?? ticket.ticket_type.replace('_', ' ')}
            </div>
            <div className="lg-article__above-r">
              <b>#{ticket.id.slice(0, 8)}</b>
              <br />
              Opened {timeAgo(ticket.created_at)} · {fmtDateTime(ticket.created_at)}
            </div>
          </div>

          <h1 className="lg-article__h1">{ticket.title}</h1>
          <div className="lg-article__meta">
            <div className="mi"><div className="k">Client</div><div className="v">{clientName}</div></div>
            <div className="mi"><div className="k">Status</div><div className="v red">{ticket.status}</div></div>
            <div className="mi"><div className="k">Due in</div><div className="v red">{dueLbl}</div></div>
            <div className="mi"><div className="k">Priority</div><div className="v">{ticket.priority}</div></div>
          </div>
          {ticket.description && (
            <div className="lg-article__dek"><ReactMarkdown remarkPlugins={[remarkGfm]}>{ticket.description}</ReactMarkdown></div>
          )}

          <div className="lg-article__byline">
            <span className="lg-article__byline-av">
              {clientInitials(clientMap[ticket.client_id] ?? ticket.client_id)}
            </span>
            <div className="lg-article__byline-nm">
              {clientName} <b>· created by {createdByLabel}</b>
              <small>
                {thread.length} message{thread.length === 1 ? '' : 's'} · due {dueLbl}
              </small>
            </div>
            <div className="lg-article__byline-pills">
              <StatusPill status={ticket.status} />
              <PriorityBars priority={ticket.priority} />
            </div>
          </div>

          {/* ── Thread ─────────────────────────────────────────────── */}
          <div className="lg-thread">
            {thread.length === 0 && (
              <p style={{ fontFamily: 'var(--sans)', fontStyle: 'italic', color: 'var(--mid)' }}>
                No messages yet — the description above is everything.
              </p>
            )}
            {thread.map((m, i) => {
              // The handoff doesn't currently tell us if a sender is desk or
              // client per message. We fall back to a sender_id comparison.
              const isFromDesk = !!user && m.sender_id === user.sub && isDesk;
              return (
                <div
                  key={m.id}
                  className={'lg-msg' + (isFromDesk ? ' is-desk' : '')}
                  style={{ ['--i' as string]: i }}
                >
                  <div className="lg-msg__marker">
                    <div className="lg-msg__num">{String(i + 1).padStart(2, '0')}</div>
                    <div className="lg-msg__role">{isFromDesk ? 'From the desk' : 'Client'}</div>
                  </div>
                  <div>
                    <div className="lg-msg__who">
                      {m.sender_id.slice(0, 8)} <b>· {fmtDateTime(m.created_at)}</b>
                    </div>
                    <div className="lg-msg__txt">{m.body}</div>
                    {m.attachment_path && (
                      <span className="lg-msg__attach" title="Backend doesn't yet expose a download route (see Known Gaps in the handoff).">
                        📎 {m.attachment_path.split('/').pop()} (download n/a)
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>

          {/* ── Composer ──────────────────────────────────────────── */}
          {ticket.status === 'closed' && (
            <div style={{ fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '.12em', color: 'var(--mid)', padding: '12px 0 4px', textTransform: 'uppercase' }}>
              — Replying will reopen this ticket
            </div>
          )}
          <form className="lg-composer" onSubmit={submitComposer}>
              <div className="lg-composer__tabs" role="tablist">
                <button
                  type="button"
                  role="tab"
                  className={'lg-composer__tab' + (composerTab === 'reply' ? ' is-active' : '')}
                  onClick={() => setComposerTab('reply')}
                >
                  Reply to {isDesk ? 'client' : 'desk'}
                </button>
                {isDesk && (
                  <button
                    type="button"
                    role="tab"
                    className={'lg-composer__tab is-note' + (composerTab === 'note' ? ' is-active' : '')}
                    onClick={() => setComposerTab('note')}
                  >
                    Internal note
                  </button>
                )}
                <span style={{ marginLeft: 'auto' }}>⌘↵ to send</span>
              </div>

              <textarea
                placeholder={composerTab === 'note'
                  ? 'A note for yourself — clients will never see this.'
                  : 'Write a reply…'}
                value={composerBody}
                onChange={(e) => setComposerBody(e.target.value)}
                onKeyDown={(e) => {
                  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                    void submitComposer(e as unknown as React.FormEvent);
                  }
                }}
                rows={4}
              />

              <div className="lg-composer__ctrls">
                <div className="lg-composer__l">
                  {composerTab === 'reply' && (
                    <label>
                      + Attach{pendingFile ? `: ${pendingFile.name}` : ''}
                      <input
                        ref={fileRef}
                        type="file"
                        hidden
                        onChange={(e) => setPendingFile(e.target.files?.[0] ?? null)}
                      />
                    </label>
                  )}
                </div>
                <button
                  type="submit"
                  className="lg-composer__send"
                  disabled={replyM.isPending || noteM.isPending || !composerBody.trim()}
                >
                  {composerTab === 'note' ? 'Save note →' : 'Send reply →'}
                </button>
              </div>
            </form>
        </article>

        {/* ── Props rail (right) ───────────────────────────────────── */}
        <aside className="lg-props">
          <div className="lg-props__h">
            <button
              type="button"
              className="lg-props__toggle"
              onClick={() => setPropsCollapsed((v) => !v)}
              aria-label={propsCollapsed ? 'Expand controls' : 'Collapse controls'}
            >
              {propsCollapsed ? '‹' : '›'}
            </button>
            <span>Controls</span>
          </div>

          <div className="lg-props__collapsed">
            <span className="vrt">Controls · expand</span>
          </div>

          {isDesk && (
            <PropsContent
              ticket={ticket}
              notes={notes}
              can={can}
              transition={(k) => transitionM.mutate(k)}
              shareMagicLink={() => magicM.mutate()}
              transitionPending={transitionM.isPending}
              magicPending={magicM.isPending}
            />
          )}
          {!isDesk && (
            <ReadOnlyProps ticket={ticket} />
          )}
        </aside>
      </div>

      {/* ── Mobile bottom sheet ─────────────────────────────────────── */}
      <div className={'lg-sheet' + (sheetOpen ? ' is-open' : '')}>
        <div className="lg-sheet__h">
          <span className="t"><i>Controls</i></span>
          <button type="button" className="x" onClick={() => setSheetOpen(false)}>Close</button>
        </div>
        {isDesk ? (
          <PropsContent
            ticket={ticket}
            notes={notes}
            can={can}
            transition={(k) => { transitionM.mutate(k); setSheetOpen(false); }}
            shareMagicLink={() => magicM.mutate()}
            transitionPending={transitionM.isPending}
            magicPending={magicM.isPending}
          />
        ) : (
          <ReadOnlyProps ticket={ticket} />
        )}
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// PropsContent — the desk-only controls panel (transitions + properties + note
// + share). Used twice: in the right rail (desktop) and in the bottom sheet
// (mobile). Keep the markup identical so styles cascade.
// ─────────────────────────────────────────────────────────────────────────────
function PropsContent({
  ticket,
  notes,
  can,
  transition,
  shareMagicLink,
  transitionPending,
  magicPending,
}: {
  ticket: TicketResponse;
  notes: InternalNote[];
  can: { ack: boolean; pend: boolean; close: boolean; reopen: boolean };
  transition: (k: 'ack' | 'pend' | 'close' | 'reopen') => void;
  shareMagicLink: () => void;
  transitionPending: boolean;
  magicPending: boolean;
}) {
  return (
    <>
      <div className="lg-props__sec">
        <div className="lg-props__lbl">Transition</div>
        <div className="lg-props__actions">
          <button
            type="button"
            className="lg-props__act is-primary"
            disabled={!can.ack || transitionPending}
            onClick={() => transition('ack')}
          >
            <span>Acknowledge</span>
            <span className="arr">{ticket.status} → ack</span>
          </button>
          <button
            type="button"
            className="lg-props__act"
            disabled={!can.pend || transitionPending}
            onClick={() => transition('pend')}
          >
            <span>Pend on client</span>
            <span className="arr">→ pending</span>
          </button>
          <button
            type="button"
            className="lg-props__act is-danger"
            disabled={!can.close || transitionPending}
            onClick={() => {
              if (confirm('Close this ticket? Clients can still reply to reopen it.')) {
                transition('close');
              }
            }}
          >
            <span>Close ticket</span>
            <span className="arr">→ closed</span>
          </button>
          <button
            type="button"
            className="lg-props__act"
            disabled={!can.reopen || transitionPending}
            onClick={() => transition('reopen')}
          >
            <span>Reopen ticket</span>
            <span className="arr">closed → open</span>
          </button>
        </div>
      </div>

      <div className="lg-props__sec">
        <div className="lg-props__lbl">Properties</div>
        <div className="lg-props__kv">
          <div className="row"><span className="k">Priority</span><span className="v italic">{ticket.priority}</span></div>
          <div className="row"><span className="k">Category</span><span className="v italic">{ticket.category ?? '—'}</span></div>
          <div className="row"><span className="k">Type</span><span className="v">{ticket.ticket_type.replace('_', ' ')}</span></div>
          <div className="row"><span className="k">Due</span><span className="v">{ticket.due_date ?? '—'}</span></div>
          <div className="row"><span className="k">Created</span><span className="v">{new Date(ticket.created_at).toLocaleDateString('en-GB')}</span></div>
          <div className="row"><span className="k">Recurring</span><span className="v" style={{ color: ticket.recurring ? 'var(--red)' : 'var(--mid)' }}>
            {ticket.recurring ? `every ${ticket.recurring_interval_days ?? '?'}d` : '—'}
          </span></div>
        </div>
      </div>

      <div className="lg-props__sec">
        <div className="lg-props__lbl">Client</div>
        <div className="lg-props__kv">
          <div className="row"><span className="k">ID</span><span className="v">{ticket.client_id.slice(0, 8)}</span></div>
          <div className="row"><span className="k">Created by</span><span className="v">{ticket.created_by.slice(0, 8)}</span></div>
        </div>
      </div>

      {notes.length > 0 && (
        <div className="lg-props__sec">
          <div className="lg-props__lbl">Internal notes · {notes.length}</div>
          {notes.map((n) => (
            <div key={n.id} className="lg-props__note" style={{ marginBottom: 8 }}>
              <div className="lg-props__note-h">Desk only</div>
              <div className="lg-props__note-b">{n.body}</div>
              <div className="lg-props__note-f">
                {n.author_id.slice(0, 8)} · {new Date(n.created_at).toLocaleString('en-GB')}
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="lg-props__sec">
        <div className="lg-props__lbl">Share</div>
        <div className="lg-props__actions">
          <button
            type="button"
            className="lg-props__act"
            disabled={magicPending}
            onClick={shareMagicLink}
          >
            <span>{magicPending ? 'Generating…' : 'Magic link for client'}</span>
            <span className="arr">→ copy</span>
          </button>
        </div>
      </div>
    </>
  );
}

/** Client-side props rail — read-only summary, no transitions / no notes. */
function ReadOnlyProps({ ticket }: { ticket: TicketResponse }) {
  return (
    <>
      <div className="lg-props__sec">
        <div className="lg-props__lbl">Properties</div>
        <div className="lg-props__kv">
          <div className="row"><span className="k">Status</span><span className="v italic">{ticket.status}</span></div>
          <div className="row"><span className="k">Priority</span><span className="v italic">{ticket.priority}</span></div>
          <div className="row"><span className="k">Category</span><span className="v italic">{ticket.category ?? '—'}</span></div>
          <div className="row"><span className="k">Due</span><span className="v">{ticket.due_date ?? '—'}</span></div>
          <div className="row"><span className="k">Created</span><span className="v">{new Date(ticket.created_at).toLocaleDateString('en-GB')}</span></div>
        </div>
      </div>
    </>
  );
}
