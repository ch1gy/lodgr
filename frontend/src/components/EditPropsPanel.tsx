import { useState } from 'react';
import type { PatchTicketPayload, TicketPriority, TicketResponse, TicketType } from '../api/types';
import { TICKET_TYPE_LABEL } from '../utils/format';

const PRIORITIES: TicketPriority[] = ['low', 'medium', 'high', 'urgent'];
const TICKET_TYPES: TicketType[]   = ['standard', 'maintenance', 'security_log'];

interface Props {
  ticket: TicketResponse;
  onCancel: () => void;
  onSave: (p: PatchTicketPayload) => void;
  savePending: boolean;
}

export function EditPropsPanel({ ticket, onCancel, onSave, savePending }: Props) {
  const [priority, setPriority]     = useState<TicketPriority>(ticket.priority);
  const [ticketType, setTicketType] = useState<TicketType>(ticket.ticket_type);
  const [category, setCategory]     = useState(ticket.category ?? '');
  const [dueDate, setDueDate]       = useState(ticket.due_date ?? '');
  const [recurring, setRecurring]   = useState(ticket.recurring);
  const [interval, setInterval]     = useState(String(ticket.recurring_interval_days ?? 30));

  function handleSave() {
    onSave({
      priority,
      ticket_type: ticketType,
      category: category.trim() || undefined,
      due_date: dueDate || undefined,
      recurring,
      recurring_interval_days: recurring ? Number(interval) : undefined,
    });
  }

  return (
    <div className="lg-ed">
      <div className="lg-ed__head">
        <span className="lbl">— Edit properties · {ticket.id.slice(0, 8)}</span>
        <span className="ed">Editing ⊙</span>
      </div>

      {/* Priority */}
      <div className="lg-ed__sec">
        <div className="lg-ed__sec-lbl">— Priority</div>
        <div className="lg-ed__chips">
          {PRIORITIES.map((p) => (
            <button
              key={p}
              type="button"
              className={`lg-ed__chip${priority === p ? (p === 'urgent' ? ' on red' : ' on') : ''}`}
              onClick={() => setPriority(p)}
            >
              {p[0].toUpperCase() + p.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {/* Ticket type */}
      <div className="lg-ed__sec">
        <div className="lg-ed__sec-lbl">— Ticket type</div>
        <div className="lg-ed__chips">
          {TICKET_TYPES.map((t) => (
            <button
              key={t}
              type="button"
              className={`lg-ed__chip${ticketType === t ? ' on' : ''}`}
              onClick={() => setTicketType(t)}
            >
              {TICKET_TYPE_LABEL[t]}
            </button>
          ))}
        </div>
      </div>

      {/* Details */}
      <div className="lg-ed__sec">
        <div className="lg-ed__sec-lbl">— Details</div>
        <div className="lg-ed__row">
          <span className="k">Category</span>
          <input
            className="v-edit"
            value={category}
            onChange={(e) => setCategory(e.target.value)}
            placeholder="e.g. Mail & Calendar"
            maxLength={100}
          />
        </div>
        <div className="lg-ed__row">
          <span className="k">Due date</span>
          <input
            className="v-edit"
            type="date"
            value={dueDate}
            onChange={(e) => setDueDate(e.target.value)}
            style={{ fontFamily: 'var(--mono)', fontSize: 13 }}
          />
        </div>
        <div className="lg-ed__row">
          <span className="k">Created</span>
          <span style={{
            fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '.04em', color: 'var(--mid)',
          }}>
            {new Date(ticket.created_at).toLocaleDateString('en-GB')} · read-only
          </span>
        </div>
      </div>

      {/* Recurrence */}
      <div className="lg-ed__sec">
        <div className="lg-ed__sec-lbl">— Recurrence</div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14, paddingTop: 4 }}>
          <button
            type="button"
            className={`lg-ck${recurring ? ' on' : ''}`}
            onClick={() => setRecurring((v) => !v)}
          >
            <span className="lg-ck__b">{recurring ? '✓' : ''}</span>
            <span className="lg-ck__l">Recurring</span>
          </button>
        </div>
        {recurring && (
          <div className="lg-ed__row" style={{ marginTop: 4 }}>
            <span className="k">Every</span>
            <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
              <input
                className="v-edit"
                type="number"
                min={1}
                value={interval}
                onChange={(e) => setInterval(e.target.value)}
                style={{ width: 56, textAlign: 'center', fontFamily: 'var(--mono)', fontSize: 14 }}
              />
              <span style={{
                fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '.14em',
                textTransform: 'uppercase', color: 'var(--mid)',
              }}>days</span>
            </div>
          </div>
        )}
      </div>

      {/* Read-only client info */}
      <div className="lg-ed__sec">
        <div className="lg-ed__sec-lbl">— Read-only</div>
        <div className="lg-ed__row">
          <span className="k">Status</span>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--mid)' }}>
            {ticket.status}
          </span>
        </div>
        <div className="lg-ed__row">
          <span className="k">Client ID</span>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--mid)' }}>
            {ticket.client_id.slice(0, 8)}
          </span>
        </div>
      </div>

      {/* Save / Cancel */}
      <div className="lg-ed__save">
        <button type="button" className="cancel" onClick={onCancel} disabled={savePending}>
          Cancel
        </button>
        <button type="button" className="save" onClick={handleSave} disabled={savePending}>
          {savePending ? 'Saving…' : 'Save changes'} <span className="arr">↗</span>
        </button>
      </div>
    </div>
  );
}
