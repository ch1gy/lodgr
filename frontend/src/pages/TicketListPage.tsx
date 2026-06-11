// ─────────────────────────────────────────────────────────────────────────────
// TicketListPage.tsx — the editorial queue.
//
// Data path:
//   • useQuery(['tickets', page, limit]) → GET /tickets?page=&limit=
//   • Auto-polled every 30s (per handoff — no SSE yet, polling is pragmatic).
//   • Local "status" filter ('all' | TicketStatus) applied client-side; the
//     backend doesn't filter by status, so we fetch everything and slice.
//
// Routing:
//   • Each row is a <Link to={`/tickets/${id}`}> for keyboard + cmd-click.
//
// Role:
//   • Desk sees aggregate KPIs + swipe-to-triage + sort control.
//   • Clients see a quieter headline ("Your tickets · N open").
// ─────────────────────────────────────────────────────────────────────────────

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { flushSync } from 'react-dom';
import { Link, useNavigate } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { tickets as ticketsApi } from '../api/tickets';
import type { TicketResponse, TicketStatus } from '../api/types';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { StatusPill } from '../components/StatusPill';
import { CreateTicketModal } from '../components/CreateTicketModal';
import { Segmented } from '../components/Segmented';
import { SlaOdometer } from '../components/SlaOdometer';
import { CountUp } from '../components/CountUp';
import { DraggableRow } from '../components/DraggableRow';
import { useAuth } from '../auth/AuthContext';
import { useFlip } from '../hooks/useFlip';
import { useMorph, type DocumentWithVT } from '../theme/MorphContext';
import { timeAgo, TICKET_TYPE_LABEL } from '../utils/format';

// Applies a view-transition-name inline style. Casting needed — the property
// is not yet in TypeScript's React.CSSProperties definitions.
const vt = (name: string): React.CSSProperties =>
  ({ viewTransitionName: name } as unknown as React.CSSProperties);
import '../styles/list.css';
import '../styles/palette.css';

type Filter  = 'all' | TicketStatus;
type SortKey = 'newest' | 'sla' | 'status' | 'priority';

const SORT_LABELS: Record<SortKey, string> = {
  newest:   'Newest',
  sla:      'Time Left',
  status:   'Status',
  priority: 'Priority',
};

const LABEL_TO_KEY: Record<string, SortKey> = Object.fromEntries(
  Object.entries(SORT_LABELS).map(([k, v]) => [v, k as SortKey])
);

const STATUS_ORDER: Record<string, number> = {
  open: 0, acknowledged: 1, pending: 2, closed: 3,
};
const PRIORITY_ORDER: Record<string, number> = {
  urgent: 0, high: 1, medium: 2, low: 3,
};

function clientInitials(id: string): string {
  return id.replace(/[^a-z0-9]/gi, '').slice(0, 2).toUpperCase() || '??';
}

function sortTickets(list: TicketResponse[], key: SortKey): TicketResponse[] {
  return [...list].sort((a, b) => {
    switch (key) {
      case 'newest':
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      case 'sla': {
        const now = Date.now();
        const aMs = a.due_date ? new Date(a.due_date).getTime() - now : Infinity;
        const bMs = b.due_date ? new Date(b.due_date).getTime() - now : Infinity;
        return aMs - bMs;
      }
      case 'status':
        return (STATUS_ORDER[a.status] ?? 99) - (STATUS_ORDER[b.status] ?? 99);
      case 'priority':
        return (PRIORITY_ORDER[a.priority] ?? 99) - (PRIORITY_ORDER[b.priority] ?? 99);
    }
  });
}

