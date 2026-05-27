// ─────────────────────────────────────────────────────────────────────────────
// CreateTicketModal.tsx — "Open a new ticket" modal.
//
// Wires to POST /tickets. On success the new ticket is navigated to and the
// ticket list cache is invalidated.
//
// Role note: both desk and client can create tickets. Desk users must pick a
// client from the selector — the chosen client_id is sent to the backend which
// validates the target is an active client-role user.
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { tickets as ticketsApi } from '../api/tickets';
import { admin } from '../api/admin';
import type { Client, TicketPriority, TicketType } from '../api/types';
import { useAuth } from '../auth/AuthContext';
import '../styles/v2.css';

interface Props {
  onClose: () => void;
}

const PRIORITIES: TicketPriority[] = ['low', 'medium', 'high', 'urgent'];
const TICKET_TYPES: TicketType[]   = ['standard', 'maintenance', 'security_log'];
const TYPE_LABEL: Record<TicketType, string> = {
  standard: 'Standard',
  maintenance: 'Maintenance',
  security_log: 'Security log',
};

export function CreateTicketModal({ onClose }: Props) {
  const nav = useNavigate();
  const qc  = useQueryClient();
  const { user, isDesk } = useAuth();

  const [title, setTitle]             = useState('');
  const [description, setDescription] = useState('');
  const [priority, setPriority]       = useState<TicketPriority>('medium');
  const [ticketType, setTicketType]   = useState<TicketType>('standard');
  const [category, setCategory]       = useState('');
  const [dueDate, setDueDate]         = useState('');
  const [recurring, setRecurring]     = useState(false);
  const [interval, setInterval]       = useState('30');

  // Desk-only: client selector
  const [clientSearch, setClientSearch]       = useState('');
  const [selectedClient, setSelectedClient]   = useState<Client | null>(null);
  const [clientDropOpen, setClientDropOpen]   = useState(false);
  const clientDropRef = useRef<HTMLDivElement>(null);

  // Close the dropdown when the user clicks outside the entire client field.
  useEffect(() => {
    if (!clientDropOpen) return;
    function handleOutside(e: MouseEvent) {
      if (clientDropRef.current && !clientDropRef.current.contains(e.target as Node)) {
        setClientDropOpen(false);
      }
    }
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [clientDropOpen]);

  const clientsQ = useQuery({
    queryKey: ['clients'],
    queryFn: () => admin.listClients(),
    enabled: isDesk,
  });

  const filteredClients = useMemo(() => {
    const q = clientSearch.toLowerCase();
    return (clientsQ.data ?? [])
      .filter((c) => !c.deleted_at)
      .filter((c) => !q || c.name.toLowerCase().includes(q) || c.email.toLowerCase().includes(q));
  }, [clientsQ.data, clientSearch]);

  const createM = useMutation({
    mutationFn: () =>
      ticketsApi.create({
        title: title.trim(),
        description: description.trim(),
        priority,
        ticket_type: ticketType,
        category: category.trim() || undefined,
        due_date: dueDate || undefined,
        recurring,
        recurring_interval_days: recurring ? Number(interval) : undefined,
        client_id: selectedClient?.id,
      }),
    onSuccess: (data) => {
      void qc.invalidateQueries({ queryKey: ['tickets'] });
      onClose();
      nav(`/tickets/${data.id}`);
    },
  });

  const err = createM.error
    ? ((createM.error as { response?: { data?: { error?: string } } })
        .response?.data?.error ?? 'Something went wrong. Try again.')
    : null;

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim() || !description.trim()) return;
    if (isDesk && !selectedClient) return;
    createM.mutate();
  }

  const now = new Date();
  const filedAs = user?.email ?? 'you';
  const filedDate = now.toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: 'numeric' });

  return (
    <div className="lg-ov" role="dialog" aria-modal aria-label="Open a new ticket">
      <div className="lg-mdl create">
        {/* ── Header ──────────────────────────────────────────────── */}
        <div className="lg-mdl__top">
          <span className="lg-mdl__eye">— Section 02 — Open a new ticket</span>
          <span className="lg-mdl__no"><i>Lodgr</i><span className="dot">.</span></span>
          <button type="button" className="lg-mdl__x" onClick={onClose}>Close ✕</button>
        </div>

        {/* ── Body ────────────────────────────────────────────────── */}
        <form id="create-ticket-form" onSubmit={handleSubmit}>
          <div className="lg-mdl__body">
            <div className="lg-mdl__h1">A fresh entry <em>for the desk.</em></div>
            <div className="lg-mdl__dek">
              Tickets are filed by the desk on behalf of a client, or by the client directly.
              Once submitted, it lands in the queue immediately.
            </div>
            <div className="lg-mdl__rule" />

            <div className="lg-f-grid">
              <div className="lg-f full">
                <div className="lg-f__lbl"><span>Title</span><span className="req">Required</span></div>
                <input
                  className="lg-f__inp"
                  placeholder="e.g. Outlook calendar invites not syncing to iPhones"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  maxLength={200}
                  required
                  autoFocus
                />
              </div>

              <div className="lg-f full">
                <div className="lg-f__lbl"><span>Description</span><span className="req">Required</span></div>
                <textarea
                  className="lg-f__ta"
                  placeholder="What's happening? The more detail, the faster the desk can act."
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  rows={4}
                  required
                />
                <span className="lg-f__hint">Max 10,000 characters</span>
              </div>

              {isDesk && (
                <div ref={clientDropRef} className="lg-f full">
                  <div className="lg-f__lbl">
                    <span>Client</span>
                    <span className="req">Required</span>
                  </div>
                  {/* Wrap input + dropdown together so top:100% anchors to the input bottom. */}
                  <div style={{ position: 'relative' }}>
                    <input
                      className="lg-f__inp"
                      placeholder="Search by name or email…"
                      value={clientSearch}
                      onChange={(e) => {
                        setClientSearch(e.target.value);
                        setSelectedClient(null);
                        setClientDropOpen(true);
                      }}
                      onFocus={() => setClientDropOpen(true)}
                      autoComplete="off"
                    />
                    {clientDropOpen && filteredClients.length > 0 && !selectedClient && (
                      <div style={{
                        position: 'absolute', top: '100%', left: 0, right: 0, zIndex: 50,
                        border: '1px solid var(--ink)', borderTop: 'none',
                        background: 'var(--cream)', maxHeight: 160, overflowY: 'auto',
                      }}>
                        {filteredClients.map((c) => (
                          <button
                            key={c.id}
                            type="button"
                            style={{
                              display: 'flex', alignItems: 'baseline',
                              justifyContent: 'space-between', gap: 16,
                              width: '100%', padding: '10px 14px',
                              background: 'none', border: 'none',
                              borderBottom: '1px solid var(--rule)', cursor: 'pointer',
                            }}
                            onClick={() => {
                              setSelectedClient(c);
                              setClientSearch(c.name);
                              setClientDropOpen(false);
                            }}
                          >
                            <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 16 }}>
                              {c.name}
                            </span>
                            <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--mid)', flexShrink: 0 }}>
                              {c.email}
                            </span>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                  {selectedClient && (
                    <span className="lg-f__hint">
                      {selectedClient.email} · {selectedClient.id.slice(0, 8)}
                    </span>
                  )}
                </div>
              )}
            </div>

            <div style={{ height: 24 }} />

            <div className="grid-priority">
              <div className="lg-f">
                <div className="lg-f__lbl"><span>Priority</span></div>
                <div className="lg-seg">
                  {PRIORITIES.map((p) => (
                    <button
                      key={p}
                      type="button"
                      className={`lg-seg__o${priority === p ? (p === 'urgent' ? ' on red' : ' on') : ''}`}
                      onClick={() => setPriority(p)}
                    >
                      {p[0].toUpperCase() + p.slice(1)}
                    </button>
                  ))}
                </div>
              </div>

              <div className="lg-f">
                <div className="lg-f__lbl"><span>Ticket type</span></div>
                <div className="lg-seg">
                  {TICKET_TYPES.map((t) => (
                    <button
                      key={t}
                      type="button"
                      className={`lg-seg__o${ticketType === t ? ' on' : ''}`}
                      onClick={() => setTicketType(t)}
                    >
                      {TYPE_LABEL[t]}
                    </button>
                  ))}
                </div>
              </div>

              <div className="lg-f">
                <div className="lg-f__lbl"><span>Category</span><span className="opt">Optional</span></div>
                <input
                  className="lg-f__inp"
                  placeholder="e.g. Mail & Calendar"
                  value={category}
                  onChange={(e) => setCategory(e.target.value)}
                  maxLength={100}
                />
              </div>
            </div>

            <div style={{ height: 24 }} />

            <div className="recurring-row">
              <div className="lg-f">
                <div className="lg-f__lbl"><span>Due date</span><span className="opt">Optional</span></div>
                <input
                  className="lg-f__inp mono"
                  type="date"
                  value={dueDate}
                  onChange={(e) => setDueDate(e.target.value)}
                />
                <span className="lg-f__hint">Leave blank for no deadline</span>
              </div>

              <div className="lg-f" style={{ paddingTop: 4 }}>
                <div className="lg-f__lbl"><span>Recurrence</span><span className="opt">Optional</span></div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 24, paddingTop: 8 }}>
                  <button
                    type="button"
                    className={`lg-ck${recurring ? ' on' : ''}`}
                    onClick={() => setRecurring((v) => !v)}
                  >
                    <span className="lg-ck__b">{recurring ? '✓' : ''}</span>
                    <span className="lg-ck__l">Recurring</span>
                  </button>
                  <div style={{ display: 'flex', alignItems: 'baseline', gap: 10, opacity: recurring ? 1 : 0.35 }}>
                    <span className="lg-f__lbl" style={{ margin: 0 }}>Every</span>
                    <input
                      className="lg-f__inp mono"
                      value={interval}
                      onChange={(e) => setInterval(e.target.value)}
                      disabled={!recurring}
                      style={{ width: 56, textAlign: 'center' }}
                      min={1}
                      type="number"
                    />
                    <span className="lg-f__lbl" style={{ margin: 0 }}>days</span>
                  </div>
                </div>
              </div>
            </div>

            {err && <div className="lg-f__err" style={{ marginTop: 16 }}>{err}</div>}
          </div>
        </form>

        {/* ── Footer ──────────────────────────────────────────────── */}
        <div className="lg-mdl__foot">
          <span className="meta">
            Filing as <b style={{ color: 'var(--ink)' }}>{filedAs}</b> · {filedDate}
          </span>
          <div className="lg-mdl__btns">
            <button type="button" className="lg-bt lg-bt--text" onClick={onClose}>Cancel</button>
            <button
              type="submit"
              form="create-ticket-form"
              className="lg-bt lg-bt--solid"
              disabled={createM.isPending || !title.trim() || !description.trim() || (isDesk && !selectedClient)}
            >
              {createM.isPending ? 'Opening…' : 'Open ticket'} <span className="arr">↗</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
