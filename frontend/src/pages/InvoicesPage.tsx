import { useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { ConfirmModal } from '../components/ConfirmModal';
import { admin } from '../api/admin';
import { api } from '../api/client';
import { downloadBlob } from '../utils/format';
import type {
  Client,
  CreateInvoicePayload,
  InvoiceItem,
  InvoiceNote,
  InvoiceResponse,
  InvoiceStatus,
  RecurInterval,
  UpdateInvoicePayload,
} from '../api/types';
import '../styles/v2.css';

// ── Helpers ───────────────────────────────────────────────────────────────────


function fmtDate(s: string): string {
  if (!s) return '—';
  const d = new Date(s + 'T00:00:00');
  return d.toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
}

const STATUS_LABEL: Record<InvoiceStatus, string> = {
  draft: 'Draft',
  sent: 'Sent',
  paid: 'Paid',
};

const STATUS_COLOR: Record<InvoiceStatus, string> = {
  draft: 'var(--mid)',
  sent: 'var(--ink)',
  paid: '#2a7a3b',
};

// ── Shared invoice field helpers ──────────────────────────────────────────────

function InvField({ label, children, span2 }: { label: string; children: React.ReactNode; span2?: boolean }) {
  return (
    <div style={span2 ? { gridColumn: '1 / -1' } : undefined}>
      <div className="inv-lbl">{label}</div>
      {children}
    </div>
  );
}

// ── Shared sub-components ─────────────────────────────────────────────────────

interface LineItemsEditorProps {
  items: InvoiceItem[];
  currency: string;
  total: number;
  onSetItem: (i: number, f: keyof InvoiceItem, v: string | number) => void;
  onAddItem: () => void;
  onRemoveItem: (i: number) => void;
}

function LineItemsEditor({ items, currency, total, onSetItem, onAddItem, onRemoveItem }: LineItemsEditorProps) {
  return (
    <>
      <div className="inv-sec">Line items</div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 56px 96px 22px', gap: 6, marginBottom: 4 }}>
        {(['Description', 'Qty', 'Rate'] as const).map((h) => (
          <span key={h} style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.14em', textTransform: 'uppercase', color: 'var(--mid)', textAlign: h !== 'Description' ? 'right' : 'left' }}>{h}</span>
        ))}
      </div>
      {items.map((it, i) => (
        <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 56px 96px 22px', gap: 6, marginBottom: 6, alignItems: 'start' }}>
          <div>
            <input className="inv-inp" value={it.name} onChange={(e) => onSetItem(i, 'name', e.target.value)} placeholder="Item name" />
            <input className="inv-inp" style={{ marginTop: 3, fontSize: 10, color: 'var(--mid)' }} value={it.sub ?? ''} onChange={(e) => onSetItem(i, 'sub', e.target.value)} placeholder="Description (optional)" />
          </div>
          <input className="inv-inp" style={{ textAlign: 'right' }} type="number" min="1" value={it.qty} onChange={(e) => onSetItem(i, 'qty', Number(e.target.value))} placeholder="1" />
          <input className="inv-inp" style={{ textAlign: 'right' }} type="number" min="0" value={it.rate} onChange={(e) => onSetItem(i, 'rate', Number(e.target.value))} placeholder="0" />
          <button type="button" onClick={() => onRemoveItem(i)} disabled={items.length === 1}
            style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 12, padding: 0, alignSelf: 'center' }}>✕</button>
        </div>
      ))}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 6 }}>
        <button type="button" onClick={onAddItem}
          style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase', background: 'none', border: '1px dashed var(--rule)', padding: '3px 10px', cursor: 'pointer', color: 'var(--mid)' }}>
          + Add item
        </button>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink)' }}>
          Total: <b>{currency} {total.toLocaleString('en-US')}</b>
        </span>
      </div>
    </>
  );
}

interface NotesEditorProps {
  editorNote: string;
  onEditorNoteChange: (v: string) => void;
  notes: InvoiceNote[];
  onSetNote: (i: number, f: 'k' | 'v', v: string) => void;
  onAddNote: () => void;
  onRemoveNote: (i: number) => void;
}

function NotesEditor({ editorNote, onEditorNoteChange, notes, onSetNote, onAddNote, onRemoveNote }: NotesEditorProps) {
  return (
    <>
      <div className="inv-sec">Notes</div>
      <InvField label="Editor note (shown on invoice)">
        <textarea className="inv-inp" style={{ height: 52, resize: 'vertical', fontStyle: 'italic' }}
          value={editorNote} onChange={(e) => onEditorNoteChange(e.target.value)} placeholder="A short personal note to the client…" />
      </InvField>
      <div style={{ marginTop: 10 }}>
        {notes.map((n, i) => (
          <div key={i} style={{ display: 'grid', gridTemplateColumns: '110px 1fr 22px', gap: 6, marginBottom: 6 }}>
            <input className="inv-inp" value={n.k} onChange={(e) => onSetNote(i, 'k', e.target.value)} placeholder="Label" />
            <input className="inv-inp" value={n.v} onChange={(e) => onSetNote(i, 'v', e.target.value)} placeholder="Text" />
            <button type="button" onClick={() => onRemoveNote(i)}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 12 }}>✕</button>
          </div>
        ))}
        <button type="button" onClick={onAddNote}
          style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase', background: 'none', border: '1px dashed var(--rule)', padding: '3px 10px', cursor: 'pointer', color: 'var(--mid)' }}>
          + Add note
        </button>
      </div>
    </>
  );
}

