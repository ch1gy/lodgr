// ─────────────────────────────────────────────────────────────────────────────
// TicketListPage.tsx — the editorial queue.
//
// Data path:
//   • useQuery(['tickets', page, limit]) → GET /tickets?page=&limit=
//   • Auto-polled every 30s (per handoff — no SSE yet, polling is pragmatic).
//   • Local "status" filter ('all' | TicketStatus) applied client-side; the
//     backend doesn't filter by status, so we fetch everything and slice.
//     Swap to a server query string the day the backend adds the param.
//
// Routing:
//   • Each row is a <Link to={`/tickets/${id}`}> for keyboard + cmd-click.
//
// Role:
//   • Desk sees aggregate KPIs ("urgent in <48h"). Clients see a quieter
//     headline ("Your tickets · N open"). The headline meta auto-shrinks
//     for clients since their counts are smaller.
// ─────────────────────────────────────────────────────────────────────────────

import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { tickets as ticketsApi } from '../api/tickets';
import type { TicketResponse, TicketStatus } from '../api/types';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { StatusPill } from '../components/StatusPill';
import { PriorityBars } from '../components/PriorityBars';
import { CreateTicketModal } from '../components/CreateTicketModal';
import { useAuth } from '../auth/AuthContext';
import { timeAgo, daysUntil, TICKET_TYPE_LABEL } from '../utils/format';
import '../styles/list.css';

type Filter = 'all' | TicketStatus;

// ── Helpers ──────────────────────────────────────────────────────────────

/** Render a due-cell value + small label, marking overdue/today as urgent. */
function dueParts(dueDate: string | null): { v: string; lbl: string; urgent: boolean } {
  const d = daysUntil(dueDate);
  if (d === null) return { v: '—', lbl: 'no due date', urgent: false };
  if (d < 0)  return { v: 'Overdue', lbl: `${Math.abs(d)}d ago`, urgent: true };
  if (d === 0) return { v: 'Today', lbl: 'due today', urgent: true };
  if (d === 1) return { v: 'Tomorrow', lbl: '1 day', urgent: false };
  return { v: `${d}d`, lbl: 'remaining', urgent: false };
}

/** UUID-based avatar initials — first two alphanumeric chars uppercased. */
function clientInitials(id: string): string {
  return id.replace(/[^a-z0-9]/gi, '').slice(0, 2).toUpperCase() || '??';
}

// ── Component ────────────────────────────────────────────────────────────