// ── Skeleton row ──────────────────────────────────────────────────────────
function SkeletonRow({ i }: { i: number }) {
  return (
    <div
      className="lg-row lg-row--skel"
      style={{ '--i': i } as React.CSSProperties}
      aria-hidden
    >
      <div className="skel" style={{ width: 30, height: 28 }} />
      <div className="lg-row__client">
        <div className="skel" style={{ width: 28, height: 28, flexShrink: 0 }} />
        <div className="skel" style={{ width: 60, height: 10 }} />
      </div>
      <div className="lg-row__title-blk">
        <div className="skel" style={{ width: '38%', height: 9, marginBottom: 6 }} />
        <div className="skel" style={{ width: '72%', height: 20, marginBottom: 5 }} />
        <div className="skel" style={{ width: '28%', height: 9 }} />
      </div>
      <div className="skel" style={{ width: 80, height: 22, alignSelf: 'center' }} />
      <div className="skel" style={{ width: 90, height: 22, alignSelf: 'center' }} />
      <div className="skel" style={{ width: 16, height: 16, alignSelf: 'center' }} />
    </div>
  );
}

// ── Component ────────────────────────────────────────────────────────────

export function TicketListPage() {
  const { isDesk } = useAuth();
  const navigate = useNavigate();
  const { morphId, setMorphId } = useMorph();

  const [filter,     setFilter]     = useState<Filter>('all');
  const [search,     setSearch]     = useState('');
  const [sort,       setSort]       = useState<SortKey>('newest');
  const [createOpen, setCreateOpen] = useState(false);
  const [page,       setPage]       = useState(1);
  const [newCount,   setNewCount]   = useState(0);
  const [newIds,     setNewIds]     = useState<Set<string>>(new Set());

  // ── Bulk triage ─────────────────────────────────────────────────────────
  const [selected,   setSelected]   = useState<Set<string>>(new Set());
  const qc = useQueryClient();

  const bulkAckM = useMutation({
    mutationFn: (ids: string[]) => Promise.all(ids.map((id) => ticketsApi.ack(id))),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['tickets'] });
      setSelected(new Set());
    },
  });
  const bulkCloseM = useMutation({
    mutationFn: (ids: string[]) => Promise.all(ids.map((id) => ticketsApi.close(id))),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['tickets'] });
      setSelected(new Set());
    },
  });

  const toggleSelect = useCallback((id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }, []);

  const isSelecting = selected.size > 0;

  const LIMIT = 50;

  // ── FLIP sort ───────────────────────────────────────────────────────────
  const { capture, play }   = useFlip();
  const rowEls              = useRef<Map<string, HTMLElement>>(new Map());
  const pendingFlip         = useRef(false);

  // ── New-arrived tracking ────────────────────────────────────────────────
  const knownIds    = useRef<Set<string> | null>(null);
  const isFirstLoad = useRef(true);

  const query = useQuery({
    queryKey: ['tickets', page, LIMIT],
    queryFn:  () => ticketsApi.list(page, LIMIT),
    refetchInterval:      30_000,
    refetchOnWindowFocus: false,
  });

  const all: TicketResponse[] = useMemo(
    () => query.data?.tickets ?? [],
    [query.data?.tickets]
  );

  // Detect newly-arrived tickets on each poll refetch
  useEffect(() => {
    if (query.isLoading || !query.data) return;
    if (isFirstLoad.current) {
      isFirstLoad.current = false;
      knownIds.current = new Set(all.map((t) => t.id));
      return;
    }
    const arrived = all.filter((t) => !knownIds.current!.has(t.id));
    if (arrived.length > 0) {
      setNewIds((prev) => new Set([...prev, ...arrived.map((t) => t.id)]));
      setNewCount((prev) => prev + arrived.length);
      arrived.forEach((t) => knownIds.current!.add(t.id));
    }
  }, [all, query.isLoading, query.data]);

  // ── Counts ─────────────────────────────────────────────────────────────
  const counts = useMemo(() => {
    const c = { all: all.length, open: 0, acknowledged: 0, pending: 0, closed: 0 };
    for (const t of all) c[t.status as keyof typeof c]++;
    return c;
  }, [all]);

  const urgentCount = useMemo(
    () => all.filter((t) => t.priority === 'urgent' && t.status !== 'closed').length,
    [all]
  );

  // ── Filtered + sorted list ──────────────────────────────────────────────
  const filtered = useMemo(() => {
    let list = filter === 'all' ? all : all.filter((t) => t.status === filter);
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter((t) =>
        t.title.toLowerCase().includes(q) ||
        t.id.toLowerCase().includes(q) ||
        (t.category ?? '').toLowerCase().includes(q) ||
        (t.sub_client_name ?? '').toLowerCase().includes(q)
      );
    }
    return list;
  }, [all, filter, search]);

  const visible = useMemo(() => sortTickets(filtered, sort), [filtered, sort]);

  // Play FLIP after sort state update lands in the DOM
  useLayoutEffect(() => {
    if (!pendingFlip.current) return;
    pendingFlip.current = false;
    play((id) => rowEls.current.get(id) ?? null, visible.map((t) => t.id));
  }, [sort]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleSort = (label: string) => {
    const key = LABEL_TO_KEY[label] ?? 'newest';
    if (key === sort) return;
    // Capture "First" positions before state update
    for (const [id, el] of rowEls.current) capture(id, el);
    pendingFlip.current = true;
    setSort(key);
  };

  const totalPages = Math.ceil((query.data?.total ?? 0) / LIMIT);

  // ── VT row-click: intercepts primary clicks to morph into detail (§6.2) ──
  const handleRowClick = useCallback((e: React.MouseEvent, ticketId: string) => {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const doc = document as DocumentWithVT;
    if (typeof doc.startViewTransition !== 'function') return;
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return;
    e.preventDefault();
    // Apply morphId synchronously so the named elements exist in the "before" snapshot
    flushSync(() => setMorphId(ticketId));
    doc.startViewTransition(() => {
      flushSync(() => navigate(`/tickets/${ticketId}`));
    });
    // Clear after transition completes
    setTimeout(() => setMorphId(null), 700);
  }, [navigate, setMorphId]);

  const headline = isDesk ? 'The desk' : 'Your tickets';
  const sub = isDesk
    ? `${counts.open + counts.acknowledged} open conversations · ${urgentCount} flagged urgent`
    : `${counts.open + counts.acknowledged} open · ${counts.closed} closed`;

  // ── Render ──────────────────────────────────────────────────────────────
  return (
    <div className="lg-list grain sig-stage">
      <Masthead active="tickets" />

      <div className="lg-list__body">
        {/* ── Headline strip ─────────────────────────────────────────── */}
        <div className="lg-list__head">
          <div>
            <h1 className="lg-list__title press">
              {headline}
              <span className="lg-list__title-count">
                / <CountUp value={counts.all} pad={3} />
              </span>
            </h1>
            <div className="lg-list__sub">{sub}</div>
          </div>
          {isDesk && (
            <div className="lg-list__head-meta">
              <div className="lg-list__kpi">
                <span className="red ink-red">
                  <CountUp value={urgentCount} pad={2} />
                </span>{' '}urgent
              </div>
              <div className="lg-list__kpi-lbl">due in &lt; 48 hrs</div>
            </div>
          )}
        </div>

        {/* ── Filter + sort bar ──────────────────────────────────────── */}
        <div className="lg-list__filt">
          <div className="lg-list__filt-group">
            {(['all', 'open', 'acknowledged', 'pending', 'closed'] as Filter[]).map((f) => (
              <button
                key={f}
                type="button"
                className={'lg-list__filt-tab' + (filter === f ? ' is-active' : '')}
                onClick={() => { setFilter(f); setPage(1); }}
              >
                {f === 'all' ? 'All' : f[0].toUpperCase() + f.slice(1)}{' '}
                <b>{f === 'all' ? counts.all : counts[f as TicketStatus]}</b>
              </button>
            ))}
          </div>
          <div className="lg-list__filt-spacer" />

          {isDesk && (
            <Segmented
              options={['Newest', 'Time Left', 'Status', 'Priority']}
              value={SORT_LABELS[sort]}
              onChange={handleSort}
              className="lg-list__sort"
            />
          )}

          <input
            className="lg-list__filt-search"
            type="search"
            placeholder="/ search tickets"
            value={search}
            onChange={(e) => { setSearch(e.target.value); setPage(1); }}
            aria-label="Search tickets"
            style={{
              background: 'none', border: 'none',
              borderBottom: '1px dashed var(--rule)', outline: 'none',
              fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '.12em',
              color: 'var(--mid)', padding: '4px 0', width: 160, marginLeft: 16,
            }}
          />

          {query.isFetching && !query.isLoading && (
            <span className="lg-list__filt-meta">syncing…</span>
          )}

          {totalPages > 1 && (
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '.10em', marginLeft: 12 }}>
              <button
                type="button"
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                disabled={page <= 1}
                style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--mid)', fontSize: 12 }}
              >←</button>
              <span style={{ color: 'var(--mid)' }}>{page}/{totalPages}</span>
              <button
                type="button"
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                disabled={page >= totalPages}
                style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--mid)', fontSize: 12 }}
              >→</button>
            </div>
          )}

          <button
            type="button"
            className="lg-list__filt-new"
            onClick={() => setCreateOpen(true)}
          >
            + New ticket
          </button>
        </div>

        {/* ── New-arrived pill ───────────────────────────────────────── */}
        {newCount > 0 && (
          <div className="lg-list__arrived">
            <button
              type="button"
              className="lg-list__arrived-pill"
              onClick={() => { setNewCount(0); setNewIds(new Set()); }}
            >
              ↓ {newCount} new ticket{newCount !== 1 ? 's' : ''} arrived
            </button>
          </div>
        )}

        {/* ── Rows ───────────────────────────────────────────────────── */}
        <div className={`lg-list__rows scroll-area${isSelecting ? ' is-selecting' : ''}`}>

          {/* Loading: skeleton rows */}
          {query.isLoading && (
            Array.from({ length: 6 }, (_, i) => <SkeletonRow key={i} i={i} />)
          )}

          {/* Error: editorial voice + retry */}
          {query.isError && (
            <div className="lg-list__state">
              <h3>Couldn't load tickets.</h3>
              <p>{(query.error as Error)?.message ?? 'Network error — try again.'}</p>
              <button
                type="button"
                className="lg-bt lg-bt--ghost"
                style={{ marginTop: 24 }}
                onClick={() => query.refetch()}
              >
                Try again <span className="arr">→</span>
              </button>
            </div>
          )}

          {/* Empty: editorial voice + recovery action */}
          {!query.isLoading && !query.isError && visible.length === 0 && (
            <div className="lg-list__state">
              <h3>
                {filter === 'all'
                  ? 'Nothing in the queue.'
                  : `No ${filter} tickets.`}
              </h3>
              <p>
                {filter === 'all' && search.trim()
                  ? 'No tickets match that search.'
                  : filter === 'all'
                  ? "You're all caught up. Pour another cup."
                  : 'Try a different filter — or raise a new one.'}
              </p>
              {filter !== 'all' && (
                <button
                  type="button"
                  className="lg-bt lg-bt--text"
                  style={{ marginTop: 16 }}
                  onClick={() => setFilter('all')}
                >
                  Show all tickets
                </button>
              )}
              {isDesk && filter === 'all' && !search.trim() && (
                <button
                  type="button"
                  className="lg-bt lg-bt--ghost"
                  style={{ marginTop: 20 }}
                  onClick={() => setCreateOpen(true)}
                >
                  + New ticket <span className="arr">→</span>
                </button>
              )}
            </div>
          )}

          {/* Ticket rows */}
          {visible.map((t, i) => {
            const isHot   = t.priority === 'urgent' && t.status !== 'closed';
            const isNew   = newIds.has(t.id);
            const isMorph = morphId === t.id;
            const isChecked = selected.has(t.id);
            const canSelect = isDesk && t.status !== 'closed';

            const rowLink = (
              <Link
                to={`/tickets/${t.id}`}
                className={
                  'lg-row' +
                  (isHot     ? ' is-hot'     : '') +
                  (isNew     ? ' is-new'     : '') +
                  (isChecked ? ' is-checked' : '')
                }
                style={{ '--i': i } as React.CSSProperties}
                onClick={(e) => {
                  // If a checkbox is being used, don't navigate
                  if (isSelecting && canSelect) { e.preventDefault(); toggleSelect(t.id); return; }
                  handleRowClick(e, t.id);
                }}
              >
                {canSelect && (
                  <div className={`lg-row__sel${isSelecting ? ' forced' : ''}`} onClick={(e) => { e.preventDefault(); e.stopPropagation(); toggleSelect(t.id); }}>
                    <input
                      type="checkbox"
                      className="lg-row__selck"
                      checked={isChecked}
                      onChange={() => toggleSelect(t.id)}
                      aria-label={`Select ticket ${t.id.slice(0, 8)}`}
                    />
                  </div>
                )}
                <div className="lg-row__num">{String(i + 1).padStart(3, '0')}</div>
                <div className="lg-row__client">
                  <span
                    className="lg-row__av"
                    style={isMorph ? vt('sig-avatar') : undefined}
                  >
                    {clientInitials(t.client_id)}
                  </span>
                  <span className="lg-row__client-nm">{t.client_id.slice(0, 8)}</span>
                </div>
                <div className="lg-row__title-blk">
                  <div className="lg-row__cat">
                    {t.category ?? TICKET_TYPE_LABEL[t.ticket_type]} · {t.id.slice(0, 8)}
                    {t.sub_client_name && (
                      <span style={{ marginLeft: 8, color: 'var(--red)', fontFamily: 'var(--mono)', fontSize: 9 }}>
                        › {t.sub_client_name}
                      </span>
                    )}
                  </div>
                  <div
                    className="lg-row__title"
                    style={isMorph ? vt('sig-headline') : undefined}
                  >
                    {t.title}
                  </div>
                  <div className="lg-row__meta">
                    <span>opened <b>{timeAgo(t.created_at)}</b></span>
                    {t.recurring && (
                      <><span>·</span><span style={{ color: 'var(--red)' }}>recurring</span></>
                    )}
                  </div>
                </div>
                <StatusPill status={t.status} />
                <span className="lg-row__sla" style={isMorph ? vt('sig-clock') : undefined}>
                  <SlaOdometer
                    dueDate={t.due_date}
                    estimatedCompletion={t.estimated_completion}
                  />
                </span>
                <div className="lg-row__chevron">→</div>
              </Link>
            );

            return (
              <div
                key={t.id}
                ref={(el) => {
                  if (el) rowEls.current.set(t.id, el);
                  else    rowEls.current.delete(t.id);
                }}
              >
                {isDesk && t.status !== 'closed' ? (
                  <DraggableRow id={t.id} status={t.status}>
                    {rowLink}
                  </DraggableRow>
                ) : rowLink}
              </div>
            );
          })}
        </div>
      </div>

      {/* FAB — mobile only (CSS hides on desktop) */}
      <button
        className="lg-fab"
        aria-label="Open new ticket"
        onClick={() => setCreateOpen(true)}
      >+</button>

      {/* Bulk triage action bar — fixed bottom, springs up when rows selected */}
      {isSelecting && isDesk && (
        <div className="lg-tribar" role="toolbar" aria-label="Bulk triage">
          <span className="lg-tribar__count">{selected.size} selected</span>
          <button
            type="button"
            className="lg-tribar__btn"
            disabled={bulkAckM.isPending || bulkCloseM.isPending}
            onClick={() => bulkAckM.mutate([...selected].filter((id) => {
              const t = all.find((x) => x.id === id);
              return t?.status === 'open';
            }))}
          >
            ✓ Acknowledge
          </button>
          <button
            type="button"
            className="lg-tribar__btn lg-tribar__btn--danger"
            disabled={bulkAckM.isPending || bulkCloseM.isPending}
            onClick={() => bulkCloseM.mutate([...selected])}
          >
            × Close all
          </button>
          <button
            type="button"
            className="lg-tribar__dismiss"
            onClick={() => setSelected(new Set())}
            aria-label="Deselect all"
          >✕ Deselect</button>
        </div>
      )}

      <BottomTabBar active="tickets" />

      {createOpen && <CreateTicketModal onClose={() => setCreateOpen(false)} />}
    </div>
  );
}