// ── Create invoice modal ──────────────────────────────────────────────────────

interface CreateModalProps {
  clients: Client[];
  onClose: () => void;
}

function CreateInvoiceModal({ clients, onClose }: CreateModalProps) {
  const qc = useQueryClient();
  const today = new Date().toISOString().slice(0, 10);
  const twoWeeks = new Date(Date.now() + 14 * 86400000).toISOString().slice(0, 10);

  const [clientSearch, setClientSearch] = useState('');
  const [selectedClient, setSelectedClient] = useState<Client | null>(null);
  const [clientDropOpen, setClientDropOpen] = useState(false);
  const clientDropRef = useRef<HTMLDivElement>(null);
  const [currency, setCurrency] = useState('KES');
  const [terms, setTerms] = useState('Net 14');
  const [issuedDate, setIssuedDate] = useState(today);
  const [dueDate, setDueDate] = useState(twoWeeks);
  const [projectType, setProjectType] = useState('');
  const [projectLocation, setProjectLocation] = useState('Nairobi');
  const [billedToName, setBilledToName] = useState('');
  const [billedToRole, setBilledToRole] = useState('');
  const [billedToAddr1, setBilledToAddr1] = useState('');
  const [billedToAddr2, setBilledToAddr2] = useState('');
  const [billedToPin, setBilledToPin] = useState('');
  const [billedToEmail, setBilledToEmail] = useState('');
  const [billedToPhone, setBilledToPhone] = useState('');
  const [editorNote, setEditorNote] = useState('');
  const [kraNumber] = useState('');
  const [recurring, setRecurring] = useState(false);
  const [recurInterval, setRecurInterval] = useState<RecurInterval>('monthly');
  const [nextRecurDate, setNextRecurDate] = useState('');
  const [items, setItems] = useState<InvoiceItem[]>([{ name: '', qty: 1, rate: 0 }]);
  const [notes, setNotes] = useState<InvoiceNote[]>([]);
  const [err, setErr] = useState('');

  // Outside-click closes client dropdown
  useEffect(() => {
    if (!clientDropOpen) return;
    function h(e: MouseEvent) {
      if (clientDropRef.current && !clientDropRef.current.contains(e.target as Node))
        setClientDropOpen(false);
    }
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, [clientDropOpen]);

  const activeClients = useMemo(() => clients.filter((c) => c.deleted_at === null), [clients]);
  const filteredClients = useMemo(() => {
    const q = clientSearch.toLowerCase();
    return activeClients.filter((c) => !q || c.name.toLowerCase().includes(q) || c.email.toLowerCase().includes(q));
  }, [activeClients, clientSearch]);

  function selectClient(c: Client) {
    setSelectedClient(c);
    setClientSearch(c.name);
    setClientDropOpen(false);
    setBilledToName(c.name);
    setBilledToRole(c.contact_person ?? '');
    setBilledToAddr1(c.address_line1 ?? '');
    setBilledToAddr2(c.address_line2 ?? '');
    setBilledToPin(c.pin_number ?? '');
    setBilledToEmail(c.email ?? '');
    setBilledToPhone(c.phone ?? '');
  }

  function addItem() { setItems((p) => [...p, { name: '', qty: 1, rate: 0 }]); }
  function removeItem(i: number) { setItems((p) => p.filter((_, idx) => idx !== i)); }
  function setItem(i: number, f: keyof InvoiceItem, v: string | number) {
    setItems((p) => p.map((it, idx) => idx === i ? { ...it, [f]: v } : it));
  }
  function addNote() { setNotes((p) => [...p, { k: '', v: '' }]); }
  function removeNote(i: number) { setNotes((p) => p.filter((_, idx) => idx !== i)); }
  function setNote(i: number, f: 'k' | 'v', v: string) {
    setNotes((p) => p.map((n, idx) => idx === i ? { ...n, [f]: v } : n));
  }

  const total = items.reduce((acc, it) => acc + it.qty * it.rate, 0);

  const createM = useMutation({
    mutationFn: (payload: CreateInvoicePayload) => admin.createInvoice(payload),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['invoices'] }); onClose(); },
    onError: (e: Error) => setErr(e.message),
  });

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setErr('');
    if (!selectedClient) { setErr('Select a client'); return; }
    if (items.some((it) => !it.name.trim())) { setErr('All line items need a name'); return; }
    createM.mutate({
      client_id: selectedClient.id,
      currency, terms, issued_date: issuedDate, due_date: dueDate,
      project_type: projectType || undefined, project_location: projectLocation || undefined,
      billed_to_name: billedToName, billed_to_role: billedToRole || undefined,
      billed_to_addr1: billedToAddr1 || undefined, billed_to_addr2: billedToAddr2 || undefined,
      billed_to_pin: billedToPin || undefined,
      billed_to_email: billedToEmail || undefined, billed_to_phone: billedToPhone || undefined,
      items, notes: notes.filter((n) => n.k.trim()),
      editor_note: editorNote || undefined, kra_number: kraNumber || undefined,
      recurring, recur_interval: recurring ? recurInterval : undefined,
      next_recur_date: recurring && nextRecurDate ? nextRecurDate : undefined,
    });
  }

  return (
    <div className="lg-ov" role="dialog" aria-modal aria-label="New invoice">
      <div className="lg-mdl invoice">
        <div className="lg-mdl__top">
          <span className="lg-mdl__eye">— Invoices — New invoice</span>
          <span className="lg-mdl__no"><i>Lodgr</i><span className="dot">.</span></span>
          <button type="button" className="lg-mdl__x" onClick={onClose}>Close ✕</button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="lg-mdl__body">

            {/* ── Identity ──────────────────────────────────────────────── */}
            <div className="inv-grid-2" style={{ marginBottom: 0 }}>
              {/* Client — custom dropdown */}
              <div>
                <div className="inv-lbl">Client *</div>
                <div ref={clientDropRef} style={{ position: 'relative' }}>
                  <input
                    className="inv-inp"
                    placeholder="Search by name…"
                    value={clientSearch}
                    onChange={(e) => { setClientSearch(e.target.value); setSelectedClient(null); setClientDropOpen(true); }}
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
                        <button key={c.id} type="button"
                          style={{
                            display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: 12,
                            width: '100%', padding: '8px 10px', background: 'none', border: 'none',
                            borderBottom: '1px solid var(--rule)', cursor: 'pointer',
                          }}
                          onClick={() => selectClient(c)}
                        >
                          <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 14 }}>{c.name}</span>
                          <span style={{ fontFamily: 'var(--mono)', fontSize: 9, color: 'var(--mid)', flexShrink: 0 }}>{c.email}</span>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </div>

              <InvField label="Project type">
                <input className="inv-inp" value={projectType} onChange={(e) => setProjectType(e.target.value)} placeholder="e.g. Website Maintenance" />
              </InvField>
            </div>

            {/* ── Dates & meta ──────────────────────────────────────────── */}
            <div className="inv-sec">Dates & billing</div>
            <div className="inv-grid-4">
              <InvField label="Issued *">
                <input className="inv-inp" type="date" value={issuedDate} onChange={(e) => setIssuedDate(e.target.value)} required />
              </InvField>
              <InvField label="Due *">
                <input className="inv-inp" type="date" value={dueDate} onChange={(e) => setDueDate(e.target.value)} required />
              </InvField>
              <InvField label="Terms">
                <input className="inv-inp" value={terms} onChange={(e) => setTerms(e.target.value)} placeholder="Net 14" />
              </InvField>
              <InvField label="Currency">
                <input className="inv-inp" value={currency} onChange={(e) => setCurrency(e.target.value)} placeholder="KES" />
              </InvField>
            </div>

            {/* ── Billed to ─────────────────────────────────────────────── */}
            <div className="inv-sec">Billed to</div>
            <div className="inv-grid-2">
              <InvField label="Name *">
                <input className="inv-inp" value={billedToName} onChange={(e) => setBilledToName(e.target.value)} placeholder="Company name" required />
              </InvField>
              <InvField label="Contact / role">
                <input className="inv-inp" value={billedToRole} onChange={(e) => setBilledToRole(e.target.value)} placeholder="e.g. Jane Doe" />
              </InvField>
              <InvField label="Address line 1">
                <input className="inv-inp" value={billedToAddr1} onChange={(e) => setBilledToAddr1(e.target.value)} placeholder="Street address" />
              </InvField>
              <InvField label="Address line 2">
                <input className="inv-inp" value={billedToAddr2} onChange={(e) => setBilledToAddr2(e.target.value)} placeholder="City, Country" />
              </InvField>
              <InvField label="PIN / KRA">
                <input className="inv-inp" value={billedToPin} onChange={(e) => setBilledToPin(e.target.value)} placeholder="e.g. P051234567X" />
              </InvField>
              <InvField label="Email">
                <input className="inv-inp" type="email" value={billedToEmail} onChange={(e) => setBilledToEmail(e.target.value)} placeholder="client@example.com" />
              </InvField>
              <InvField label="Phone">
                <input className="inv-inp" type="tel" value={billedToPhone} onChange={(e) => setBilledToPhone(e.target.value)} placeholder="+254 7XX XXX XXX" />
              </InvField>
              <InvField label="Location (for strap)">
                <input className="inv-inp" value={projectLocation} onChange={(e) => setProjectLocation(e.target.value)} placeholder="e.g. Nairobi" />
              </InvField>
            </div>

            {/* ── Line items ────────────────────────────────────────────── */}
            <LineItemsEditor
              items={items} currency={currency} total={total}
              onSetItem={setItem} onAddItem={addItem} onRemoveItem={removeItem}
            />

            {/* ── Editor note & notes ───────────────────────────────────── */}
            <NotesEditor
              editorNote={editorNote} onEditorNoteChange={setEditorNote}
              notes={notes} onSetNote={setNote} onAddNote={addNote} onRemoveNote={removeNote}
            />

            {/* ── Recurring ─────────────────────────────────────────────── */}
            <div className="inv-sec">Recurring</div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
              <button type="button" className={`lg-ck${recurring ? ' on' : ''}`} onClick={() => setRecurring((v) => !v)}>
                <span className="lg-ck__b">{recurring ? '✓' : ''}</span>
                <span className="lg-ck__l">Recurring invoice</span>
              </button>
            </div>
            {recurring && (
              <div className="inv-grid-3" style={{ marginTop: 10 }}>
                <div>
                  <div className="inv-lbl">Interval</div>
                  <div className="lg-seg" style={{ marginTop: 4 }}>
                    {(['monthly', 'quarterly', 'yearly'] as RecurInterval[]).map((v) => (
                      <button key={v} type="button"
                        className={`lg-seg__o${recurInterval === v ? ' on' : ''}`}
                        onClick={() => setRecurInterval(v)}>
                        {v[0].toUpperCase() + v.slice(1)}
                      </button>
                    ))}
                  </div>
                </div>
                <div style={{ gridColumn: '2 / 4' }}>
                  <InvField label="First auto-create on">
                    <input className="inv-inp" type="date" value={nextRecurDate} onChange={(e) => setNextRecurDate(e.target.value)} />
                  </InvField>
                </div>
              </div>
            )}

            {err && <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--red)', marginTop: 12 }}>{err}</div>}
          </div>

          <div className="lg-mdl__foot">
            <div />
            <div className="lg-mdl__btns">
              <button type="button" className="lg-bt lg-bt--text" onClick={onClose}>Cancel</button>
              <button type="submit" className="lg-bt lg-bt--solid" disabled={createM.isPending}>
                <span className="lbl">{createM.isPending ? 'Creating…' : 'Create draft'}</span>
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Edit invoice modal ────────────────────────────────────────────────────────