export function TicketListPage() {
  const { isDesk } = useAuth();
  const [filter, setFilter]       = useState<Filter>('all');
  const [createOpen, setCreateOpen] = useState(false);
  const [page] = useState(1);
  const LIMIT = 50;

  const query = useQuery({
    queryKey: ['tickets', page, LIMIT],
    queryFn: () => ticketsApi.list(page, LIMIT),
    // 30s polling per the handoff — fine until the backend ships SSE.
    refetchInterval: 30_000,
    // Don't refetch on window focus — too aggressive for a long-running desk.
    refetchOnWindowFocus: false,
  });

  // Stable reference so the dependent useMemos don't churn on every render.
  const all: TicketResponse[] = useMemo(
    () => query.data?.tickets ?? [],
    [query.data?.tickets]
  );

  // ── Counts per status ────────────────────────────────────────────────
  const counts = useMemo(() => {
    const c = { all: all.length, open: 0, acknowledged: 0, pending: 0, closed: 0 };
    for (const t of all) c[t.status]++;
    return c;
  }, [all]);

  const urgentCount = useMemo(
    () => all.filter((t) => t.priority === 'urgent' && t.status !== 'closed').length,
    [all]
  );

  // ── Client-side filter ───────────────────────────────────────────────
  const visible = useMemo(
    () => (filter === 'all' ? all : all.filter((t) => t.status === filter)),
    [all, filter]
  );

  // ── Headline copy — desk vs client ───────────────────────────────────
  const headline = isDesk ? 'The desk' : 'Your tickets';
  const sub = isDesk
    ? `${counts.open + counts.acknowledged} open conversations · ${urgentCount} flagged urgent`
    : `${counts.open + counts.acknowledged} open · ${counts.closed} closed`;

  return (
    <div className="lg-list grain">
      <Masthead active="tickets" />

      <div className="lg-list__body">
        <div className="lg-list__head">
          <div>
            <h1 className="lg-list__title">
              {headline}
              <span className="lg-list__title-count">/ {String(counts.all).padStart(3, '0')}</span>
            </h1>
            <div className="lg-list__sub">{sub}</div>
          </div>
          {isDesk && (
            <div className="lg-list__head-meta">
              <div className="lg-list__kpi">
                <span className="red">{String(urgentCount).padStart(2, '0')}</span> urgent
              </div>
              <div className="lg-list__kpi-lbl">due in &lt; 48 hrs</div>
            </div>
          )}
        </div>

        {/* ── Filter bar ─────────────────────────────────────────────── */}
        <div className="lg-list__filt">
          <div className="lg-list__filt-group">
            {(['all', 'open', 'acknowledged', 'pending', 'closed'] as Filter[]).map((f) => (
              <button
                key={f}
                type="button"
                className={'lg-list__filt-tab' + (filter === f ? ' is-active' : '')}
                onClick={() => setFilter(f)}
              >
                {f === 'all' ? 'All' : f[0].toUpperCase() + f.slice(1)}{' '}
                <b>{f === 'all' ? counts.all : counts[f]}</b>
              </button>
            ))}
          </div>
          <div className="lg-list__filt-spacer" />
          <span className="lg-list__filt-search">/ search the desk</span>
          {query.isFetching && !query.isLoading && (
            <span className="lg-list__filt-meta">syncing…</span>
          )}
          <button
            type="button"
            className="lg-list__filt-new"
            onClick={() => setCreateOpen(true)}
          >
            + New ticket
          </button>
        </div>

        {/* ── Rows / states ─────────────────────────────────────────── */}
        <div className="lg-list__rows">
          {query.isLoading && (
            <div className="lg-list__state">
              <h3>Loading the desk…</h3>
              Fetching tickets from the backend.
            </div>
          )}

          {query.isError && (
            <div className="lg-list__state">
              <h3>Couldn't load tickets.</h3>
              {(query.error as Error)?.message ?? 'Network error — try again.'}
            </div>
          )}

          {!query.isLoading && !query.isError && visible.length === 0 && (
            <div className="lg-list__state">
              <h3>Nothing in the queue.</h3>
              {filter === 'all'
                ? 'You\'re all caught up. Pour another cup.'
                : `No ${filter} tickets right now.`}
            </div>
          )}

          {visible.map((t, i) => {
            const due = dueParts(t.due_date);
            const isHot = t.priority === 'urgent' && t.status !== 'closed';
            return (
              <Link
                key={t.id}
                to={`/tickets/${t.id}`}
                className={'lg-row' + (isHot ? ' is-hot' : '')}
                style={{ ['--i' as string]: i }}
              >
                <div className="lg-row__num">{String(i + 1).padStart(3, '0')}</div>
                <div className="lg-row__client">
                  <span className="lg-row__av">{clientInitials(t.client_id)}</span>
                  <span className="lg-row__client-nm">
                    {t.client_id.slice(0, 8)}
                  </span>
                </div>
                <div className="lg-row__title-blk">
                  <div className="lg-row__cat">
                    {t.category ?? TICKET_TYPE_LABEL[t.ticket_type]} · {t.id.slice(0, 8)}
                  </div>
                  <div className="lg-row__title">{t.title}</div>
                  <div className="lg-row__meta">
                    <span>opened <b>{timeAgo(t.created_at)}</b></span>
                    {t.recurring && (
                      <>
                        <span>·</span>
                        <span style={{ color: 'var(--red)' }}>recurring</span>
                      </>
                    )}
                  </div>
                </div>
                <StatusPill status={t.status} />
                <PriorityBars priority={t.priority} />
                <div className={'lg-row__due' + (due.urgent ? ' is-urgent' : '')}>
                  <span className="lg-row__due-v">{due.v}</span>
                  {due.lbl}
                </div>
                <div className="lg-row__chevron">→</div>
              </Link>
            );
          })}
        </div>
      </div>

      {/* FAB — mobile only (CSS hides on desktop) */}
      <button className="lg-fab" aria-label="Open new ticket" onClick={() => setCreateOpen(true)}>+</button>

      <BottomTabBar active="tickets" />

      {createOpen && <CreateTicketModal onClose={() => setCreateOpen(false)} />}
    </div>
  );
}
