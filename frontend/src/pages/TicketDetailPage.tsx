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
import ReactMarkdown from 'react-markdown';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { tickets as ticketsApi } from '../api/tickets';
import { api } from '../api/client';
import { downloadBlob } from '../utils/format';
import { ConfirmModal } from '../components/ConfirmModal';
import type { ConfirmOptions } from '../components/ConfirmModal';
import type {
  InternalNote,
  PatchTicketPayload,
  TicketResponse,
  TicketWithThread,
  ThreadEntry,
} from '../api/types';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { MagicLinkModal } from '../components/MagicLinkModal';
import { StatusPill } from '../components/StatusPill';
import { PriorityBars } from '../components/PriorityBars';
import { SlaOdometer } from '../components/SlaOdometer';
import { EditPropsPanel } from '../components/EditPropsPanel';
import { ReadOnlyProps } from '../components/ReadOnlyProps';
import { useAuth } from '../auth/AuthContext';
import { timeAgo, daysUntil, fmtDateTime, TICKET_TYPE_LABEL, extractApiError } from '../utils/format';
import '../styles/detail.css';
import '../styles/v2.css';

const vt = (name: string): React.CSSProperties =>
  ({ viewTransitionName: name } as unknown as React.CSSProperties);

type ComposerTab = 'reply' | 'note';

/** Which desk transitions are legal from the ticket's current status. */
type TransitionAbility = { ack: boolean; pend: boolean; close: boolean };

// ── localStorage keys for rail collapse state ────────────────────────────
const LS_QUEUE = 'lodgr.detail.queueCollapsed';
const LS_PROPS = 'lodgr.detail.propsCollapsed';