interface EditModalProps {
  invoice: InvoiceResponse;
  onClose: () => void;
}

function EditInvoiceModal({ invoice, onClose }: EditModalProps) {
  const qc = useQueryClient();

  const [number, setNumber]               = useState(invoice.number);
  const [currency, setCurrency]           = useState(invoice.currency);
  const [terms, setTerms]                 = useState(invoice.terms);
  const [issuedDate, setIssuedDate]       = useState(invoice.issued_date);
  const [dueDate, setDueDate]             = useState(invoice.due_date);
  const [projectType, setProjectType]     = useState(invoice.project_type);
  const [projectLocation, setProjectLocation] = useState(invoice.project_location);
  const [billedToName, setBilledToName]   = useState(invoice.billed_to_name);
  const [billedToRole, setBilledToRole]   = useState(invoice.billed_to_role);
  const [billedToAddr1, setBilledToAddr1] = useState(invoice.billed_to_addr1);
  const [billedToAddr2, setBilledToAddr2] = useState(invoice.billed_to_addr2);
  const [billedToPin, setBilledToPin]     = useState(invoice.billed_to_pin);
  const [billedToEmail, setBilledToEmail] = useState(invoice.billed_to_email);
  const [billedToPhone, setBilledToPhone] = useState(invoice.billed_to_phone);
  const [editorNote, setEditorNote]       = useState(invoice.editor_note ?? '');
  const [kraNumber, setKraNumber]         = useState(invoice.kra_number ?? '');
  const [recurring, setRecurring]         = useState(invoice.recurring);
  const [recurInterval, setRecurInterval] = useState<RecurInterval>((invoice.recur_interval as RecurInterval) ?? 'monthly');
  const [nextRecurDate, setNextRecurDate] = useState(invoice.next_recur_date ?? '');
  const [items, setItems]                 = useState<InvoiceItem[]>(
    invoice.items.length ? invoice.items : [{ name: '', qty: 1, rate: 0 }],
  );
  const [notes, setNotes]                 = useState<InvoiceNote[]>(invoice.notes);
  const [err, setErr]                     = useState('');

  function addItem() { setItems((p) => [...p, { name: '', qty: 1, rate: 0 }]); }
  function removeItem(i: number) { setItems((p) => p.filter((_, idx) => idx !== i)); }
  function setItem(i: number, f: keyof InvoiceItem, v: string | number) {
    setItems((p) => p.map((it, idx) => idx === i ? { ...it, [f]: v } : it));
  }
  function addNote() { setNotes((p) => [...p, { k: '', v: '' }]); }
  function removeNote(i: number) { setNotes((p) => p.filter((_, idx) => idx !== i)); }
  function setNote(i: number, f: 'k' | 'v', v: string) {
    setNotes((p) => p.map((n, idx) => idx === i ? { ...n, [f]: v } : n));
  }

  const total = items.reduce((acc, it) => acc + it.qty * it.rate, 0);

  const updateM = useMutation({
    mutationFn: (payload: UpdateInvoicePayload) => admin.updateInvoice(invoice.id, payload),
    onSuccess: () => { void qc.invalidateQueries({ queryKey: ['invoices'] }); onClose(); },
    onError: (e: Error) => setErr(e.message),
  });

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setErr('');
    if (!number.trim()) { setErr('Invoice number is required'); return; }
    if (items.some((it) => !it.name.trim())) { setErr('All line items need a name'); return; }
    updateM.mutate({
      number: number.trim(), currency, terms,
      issued_date: issuedDate, due_date: dueDate,
      project_type: projectType || undefined, project_location: projectLocation || undefined,
      billed_to_name: billedToName, billed_to_role: billedToRole || undefined,
      billed_to_addr1: billedToAddr1 || undefined, billed_to_addr2: billedToAddr2 || undefined,
      billed_to_pin: billedToPin || undefined,
      billed_to_email: billedToEmail || undefined, billed_to_phone: billedToPhone || undefined,
      items, notes: notes.filter((n) => n.k.trim()),
      editor_note: editorNote || undefined, kra_number: kraNumber || undefined,
      recurring, recur_interval: recurring ? recurInterval : undefined,
      next_recur_date: recurring && nextRecurDate ? nextRecurDate : undefined,
    });
  }

  return (
    <div className="lg-ov" role="dialog" aria-modal aria-label="Edit invoice">
      <div className="lg-mdl invoice">
        <div className="lg-mdl__top">
          <span className="lg-mdl__eye">— Invoices — {invoice.recurring ? 'Recurring template' : STATUS_LABEL[invoice.status]}</span>
          <span className="lg-mdl__no"><i>Lodgr</i><span className="dot">.</span></span>
          <button type="button" className="lg-mdl__x" onClick={onClose}>Close ✕</button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="lg-mdl__body">
            <div style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 22, marginBottom: 16, color: 'var(--ink)' }}>
              Edit invoice <span style={{ color: 'var(--mid)', fontSize: 14 }}>· {invoice.number}</span>
            </div>

            {/* ── Identity ──────────────────────────────────────────────── */}
            <div className="inv-grid-2">
              <InvField label="Invoice number *">
                <input className="inv-inp" value={number} onChange={(e) => setNumber(e.target.value)} required />
              </InvField>
              <InvField label="KRA / official number">
                <input className="inv-inp" value={kraNumber} onChange={(e) => setKraNumber(e.target.value)} placeholder="Fill before sending" />
              </InvField>
              <InvField label="Project type">
                <input className="inv-inp" value={projectType} onChange={(e) => setProjectType(e.target.value)} placeholder="e.g. Website Maintenance" />
              </InvField>
              <InvField label="Location (for strap)">
                <input className="inv-inp" value={projectLocation} onChange={(e) => setProjectLocation(e.target.value)} />
              </InvField>
            </div>

            {/* ── Dates & meta ──────────────────────────────────────────── */}
            <div className="inv-sec">Dates & billing</div>
            <div className="inv-grid-4">
              <InvField label="Issued *">
                <input className="inv-inp" type="date" value={issuedDate} onChange={(e) => setIssuedDate(e.target.value)} required />
              </InvField>
              <InvField label="Due *">
                <input className="inv-inp" type="date" value={dueDate} onChange={(e) => setDueDate(e.target.value)} required />
              </InvField>
              <InvField label="Terms">
                <input className="inv-inp" value={terms} onChange={(e) => setTerms(e.target.value)} />
              </InvField>
              <InvField label="Currency">
                <input className="inv-inp" value={currency} onChange={(e) => setCurrency(e.target.value)} />
              </InvField>
            </div>

            {/* ── Billed to ─────────────────────────────────────────────── */}
            <div className="inv-sec">Billed to</div>
            <div className="inv-grid-2">
              <InvField label="Name *">
                <input className="inv-inp" value={billedToName} onChange={(e) => setBilledToName(e.target.value)} required />
              </InvField>
              <InvField label="Contact / role">
                <input className="inv-inp" value={billedToRole} onChange={(e) => setBilledToRole(e.target.value)} />
              </InvField>
              <InvField label="Address line 1">
                <input className="inv-inp" value={billedToAddr1} onChange={(e) => setBilledToAddr1(e.target.value)} />
              </InvField>
              <InvField label="Address line 2">
                <input className="inv-inp" value={billedToAddr2} onChange={(e) => setBilledToAddr2(e.target.value)} />
              </InvField>
              <InvField label="PIN / KRA">
                <input className="inv-inp" value={billedToPin} onChange={(e) => setBilledToPin(e.target.value)} />
              </InvField>
              <InvField label="Email">
                <input className="inv-inp" type="email" value={billedToEmail} onChange={(e) => setBilledToEmail(e.target.value)} placeholder="client@example.com" />
              </InvField>
              <InvField label="Phone">
                <input className="inv-inp" type="tel" value={billedToPhone} onChange={(e) => setBilledToPhone(e.target.value)} placeholder="+254 7XX XXX XXX" />
              </InvField>
            </div>

            {/* ── Line items ────────────────────────────────────────────── */}
            <LineItemsEditor
              items={items} currency={currency} total={total}
              onSetItem={setItem} onAddItem={addItem} onRemoveItem={removeItem}
            />

            {/* ── Notes ─────────────────────────────────────────────────── */}
            <NotesEditor
              editorNote={editorNote} onEditorNoteChange={setEditorNote}
              notes={notes} onSetNote={setNote} onAddNote={addNote} onRemoveNote={removeNote}
            />

            {/* ── Recurring ─────────────────────────────────────────────── */}
            <div className="inv-sec">Recurring</div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
              <button type="button" className={`lg-ck${recurring ? ' on' : ''}`} onClick={() => setRecurring((v) => !v)}>
                <span className="lg-ck__b">{recurring ? '✓' : ''}</span>
                <span className="lg-ck__l">Recurring invoice</span>
              </button>
            </div>
            {recurring && (
              <div className="inv-grid-3" style={{ marginTop: 10 }}>
                <div>
                  <div className="inv-lbl">Interval</div>
                  <div className="lg-seg" style={{ marginTop: 4 }}>
                    {(['monthly', 'quarterly', 'yearly'] as RecurInterval[]).map((v) => (
                      <button key={v} type="button"
                        className={`lg-seg__o${recurInterval === v ? ' on' : ''}`}
                        onClick={() => setRecurInterval(v)}>
                        {v[0].toUpperCase() + v.slice(1)}
                      </button>
                    ))}
                  </div>
                </div>
                <div style={{ gridColumn: '2 / 4' }}>
                  <InvField label="Next auto-create on">
                    <input className="inv-inp" type="date" value={nextRecurDate} onChange={(e) => setNextRecurDate(e.target.value)} />
                  </InvField>
                </div>
              </div>
            )}

            {err && <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--red)', marginTop: 12 }}>{err}</div>}
          </div>

          <div className="lg-mdl__foot">
            <div />
            <div className="lg-mdl__btns">
              <button type="button" className="lg-bt lg-bt--text" onClick={onClose}>Cancel</button>
              <button type="submit" className="lg-bt lg-bt--solid" disabled={updateM.isPending}>
                <span className="lbl">{updateM.isPending ? 'Saving…' : 'Save changes'}</span>
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Invoice row ───────────────────────────────────────────────────────────────

