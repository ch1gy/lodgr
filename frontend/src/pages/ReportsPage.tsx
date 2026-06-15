import { useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Masthead } from '../components/Masthead';
import { admin } from '../api/admin';
import { api } from '../api/client';
import { downloadBlob, extractApiError } from '../utils/format';
import type { Client } from '../api/types';
import '../styles/v2.css';

function toDateString(d: Date): string {
  return d.toISOString().slice(0, 10);
}

export function ReportsPage() {
  const now = new Date();
  const firstOfMonth = new Date(now.getFullYear(), now.getMonth(), 1);
  const today = toDateString(now);

  const [selectedClient, setSelectedClient] = useState<Client | null>(null);
  const [clientSearch, setClientSearch]     = useState('');
  const [pickerOpen, setPickerOpen]         = useState(false);
  const [from, setFrom] = useState(toDateString(firstOfMonth));
  const [to, setTo]     = useState(today);

  const pickerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!pickerOpen) return;
    function handleOutside(e: MouseEvent) {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setPickerOpen(false);
      }
    }
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [pickerOpen]);

  const clientsQ = useQuery({
    queryKey: ['clients'],
    queryFn: () => admin.listClients(),
  });

  const activeClients = useMemo(
    () => (clientsQ.data ?? []).filter((c) => !c.deleted_at),
    [clientsQ.data],
  );

  const filtered = useMemo(() => {
    const q = clientSearch.toLowerCase();
    return q
      ? activeClients.filter((c) => c.name.toLowerCase().includes(q) || c.email.toLowerCase().includes(q))
      : activeClients;
  }, [activeClients, clientSearch]);

  const rangeValid = from && to && from <= to;

  const downloadM = useMutation({
    mutationFn: () => {
      if (!selectedClient) throw new Error('No client selected');
      return api
        .get<Blob>(`/reports/range/${selectedClient.id}?from=${from}&to=${to}`, { responseType: 'blob' })
        .then((r) => r.data);
    },
    onSuccess: (blob) => {
      if (!selectedClient) return;
      downloadBlob(blob, `lodgr-report-${selectedClient.id.slice(0, 8)}-${from}-${to}.pdf`);
    },
  });

  const downloadErr = downloadM.error
    ? extractApiError(downloadM.error, 'Report generation failed. Try again.')
    : null;

  return (
    <div className="lg-v2">
      <Masthead active="reports" />
      <div className="lg-rp grain">

        {/* ── Left editorial panel ───────────────────────────────── */}
        <div className="lg-rp__left">
          <div className="lg-rp__bgnum">R</div>
          <div className="lg-rp__eye">— Section 03 · Reports</div>
          <h1 className="lg-rp__h1">A period <em>on paper.</em></h1>
          <div className="lg-rp__dek">
            One PDF per client per date range — every ticket opened, closed, acknowledged
            or escalated, with a short summary at the top.
            Generated server-side. Downloads directly.
          </div>

          <div className="lg-rp__recent">
            <div className="lbl">— Instructions</div>
            <div className="it">
              <span className="when">Step 01</span>
              <span className="what">Choose a client from the picker →</span>
              <span className="who" />
            </div>
            <div className="it">
              <span className="when">Step 02</span>
              <span className="what">Set a from and to date</span>
              <span className="who" />
            </div>
            <div className="it">
              <span className="when">Step 03</span>
              <span className="what">Hit Download PDF</span>
              <span className="who">~3 sec</span>
            </div>
          </div>
        </div>

        {/* ── Right form panel ───────────────────────────────────── */}
        <div className="lg-rp__right">
          <div className="lg-rp__form">

            {/* Client picker */}
            <div ref={pickerRef} className="lg-f">
              <div className="lg-f__lbl"><span>Client</span><span className="req">Required</span></div>
              <input
                className="lg-f__inp"
                placeholder="Search by name or email…"
                value={clientSearch}
                onFocus={() => setPickerOpen(true)}
                onChange={(e) => {
                  setClientSearch(e.target.value);
                  setSelectedClient(null);
                  setPickerOpen(true);
                }}
              />
              {pickerOpen && !selectedClient && filtered.length > 0 && (
                <div style={{
                  border: '1px solid var(--ink)',
                  borderTop: 'none',
                  background: 'var(--cream)',
                  maxHeight: 160,
                  overflowY: 'auto',
                }}>
                  {filtered.map((c) => (
                    <button
                      key={c.id}
                      type="button"
                      style={{
                        display: 'block', width: '100%', textAlign: 'left',
                        padding: '10px 14px', background: 'none', border: 'none',
                        borderBottom: '1px solid var(--rule)', cursor: 'pointer',
                      }}
                      onClick={() => {
                        setSelectedClient(c);
                        setClientSearch(c.name);
                        setPickerOpen(false);
                      }}
                    >
                      <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 16 }}>{c.name}</span>{' '}
                      <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--mid)' }}>{c.email}</span>
                    </button>
                  ))}
                </div>
              )}
              {selectedClient && (
                <span className="lg-f__hint">{selectedClient.email} · {selectedClient.id.slice(0, 8)}</span>
              )}
            </div>

            {/* Date range */}
            <div className="lg-rp__daterange">
              <div className="lg-f">
                <div className="lg-f__lbl"><span>From</span></div>
                <input
                  type="date"
                  className="lg-f__inp"
                  value={from}
                  max={to || today}
                  onChange={(e) => setFrom(e.target.value)}
                />
              </div>
              <div className="lg-f">
                <div className="lg-f__lbl"><span>To</span></div>
                <input
                  type="date"
                  className="lg-f__inp"
                  value={to}
                  min={from}
                  max={today}
                  onChange={(e) => setTo(e.target.value)}
                />
              </div>
            </div>

            {/* Preview box */}
            {selectedClient && rangeValid && (
              <div className="lg-rp__preview">
                <div className="ttl">— Preview</div>
                <div className="row">
                  <span className="k">File</span>
                  <span className="v">lodgr-report-{selectedClient.id.slice(0, 8)}-{from}-{to}.pdf</span>
                </div>
                <div className="row">
                  <span className="k">Client</span>
                  <span className="v">{selectedClient.name}</span>
                </div>
                <div className="row">
                  <span className="k">Period</span>
                  <span className="v">{from} – {to}</span>
                </div>
                <div className="row">
                  <span className="k">Format</span>
                  <span className="v">PDF · server-generated</span>
                </div>
              </div>
            )}

            {downloadErr && <div className="lg-f__err">{downloadErr}</div>}

            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 4 }}>
              <span className="lg-f__hint" style={{ margin: 0 }}>
                Generated server-side · ~3 sec · downloads when ready
              </span>
              <button
                type="button"
                className="lg-bt lg-bt--solid"
                disabled={!selectedClient || !rangeValid || downloadM.isPending}
                onClick={() => downloadM.mutate()}
              >
                {downloadM.isPending ? 'Generating…' : 'Download PDF'} <span className="arr">↓</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