function readBool(key: string): boolean {
  try { return localStorage.getItem(key) === '1'; } catch { return false; }
}
function writeBool(key: string, v: boolean) {
  try { localStorage.setItem(key, v ? '1' : '0'); } catch { /* private mode etc */ }
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

  // Magic-link modal state (replaces the old alert()).
  const [magicLinkUrl, setMagicLinkUrl] = useState<string | null>(null);

  // Edit-properties mode — shared by desktop rail and mobile sheet.
  const [editingProps, setEditingProps] = useState(false);

  // Custom confirm modal (replaces window.confirm).
  const [confirmOpts, setConfirmOpts] = useState<(ConfirmOptions & { onConfirm: () => void }) | null>(null);

  function showConfirm(opts: ConfirmOptions, onConfirm: () => void) {
    setConfirmOpts({ ...opts, onConfirm });
  }

  // Warn on reload/tab-close if editing is unsaved.
  useEffect(() => {
    if (!editingProps) return;
    const handle = (e: BeforeUnloadEvent) => { e.preventDefault(); };
    window.addEventListener('beforeunload', handle);
    return () => window.removeEventListener('beforeunload', handle);
  }, [editingProps]);

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
    mutationFn: (kind: 'ack' | 'pend' | 'close') =>
      kind === 'ack' ? ticketsApi.ack(id) :
      kind === 'pend' ? ticketsApi.pend(id) :
      ticketsApi.close(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['ticket', id] });
      void qc.invalidateQueries({ queryKey: ['tickets', 1, 50] });
    },
  });

  const magicM = useMutation({
    mutationFn: () => ticketsApi.magicLinkFor(id),
    onSuccess: (data) => {
      setMagicLinkUrl(data.url);
    },
  });

  const patchM = useMutation({
    mutationFn: (payload: PatchTicketPayload) => ticketsApi.patch(id, payload),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['ticket', id] });
      void qc.invalidateQueries({ queryKey: ['tickets', 1, 50] });
      setEditingProps(false);
    },
  });

  const deleteM = useMutation({
    mutationFn: () => ticketsApi.delete(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['tickets', 1, 50] });
      nav('/tickets');
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
    try {
      if (composerTab === 'reply') {
        await replyM.mutateAsync({ body, file: pendingFile ?? undefined });
      } else {
        await noteM.mutateAsync(body);
      }
      setComposerBody('');
      setPendingFile(null);
      if (fileRef.current) fileRef.current.value = '';
    } catch {
      // replyM.error / noteM.error surfaces the error inline; no action needed here.
    }
  }

  // ── Derived ──────────────────────────────────────────────────────────
  const ticket: TicketWithThread | undefined = ticketQ.data;
  const thread: ThreadEntry[] = ticket?.thread ?? [];
  const notes: InternalNote[] = notesQ.data ?? [];

  // Which transitions are legal from the current status?
  // open → ack → closed  |  open → pending → ack → closed
  const can = useMemo<TransitionAbility>(() => {
    const s = ticket?.status;
    return {
      ack:   s === 'open' || s === 'pending',
      pend:  s === 'open' || s === 'acknowledged',
      close: s === 'acknowledged',
    };
  }, [ticket?.status]);

  const queueUrgentCount = useMemo(
    () => (queueQ.data?.tickets ?? []).filter(
      (t) => t.priority === 'urgent' && t.status !== 'closed'
    ).length,
    [queueQ.data?.tickets]
  );

  const due = daysUntil(ticket?.due_date ?? null);
  const dueLbl = due === null ? '—' : due < 0 ? `${Math.abs(due)}d overdue` : due === 0 ? 'today' : `${due}d`;

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
        <span style={{ color: 'var(--mid)' }}>· {ticket.client_id.slice(0, 8)}</span>
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
                <span className="red">{String(queueUrgentCount).padStart(2, '0')}</span>
                /{queueQ.data?.total ?? 0}
              </span>
              <span className="vrt">Queue · expand</span>
            </div>

            {/* Item list. */}
            {(queueQ.data?.tickets ?? []).map((t: TicketResponse) => {
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
              {ticket.category ?? TICKET_TYPE_LABEL[ticket.ticket_type]}
              {ticket.sub_client_name && (
                <span style={{ marginLeft: 10, color: 'var(--red)' }}>
                  › {ticket.sub_client_name}
                </span>
              )}
            </div>
            <div className="lg-article__above-r">
              <b>{ticket.id}</b>
              <br />
              Opened {timeAgo(ticket.created_at)} · {fmtDateTime(ticket.created_at)}
            </div>
          </div>

          <h1 className="lg-article__h1" style={vt('sig-headline')}>{ticket.title}</h1>
          {ticket.description && (
            <div className="lg-article__md">
              <ReactMarkdown>{ticket.description}</ReactMarkdown>
            </div>
          )}

          <div className="lg-article__byline">
            <span className="lg-article__byline-av" style={vt('sig-avatar')}>
              {ticket.client_id.slice(0, 2).toUpperCase()}
            </span>
            <div className="lg-article__byline-nm">
              Client {ticket.client_id.slice(0, 8)} <b>· created by {ticket.created_by.slice(0, 8)}</b>
              <small>
                {thread.length} message{thread.length === 1 ? '' : 's'} · due {dueLbl}
              </small>
            </div>
            <div className="lg-article__byline-pills">
              <StatusPill status={ticket.status} />
              <PriorityBars priority={ticket.priority} />
              <span style={vt('sig-clock')}>
                <SlaOdometer
                  dueDate={ticket.due_date}
                  estimatedCompletion={ticket.estimated_completion}
                />
              </span>
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
              // The backend doesn't include sender role in ThreadEntry, so we
              // infer it:
              //   • Desk viewer  → their own messages are "from the desk".
              //   • Client viewer → any sender that isn't the ticket's client
              //                     is the desk (single-desk model for now).
              const isFromDesk = isDesk
                ? (!!user && m.sender_id === user.sub)
                : m.sender_id !== ticket.client_id;
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
                      <button
                        type="button"
                        className="lg-msg__attach"
                        onClick={async () => {
                          try {
                            const blob = await api.get<Blob>(m.attachment_path!, { responseType: 'blob' }).then((r) => r.data);
                            downloadBlob(blob, m.attachment_path!.split('/').pop() ?? 'attachment');
                          } catch {
                            /* attachment unavailable — no-op */
                          }
                        }}
                        style={{ cursor: 'pointer', background: 'none', border: 'none', textDecoration: 'underline', textAlign: 'left', padding: 0 }}
                        title="Download attachment"
                      >
                        📎 {m.attachment_path.split('/').pop()}
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>

          {/* ── Composer ──────────────────────────────────────────── */}
          {ticket.status !== 'closed' && (
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
                <span style={{ marginLeft: 'auto' }}>
                  {/Mac|iPhone|iPad/.test(navigator.platform) ? '⌘↵' : 'Ctrl+↵'} to send
                </span>
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

              {(replyM.isError || noteM.isError) && (
                <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--red)', padding: '4px 0' }}>
                  {extractApiError(replyM.error ?? noteM.error, 'Failed to send — try again.')}
                </div>
              )}

              <div className="lg-composer__ctrls">
                <div className="lg-composer__l">
                  {composerTab === 'reply' && (
                    <label>
                      + Attach{pendingFile ? `: ${pendingFile.name}` : ''}
                      <input
                        ref={fileRef}
                        type="file"
                        hidden
                        accept=".pdf,.png,.jpg,.jpeg,.gif,.txt,.md"
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
          )}
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

          {isDesk ? (
            <PropsContent
              ticket={ticket}
              notes={notes}
              can={can}
              transition={(k) => transitionM.mutate(k)}
              shareMagicLink={() => magicM.mutate()}
              onDelete={() =>
                showConfirm(
                  { title: 'Delete this ticket?', body: 'All messages and attachments will be permanently erased. This cannot be undone.', confirmLabel: 'Delete permanently', danger: true },
                  () => { deleteM.mutate(); setConfirmOpts(null); }
                )
              }
              transitionPending={transitionM.isPending}
              magicPending={magicM.isPending}
              deletePending={deleteM.isPending}
              editingProps={editingProps}
              onToggleEdit={() => setEditingProps((v) => !v)}
              onSaveEdit={(p) => patchM.mutate(p)}
              savePending={patchM.isPending}
            />
          ) : (
            <ReadOnlyProps ticket={ticket} />
          )}
        </aside>
      </div>

      {/* ── Magic-link modal ────────────────────────────────────────── */}
      {magicLinkUrl && (
        <MagicLinkModal
          url={magicLinkUrl}
          scope="ticket"
          ticketId={ticket.id.slice(0, 8)}
          onClose={() => setMagicLinkUrl(null)}
          onRegenerate={async () => {
            const data = await ticketsApi.magicLinkFor(id);
            setMagicLinkUrl(data.url);
          }}
        />
      )}

      {/* ── Custom confirm modal ────────────────────────────────────── */}
      {confirmOpts && (
        <ConfirmModal
          {...confirmOpts}
          onConfirm={confirmOpts.onConfirm}
          onCancel={() => setConfirmOpts(null)}
        />
      )}

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
            onDelete={() => {
              setSheetOpen(false);
              showConfirm(
                { title: 'Delete this ticket?', body: 'All messages and attachments will be permanently erased. This cannot be undone.', confirmLabel: 'Delete permanently', danger: true },
                () => { deleteM.mutate(); setConfirmOpts(null); }
              );
            }}
            transitionPending={transitionM.isPending}
            magicPending={magicM.isPending}
            deletePending={deleteM.isPending}
            editingProps={editingProps}
            onToggleEdit={() => setEditingProps((v) => !v)}
            onSaveEdit={(p) => patchM.mutate(p)}
            savePending={patchM.isPending}
          />
        ) : (
          <ReadOnlyProps ticket={ticket} />
        )}
      </div>
      <BottomTabBar active="tickets" />
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
  onDelete,
  transitionPending,
  magicPending,
  deletePending,
  editingProps,
  onToggleEdit,
  onSaveEdit,
  savePending,
}: {
  ticket: TicketResponse;
  notes: InternalNote[];
  can: TransitionAbility;
  transition: (k: 'ack' | 'pend' | 'close') => void;
  shareMagicLink: () => void;
  onDelete: () => void;
  transitionPending: boolean;
  magicPending: boolean;
  deletePending: boolean;
  editingProps: boolean;
  onToggleEdit: () => void;
  onSaveEdit: (p: PatchTicketPayload) => void;
  savePending: boolean;
}) {
  if (editingProps) {
    return (
      <EditPropsPanel
        ticket={ticket}
        onCancel={onToggleEdit}
        onSave={onSaveEdit}
        savePending={savePending}
      />
    );
  }

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
            onClick={() => transition('close')}
          >
            <span>Close ticket</span>
            <span className="arr">→ closed</span>
          </button>
        </div>
      </div>

      <div className="lg-props__sec">
        <div className="lg-props__lbl" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <span>Properties</span>
          <button
            type="button"
            onClick={onToggleEdit}
            style={{
              fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.14em',
              textTransform: 'uppercase', background: 'none', border: '1px solid var(--rule)',
              padding: '2px 8px', cursor: 'pointer', color: 'var(--mid)',
            }}
          >
            Edit ⊙
          </button>
        </div>
        <div className="lg-props__kv">
          <div className="row"><span className="k">Priority</span><span className="v italic">{ticket.priority}</span></div>
          <div className="row"><span className="k">Category</span><span className="v italic">{ticket.category ?? '—'}</span></div>
          <div className="row"><span className="k">Type</span><span className="v">{TICKET_TYPE_LABEL[ticket.ticket_type]}</span></div>
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
            <span className="arr">→ QR + copy</span>
          </button>
        </div>
      </div>

      <div className="lg-props__sec">
        <div className="lg-props__lbl">Danger zone</div>
        <div className="lg-props__actions">
          <button
            type="button"
            className="lg-props__act is-danger"
            disabled={deletePending}
            onClick={onDelete}
          >
            <span>{deletePending ? 'Deleting…' : 'Delete ticket'}</span>
            <span className="arr">permanent</span>
          </button>
        </div>
      </div>
    </>
  );
}

// ── Edit-properties panel ────────────────────────────────────────────────────