interface InvoiceRowProps {
  invoice: InvoiceResponse;
  clientName: string;
  onStatusChange: (id: string, status: InvoiceStatus) => void;
  onDelete: (id: string) => void;
  onDownload: (id: string) => void;
  onPreview: (id: string) => void;
  onEdit: (invoice: InvoiceResponse) => void;
}

function InvoiceRow({ invoice, clientName, onStatusChange, onDelete, onDownload, onPreview, onEdit }: InvoiceRowProps) {
  const [expanded, setExpanded] = useState(false);
  const total = invoice.items.reduce((acc, it) => acc + it.qty * it.rate, 0);

  return (
    <div style={{ borderBottom: '1px solid var(--rule)' }}>
      {/* ── Summary row ─────────────────────────────────────────────────── */}
      <div
        onClick={() => setExpanded((v) => !v)}
        style={{
          display: 'grid',
          gridTemplateColumns: '28px 1fr 120px 120px 110px 110px auto',
          gap: 12,
          padding: '12px 0',
          cursor: 'pointer',
          alignItems: 'center',
          fontFamily: 'var(--mono)',
          fontSize: 10,
        }}
      >
        <span style={{ color: 'var(--mid)' }}>{expanded ? '▾' : '▸'}</span>
        <div>
          <div style={{ fontFamily: 'var(--sans)', fontSize: 13, fontWeight: 500, color: 'var(--ink)' }}>
            {invoice.number}
            {invoice.kra_number && (
              <span style={{ marginLeft: 8, color: 'var(--mid)', fontSize: 10 }}>· {invoice.kra_number}</span>
            )}
          </div>
          <div style={{ color: 'var(--mid)', marginTop: 2 }}>{clientName}</div>
        </div>
        <div style={{ color: 'var(--mid)' }}>{fmtDate(invoice.issued_date)}</div>
        <div style={{ color: invoice.status === 'paid' ? '#2a7a3b' : invoice.status === 'draft' ? 'var(--mid)' : 'var(--ink)' }}>
          {fmtDate(invoice.due_date)}
        </div>
        <div style={{ color: STATUS_COLOR[invoice.status], fontWeight: 500 }}>
          {STATUS_LABEL[invoice.status]}
        </div>
        <div style={{ fontWeight: 600, color: 'var(--ink)', fontVariantNumeric: 'tabular-nums' }}>
          {invoice.currency} {total.toLocaleString('en-US')}
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button type="button" onClick={(e) => { e.stopPropagation(); onEdit(invoice); }}
            style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.10em', textTransform: 'uppercase', background: 'none', border: '1px solid var(--rule)', padding: '3px 10px', cursor: 'pointer', color: 'var(--ink)' }}>
            Edit ✎
          </button>
          <button type="button" onClick={(e) => { e.stopPropagation(); void onPreview(invoice.id); }}
            style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.10em', textTransform: 'uppercase', background: 'none', border: '1px solid var(--rule)', padding: '3px 10px', cursor: 'pointer', color: 'var(--mid)' }}>
            Preview ↗
          </button>
          <button type="button" onClick={(e) => { e.stopPropagation(); void onDownload(invoice.id); }}
            style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.10em', textTransform: 'uppercase', background: 'none', border: '1px solid var(--rule)', padding: '3px 10px', cursor: 'pointer', color: 'var(--mid)' }}>
            Download ↓
          </button>
          <button type="button" onClick={(e) => { e.stopPropagation(); onDelete(invoice.id); }}
            style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.10em', textTransform: 'uppercase', background: 'none', border: '1px solid var(--rule)', padding: '3px 10px', cursor: 'pointer', color: 'var(--red)' }}>
            Del
          </button>
        </div>
      </div>

      {/* ── Expanded detail ──────────────────────────────────────────────── */}
      {expanded && (
        <div style={{ padding: '0 40px 16px', borderTop: '1px dashed var(--rule)', marginTop: -1 }}>
          {/* Status control */}
          <div style={{ display: 'flex', gap: 8, marginBottom: 14, paddingTop: 14 }}>
            {(['draft', 'sent', 'paid'] as InvoiceStatus[]).map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => onStatusChange(invoice.id, s)}
                style={{
                  fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.12em', textTransform: 'uppercase',
                  background: invoice.status === s ? 'var(--ink)' : 'none',
                  color: invoice.status === s ? 'var(--paper)' : 'var(--mid)',
                  border: '1px solid var(--rule)', padding: '3px 12px', cursor: 'pointer',
                }}
              >
                {STATUS_LABEL[s]}
              </button>
            ))}
          </div>

          {/* Items table */}
          <div style={{ fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '.14em', textTransform: 'uppercase', color: 'var(--mid)', marginBottom: 6 }}>
            Line items
          </div>
          {invoice.items.map((it, i) => (
            <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 50px 100px 100px', gap: 12, fontFamily: 'var(--sans)', fontSize: 12, padding: '6px 0', borderBottom: '1px solid var(--rule)' }}>
              <div>
                <b>{it.name}</b>
                {it.sub && <div style={{ color: 'var(--mid)', fontSize: 11 }}>{it.sub}</div>}
              </div>
              <div style={{ textAlign: 'right', color: 'var(--mid)', fontFamily: 'var(--mono)', fontSize: 11 }}>{it.qty}</div>
              <div style={{ textAlign: 'right', fontFamily: 'var(--mono)', fontSize: 11 }}>@ {it.rate.toLocaleString('en-US')}</div>
              <div style={{ textAlign: 'right', fontFamily: 'var(--mono)', fontSize: 11, fontWeight: 600 }}>{(it.qty * it.rate).toLocaleString('en-US')}</div>
            </div>
          ))}
          <div style={{ textAlign: 'right', fontFamily: 'var(--mono)', fontSize: 12, fontWeight: 600, padding: '8px 0', borderTop: '2px solid var(--ink)' }}>
            {invoice.currency} {total.toLocaleString('en-US')}
          </div>

          {/* Notes */}
          {invoice.notes.length > 0 && (
            <div style={{ marginTop: 10 }}>
              {invoice.notes.map((n, i) => (
                <div key={i} style={{ display: 'grid', gridTemplateColumns: '100px 1fr', gap: 12, fontFamily: 'var(--sans)', fontSize: 11, padding: '4px 0', color: 'var(--ink-soft)' }}>
                  <span style={{ fontFamily: 'var(--mono)', fontSize: 9, color: 'var(--mid)', textTransform: 'uppercase', letterSpacing: '.1em' }}>{n.k}</span>
                  <span>{n.v}</span>
                </div>
              ))}
            </div>
          )}

          {invoice.recurring && (
            <div style={{ marginTop: 8, fontFamily: 'var(--mono)', fontSize: 9.5, color: 'var(--mid)' }}>
              Recurring {invoice.recur_interval} · next: {invoice.next_recur_date ?? '—'}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Page ──────────────────────────────────────────────────────────────────────

export function InvoicesPage() {
  const qc = useQueryClient();
  const [createOpen, setCreateOpen]         = useState(false);
  const [editingInvoice, setEditingInvoice] = useState<InvoiceResponse | null>(null);
  const [filterStatus, setFilterStatus]     = useState<InvoiceStatus | 'all'>('all');
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const invoicesQ = useQuery({
    queryKey: ['invoices'],
    queryFn: () => admin.listInvoices(),
  });

  const clientsQ = useQuery({
    queryKey: ['clients'],
    queryFn: () => admin.listClients(),
  });

  const invoices: InvoiceResponse[] = invoicesQ.data ?? [];
  const clients: Client[] = clientsQ.data ?? [];

  const clientMap = useMemo(() => {
    const m: Record<string, string> = {};
    for (const c of clients) m[c.id] = c.name;
    return m;
  }, [clients]);

  const visible = useMemo(() =>
    filterStatus === 'all' ? invoices : invoices.filter((inv) => inv.status === filterStatus),
    [invoices, filterStatus],
  );

  const statusM = useMutation({
    mutationFn: ({ id, status }: { id: string; status: InvoiceStatus }) =>
      admin.updateInvoice(id, { status } as UpdateInvoicePayload),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['invoices'] }),
  });

  const deleteM = useMutation({
    mutationFn: (id: string) => admin.deleteInvoice(id),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['invoices'] }),
  });

  async function handlePreview(id: string) {
    const html = await api
      .get<string>(`/admin/invoices/${id}/print`, { responseType: 'text' })
      .then((r) => r.data);
    const blob = new Blob([html], { type: 'text/html' });
    const url = URL.createObjectURL(blob);
    const win = window.open(url, '_blank');
    if (win) win.addEventListener('load', () => URL.revokeObjectURL(url), { once: true });
  }

  async function handleDownload(id: string) {
    const blob = await api
      .get<Blob>(`/admin/invoices/${id}/pdf`, { responseType: 'blob' })
      .then((r) => r.data);
    const inv = invoices.find((i) => i.id === id);
    const num = inv?.number ?? id.slice(0, 8);
    downloadBlob(blob, `invoice-${num}.pdf`);
  }

  const counts = useMemo(() => {
    const c = { all: invoices.length, draft: 0, sent: 0, paid: 0 };
    for (const inv of invoices) c[inv.status]++;
    return c;
  }, [invoices]);

  const totalPaid = useMemo(() =>
    invoices
      .filter((inv) => inv.status === 'paid')
      .reduce((acc, inv) => acc + inv.items.reduce((s, it) => s + it.qty * it.rate, 0), 0),
    [invoices],
  );

  return (
    <div className="lg-list grain">
      <Masthead active="invoices" />

      <div className="lg-list__body">
        <div className="lg-list__head">
          <div>
            <h1 className="lg-list__title">
              Invoices
              <span className="lg-list__title-count">/ {String(counts.all).padStart(3, '0')}</span>
            </h1>
            <div className="lg-list__sub">
              {counts.draft} draft · {counts.sent} sent · {counts.paid} paid
            </div>
          </div>
          <div className="lg-list__head-meta">
            <div className="lg-list__kpi">
              <span className="red">{totalPaid.toLocaleString('en-US')}</span>
            </div>
            <div className="lg-list__kpi-lbl">KES paid to date</div>
          </div>
        </div>

        {/* ── Filter + actions ────────────────────────────────────────────── */}
        <div className="lg-list__filt">
          <div className="lg-list__filt-group">
            {(['all', 'draft', 'sent', 'paid'] as const).map((s) => (
              <button
                key={s}
                type="button"
                className={'lg-list__filt-tab' + (filterStatus === s ? ' is-active' : '')}
                onClick={() => setFilterStatus(s)}
              >
                {s === 'all' ? 'All' : STATUS_LABEL[s]} <b>{counts[s]}</b>
              </button>
            ))}
          </div>
          <div className="lg-list__filt-spacer" />
          <button
            type="button"
            className="lg-list__filt-new"
            onClick={() => setCreateOpen(true)}
          >
            + New invoice
          </button>
        </div>

        {/* ── Column headers ───────────────────────────────────────────────── */}
        <div style={{
          display: 'grid',
          gridTemplateColumns: '28px 1fr 120px 120px 110px 110px auto',
          gap: 12,
          padding: '6px 0',
          borderBottom: '2px solid var(--ink)',
          fontFamily: 'var(--mono)',
          fontSize: 9,
          letterSpacing: '.14em',
          textTransform: 'uppercase',
          color: 'var(--mid)',
        }}>
          <span />
          <span>Ref / Client</span>
          <span>Issued</span>
          <span>Due</span>
          <span>Status</span>
          <span>Amount</span>
          <span />
        </div>

        {/* ── Rows ─────────────────────────────────────────────────────────── */}
        <div className="lg-list__rows">
          {invoicesQ.isLoading && (
            <div className="lg-list__state"><h3>Loading invoices…</h3></div>
          )}
          {invoicesQ.isError && (
            <div className="lg-list__state"><h3>Couldn't load invoices.</h3></div>
          )}
          {!invoicesQ.isLoading && visible.length === 0 && (
            <div className="lg-list__state">
              <h3>No invoices yet.</h3>
              {filterStatus === 'all' ? 'Create your first one above.' : `No ${filterStatus} invoices.`}
            </div>
          )}
          {visible.map((inv) => (
            <InvoiceRow
              key={inv.id}
              invoice={inv}
              clientName={
                inv.client_id
                  ? (clientMap[inv.client_id] ?? inv.client_id.slice(0, 8))
                  : `${inv.billed_to_name || '—'} (former client)`
              }
              onStatusChange={(id, status) => statusM.mutate({ id, status })}
              onEdit={(inv) => setEditingInvoice(inv)}
              onDelete={(id) => setConfirmDeleteId(id)}
              onPreview={handlePreview}
              onDownload={handleDownload}
            />
          ))}
        </div>
      </div>

      <BottomTabBar active="invoices" />

      {createOpen && (
        <CreateInvoiceModal
          clients={clients}
          onClose={() => setCreateOpen(false)}
        />
      )}
      {editingInvoice && (
        <EditInvoiceModal
          invoice={editingInvoice}
          onClose={() => setEditingInvoice(null)}
        />
      )}
      {confirmDeleteId && (
        <ConfirmModal
          title="Delete this invoice?"
          body="This cannot be undone."
          confirmLabel="Delete"
          danger
          onConfirm={() => { deleteM.mutate(confirmDeleteId); setConfirmDeleteId(null); }}
          onCancel={() => setConfirmDeleteId(null)}
        />
      )}
    </div>
  );
}
