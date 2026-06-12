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

import { useCallback, useEffect, useMemo, useRef, useState, useId } from 'react';
import { useNavigate } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { tickets as ticketsApi } from '../api/tickets';
import { admin } from '../api/admin';
import type { Client, SubClient, TicketPriority, TicketType } from '../api/types';
import { useAuth } from '../auth/AuthContext';

import '../styles/v2.css';
import '../styles/filing.css';
import { sfx } from '../utils/sfx';
import { extractApiError } from '../utils/format';

type FilingPhase = 'idle' | 'filing' | 'stamping' | 'filed';

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
  const { user, profile, isDesk } = useAuth();

  const [phase, setPhase]       = useState<FilingPhase>('idle');
  const [stampNo, setStampNo]   = useState('');
  const stampRot = useRef(0);
  const sheetRef = useRef<HTMLDivElement>(null);
  const drag = useRef({ startY: 0, startH: 0, on: false });

  function onGrabDown(e: React.PointerEvent) {
    const el = sheetRef.current; if (!el) return;
    drag.current = { startY: e.clientY, startH: el.getBoundingClientRect().height, on: true };
    el.classList.add('is-dragging');
    (e.target as Element).setPointerCapture(e.pointerId);
  }
  function onGrabMove(e: React.PointerEvent) {
    const el = sheetRef.current; if (!el || !drag.current.on) return;
    const h = drag.current.startH + (drag.current.startY - e.clientY);
    el.style.setProperty('--sheet-h', `${Math.min(window.innerHeight * 0.94, Math.max(120, h))}px`);
  }
  function onGrabUp() {
    const el = sheetRef.current; if (!el || !drag.current.on) return;
    drag.current.on = false;
    el.classList.remove('is-dragging');
    const h = el.getBoundingClientRect().height;
    const vh = window.innerHeight;
    if (h < vh * 0.38) { onClose(); return; }
    el.style.setProperty('--sheet-h', h > vh * 0.78 ? '94vh' : '62vh');
  }

  // Close on Escape.
  const stableClose = useCallback(onClose, [onClose]);
  useEffect(() => {
    const handle = (e: KeyboardEvent) => { if (e.key === 'Escape') stableClose(); };
    document.addEventListener('keydown', handle);
    return () => document.removeEventListener('keydown', handle);
  }, [stableClose]);

  const [title, setTitle]             = useState('');
  const [description, setDescription] = useState('');
  const [priority, setPriority]       = useState<TicketPriority>('medium');
  const [ticketType, setTicketType]   = useState<TicketType>('standard');
  const [category, setCategory]       = useState('');
  const [dueDate, setDueDate]         = useState('');
  const [recurring, setRecurring]     = useState(false);
  const [interval, setInterval]       = useState('30');
  const [closeNow, setCloseNow]         = useState(false);
  const [attachment, setAttachment]     = useState<File | null>(null);
  const [subClientId, setSubClientId]   = useState<string>('');
  const fileInputRef = useRef<HTMLInputElement>(null);
  const fileInputId  = useId();

  // Desk-only: client selector
  const [clientSearch, setClientSearch]       = useState('');
  const [selectedClient, setSelectedClient]   = useState<Client | null>(null);
  const [clientDropOpen, setClientDropOpen]   = useState(false);
  const clientDropRef = useRef<HTMLDivElement>(null);

  // Sub-client custom dropdown
  const [subClientDropOpen, setSubClientDropOpen] = useState(false);
  const subClientDropRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    if (!subClientDropOpen) return;
    function handleOutside(e: MouseEvent) {
      if (subClientDropRef.current && !subClientDropRef.current.contains(e.target as Node)) {
        setSubClientDropOpen(false);
      }
    }
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [subClientDropOpen]);

  const clientsQ = useQuery({
    queryKey: ['clients'],
    queryFn: () => admin.listClients(),
    enabled: isDesk,
  });

  const subClientsQ = useQuery({
    queryKey: ['sub-clients', selectedClient?.id],
    queryFn: () => admin.listSubClients(selectedClient!.id),
    enabled: !!selectedClient,
  });
  const subClients: SubClient[] = subClientsQ.data ?? [];

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
        initial_status: isDesk && closeNow ? 'closed' : undefined,
        sub_client_id: subClientId || undefined,
      }),
    onMutate: () => {
      setPhase('filing');
    },
    onSuccess: async (data) => {
      if (attachment) {
        try {
          await ticketsApi.postMessage(data.id, '', attachment);
        } catch {
          // non-fatal — ticket was created, attachment failed
        }
      }
      void qc.invalidateQueries({ queryKey: ['tickets'] });

      // Filing ritual: stamp → fly-up → navigate
      stampRot.current = (Math.random() - 0.5) * 14; // -7° to +7°
      setStampNo(data.id.slice(0, 6).toUpperCase());
      setPhase('stamping');
      sfx.stamp();
      setTimeout(() => {
        setPhase('filed');
        sfx.file();
        setTimeout(() => {
          onClose();
          nav(`/tickets/${data.id}`);
        }, 500);
      }, 640);
    },
    onError: () => {
      setPhase('idle');
    },
  });

  const err = createM.error
    ? extractApiError(createM.error, 'Something went wrong. Try again.')
    : null;

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim() || !description.trim()) return;
    if (isDesk && !selectedClient) return;
    createM.mutate();
  }

  const now = new Date();
  const filedAs = profile?.name ?? profile?.email ?? user?.email ?? 'you';
  const filedDate = now.toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: 'numeric' });

  const mdlClass = [
    'lg-mdl create',
    phase === 'filing' || phase === 'stamping' ? 'is-filing' : '',
    phase === 'filed' ? 'is-filed' : '',
  ].filter(Boolean).join(' ');

  return (
    <div className="lg-ov" role="dialog" aria-modal aria-label="Open a new ticket" onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      {/* Hidden SVG filter for ink-roughen effect on stamp */}
      <svg width="0" height="0" style={{ position: 'absolute', overflow: 'hidden' }} aria-hidden>
        <defs>
          <filter id="inkrough" x="-8%" y="-8%" width="116%" height="116%">
            <feTurbulence type="turbulence" baseFrequency="0.04 0.03" numOctaves="3" seed="5" result="noise" />
            <feDisplacementMap in="SourceGraphic" in2="noise" scale="2.5" xChannelSelector="R" yChannelSelector="G" />
          </filter>
        </defs>
      </svg>
      <div className={mdlClass} ref={sheetRef}>
        {/* ── Filing ritual stamp overlay ──────────────────────────── */}
        {(phase === 'stamping' || phase === 'filed') && (
          <div className="lg-stamp-ov" aria-hidden>
            <div
              className="lg-stamp"
              style={{ '--rot': `${stampRot.current}deg`, filter: 'url(#inkrough)' } as React.CSSProperties}
            >
              <div className="lg-stamp__ring" />
              <span className="lg-stamp__received">Received</span>
              <span className="lg-stamp__no">No.&thinsp;{stampNo}</span>
              <span className="lg-stamp__date">{filedDate}</span>
            </div>
          </div>
        )}

        {/* ── Grab handle (mobile sheet only) ─────────────────── */}
        <div className="lg-mdl__grab" onPointerDown={onGrabDown} onPointerMove={onGrabMove} onPointerUp={onGrabUp} onPointerCancel={onGrabUp}>
          <span className="lg-mdl__grab-bar" />
        </div>

        {/* ── Header ──────────────────────────────────────────────── */}
        <div className="lg-mdl__top">
          <span className="lg-mdl__eye">— Open a ticket</span>
          <span className="lg-mdl__no"><i>Lodgr</i><span className="dot">.</span></span>
          <button type="button" className="lg-mdl__x" onClick={onClose}>Close ✕</button>
        </div>

        {/* ── Body ────────────────────────────────────────────────── */}
        <form id="create-ticket-form" onSubmit={handleSubmit} onFocusCapture={() => {
          if (window.innerWidth <= 540 && (window.visualViewport?.height ?? 800) < 600)
            sheetRef.current?.style.setProperty('--sheet-h', '94vh');
        }}>
          <div className="lg-mdl__body">
            <div className="lg-mdl__h1">A fresh <em className="m-only">entry.</em><span className="d-only">entry <em>for the desk.</em></span></div>
            <div className="lg-mdl__dek">
              Tickets are filed by the desk on behalf of a client, or by the client directly.
              Once submitted, it lands in the queue immediately.
            </div>
            <div className="lg-mdl__rule" />

            <div className="lg-f-grid">
              <div className="lg-f full">
                <div className="lg-f__lbl"><span>Title</span><span className="req lg-f__req">Required</span></div>
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
                <div className="lg-f__lbl"><span>Description</span><span className="req lg-f__req">Required</span></div>
                <textarea
                  className="lg-f__ta"
                  placeholder="What's happening? The more detail, the faster the desk can act."
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  rows={4}
                  maxLength={50000}
                  required
                />
                <span className={`lg-f__char-count${description.length > 45000 ? ' warn' : ''}${description.length >= 50000 ? ' over' : ''}${description.length <= 45000 ? ' m-hide' : ''}`}>
                  {description.length.toLocaleString()} / 50,000
                </span>
              </div>

              <div className="lg-f full">
                <div className="lg-f__lbl"><span>Attachment</span><span className="opt lg-f__opt">Optional</span></div>
                <label htmlFor={fileInputId} className="lg-f__file-drop" data-active={!!attachment}>
                  <input
                    id={fileInputId}
                    ref={fileInputRef}
                    type="file"
                    accept=".pdf,.png,.jpg,.jpeg,.gif,.txt,.md"
                    style={{ display: 'none' }}
                    onChange={(e) => setAttachment(e.target.files?.[0] ?? null)}
                  />
                  {attachment ? (
                    <span className="lg-f__file-name">
                      📎 {attachment.name}
                      <button
                        type="button"
                        className="lg-f__file-clear"
                        onClick={(e) => { e.preventDefault(); setAttachment(null); if (fileInputRef.current) fileInputRef.current.value = ''; }}
                      >✕</button>
                    </span>
                  ) : (
                    <span className="lg-f__file-prompt">Click to attach a file — PDF, PNG, JPG, GIF, TXT, MD (max 100 MB)</span>
                  )}
                </label>
              </div>

              {isDesk && (
                <div ref={clientDropRef} className="lg-f full">
                  <div className="lg-f__lbl">
                    <span>Client</span>
                    <span className="req lg-f__req">Required</span>
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
                              setSubClientId('');
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

              {/* Sub-client selector — shown whenever a client is selected */}
              {isDesk && selectedClient && (
                <div className="lg-f full" style={{ opacity: subClientsQ.isLoading ? 0.5 : 1 }}>
                  <div className="lg-f__lbl">
                    <span>End client</span>
                    <span className="opt lg-f__opt">Optional</span>
                  </div>
                  {subClients.length > 0 ? (
                    <>
                      <div ref={subClientDropRef} style={{ position: 'relative' }}>
                        <button
                          type="button"
                          className="lg-f__inp"
                          style={{
                            width: '100%', textAlign: 'left', cursor: 'pointer',
                            display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                            background: 'var(--cream)',
                          }}
                          onClick={() => setSubClientDropOpen((v) => !v)}
                        >
                          <span style={{ color: subClientId ? 'var(--ink)' : 'var(--mid)' }}>
                            {subClientId
                              ? subClients.find(s => s.id === subClientId)?.name
                              : '— None —'}
                          </span>
                          <span style={{ fontSize: 10, color: 'var(--mid)', marginLeft: 8 }}>
                            {subClientDropOpen ? '▴' : '▾'}
                          </span>
                        </button>
                        {subClientDropOpen && (
                          <div style={{
                            position: 'absolute', top: '100%', left: 0, right: 0, zIndex: 50,
                            border: '1px solid var(--ink)', borderTop: 'none',
                            background: 'var(--cream)', maxHeight: 160, overflowY: 'auto',
                          }}>
                            <button
                              type="button"
                              style={{
                                display: 'block', width: '100%', padding: '10px 14px',
                                background: 'none', border: 'none', borderBottom: '1px solid var(--rule)',
                                textAlign: 'left', cursor: 'pointer',
                                fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--mid)',
                              }}
                              onClick={() => { setSubClientId(''); setSubClientDropOpen(false); }}
                            >
                              — None —
                            </button>
                            {subClients.map((sc) => (
                              <button
                                key={sc.id}
                                type="button"
                                style={{
                                  display: 'block', width: '100%', padding: '10px 14px',
                                  background: sc.id === subClientId ? 'var(--rule)' : 'none',
                                  border: 'none', borderBottom: '1px solid var(--rule)',
                                  textAlign: 'left', cursor: 'pointer',
                                  fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 16,
                                  color: 'var(--ink)',
                                }}
                                onClick={() => { setSubClientId(sc.id); setSubClientDropOpen(false); }}
                              >
                                {sc.name}
                              </button>
                            ))}
                          </div>
                        )}
                      </div>
                      {subClientId && (
                        <span className="lg-f__hint">
                          Tagging under {selectedClient.name} › {subClients.find(s => s.id === subClientId)?.name}
                        </span>
                      )}
                    </>
                  ) : (
                    <>
                      <div
                        className="lg-f__inp"
                        style={{
                          opacity: 0.45, cursor: 'not-allowed',
                          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                          color: 'var(--mid)',
                        }}
                      >
                        <span>No end clients on this account</span>
                        <span style={{ fontSize: 10 }}>▾</span>
                      </div>
                      <span className="lg-f__hint">
                        Add end clients from the{' '}
                        <a href="/clients" style={{ color: 'var(--ink)', textDecoration: 'underline' }}>
                          Clients page
                        </a>{' '}
                        to tag tickets this way.
                      </span>
                    </>
                  )}
                </div>
              )}
            </div>

            <div style={{ height: 24 }} />

            <div className="grid-priority">
              <div className="lg-f" style={{ opacity: closeNow ? 0.35 : 1, pointerEvents: closeNow ? 'none' : undefined }}>
                <div className="lg-f__lbl"><span>Priority</span></div>
                <div className="lg-seg">
                  {PRIORITIES.map((p) => (
                    <button
                      key={p}
                      type="button"
                      className={`lg-seg__o${priority === p ? (p === 'urgent' ? ' on red' : ' on') : ''}`}
                      onClick={() => setPriority(p)}
                      disabled={closeNow}
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
                <div className="lg-f__lbl"><span>Category</span><span className="opt lg-f__opt">Optional</span></div>
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
                <div className="lg-f__lbl"><span>Due date</span><span className="opt lg-f__opt">Optional</span></div>
                <input
                  className="lg-f__inp mono"
                  type="date"
                  value={dueDate}
                  onChange={(e) => setDueDate(e.target.value)}
                />
                <span className="lg-f__hint" style={{ fontFamily: 'var(--sans)', fontSize: 12.5, textTransform: 'none', letterSpacing: 0, color: 'var(--mid)' }}>Leave blank for no deadline</span>
              </div>

              <div className="lg-f" style={{ paddingTop: 4 }}>
                <div className="lg-f__lbl"><span>Recurrence</span><span className="opt lg-f__opt">Optional</span></div>
                <div className="lg-f-recur">
                  <button
                    type="button"
                    className={`lg-ck${recurring ? ' on' : ''}`}
                    onClick={() => setRecurring((v) => !v)}
                  >
                    <span className="lg-ck__b">{recurring ? '✓' : ''}</span>
                    <span className="lg-ck__l">Recurring</span>
                  </button>
                  <div style={{ display: 'flex', alignItems: 'baseline', gap: 8, opacity: recurring ? 1 : 0.35 }}>
                    <span style={{ fontFamily: 'var(--sans)', fontSize: 13, color: 'var(--mid)' }}>every</span>
                    <input
                      className="lg-f__inp mono"
                      value={interval}
                      onChange={(e) => setInterval(e.target.value)}
                      disabled={!recurring}
                      style={{ width: 44, textAlign: 'center' }}
                      min={1}
                      type="number"
                    />
                    <span style={{ fontFamily: 'var(--sans)', fontSize: 13, color: 'var(--mid)' }}>days</span>
                  </div>
                </div>
              </div>
            </div>

            {err && <div className="lg-f__err" style={{ marginTop: 16 }}>{err}</div>}
          </div>
        </form>

        {/* ── Footer ──────────────────────────────────────────────── */}
        <div className="lg-mdl__foot">
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <span className="meta">
              Filing as <b style={{ color: 'var(--ink)' }}>{filedAs}</b> · {filedDate}
            </span>
            {isDesk && (
              <button
                type="button"
                className={`lg-ck${closeNow ? ' on' : ''}`}
                onClick={() => setCloseNow((v) => !v)}
              >
                <span className="lg-ck__b">{closeNow ? '✓' : ''}</span>
                <span className="lg-ck__l">Close immediately</span>
              </button>
            )}
          </div>
          <div className="lg-mdl__btns">
            <button type="button" className="lg-bt lg-bt--text" onClick={onClose}>Cancel</button>
            <button
              type="submit"
              form="create-ticket-form"
              className={`lg-bt lg-bt--solid${createM.isPending ? ' is-loading' : ''}`}
              disabled={createM.isPending || !title.trim() || !description.trim() || (isDesk && !selectedClient)}
            >
              <span className="lbl">File ticket</span>
              <span className="arr">↗</span>
              <span className="spin" aria-hidden />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
