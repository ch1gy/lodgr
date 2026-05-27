// ─────────────────────────────────────────────────────────────────────────────
// ReportsPage.tsx — /reports (desk only)
//
// Client picker → year → month → download PDF.
//
// GET /reports/monthly/:client_id/:year/:month returns a PDF blob.
// We trigger a browser download by creating an <a> with an object URL.
// ─────────────────────────────────────────────────────────────────────────────

import { useMemo, useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Masthead } from '../components/Masthead';
import { BottomTabBar } from '../components/BottomTabBar';
import { admin } from '../api/admin';
import { api } from '../api/client';
import type { Client } from '../api/types';
import '../styles/v2.css';

const MONTH_NAMES = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
                     'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'] as const;

const NOW = new Date();
const THIS_YEAR  = NOW.getFullYear();
const THIS_MONTH = NOW.getMonth(); // 0-indexed

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a   = document.createElement('a');
  a.href     = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export function ReportsPage() {
  const [selectedClient, setSelectedClient] = useState<Client | null>(null);
  const [clientSearch, setClientSearch]     = useState('');
  const [year, setYear]                     = useState(THIS_YEAR);
  const [month, setMonth]                   = useState(THIS_MONTH === 0 ? 11 : THIS_MONTH - 1); // prev month

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

  const downloadM = useMutation({
    mutationFn: () => {
      if (!selectedClient) throw new Error('No client selected');
      const m = month + 1; // backend is 1-indexed
      return api
        .get<Blob>(`/reports/monthly/${selectedClient.id}/${year}/${m}`, { responseType: 'blob' })
        .then((r) => r.data);
    },
    onSuccess: (blob) => {
      if (!selectedClient) return;
      const m = String(month + 1).padStart(2, '0');
      downloadBlob(blob, `lodgr-report-${selectedClient.id.slice(0, 8)}-${year}-${m}.pdf`);
    },
  });

  // Months available = up to the last completed month of the selected year.
  const isFutureMonth = (y: number, m: number) =>
    y > THIS_YEAR || (y === THIS_YEAR && m >= THIS_MONTH);

  const years = [THIS_YEAR - 1, THIS_YEAR];

  const downloadErr = downloadM.error
    ? ((downloadM.error as { response?: { data?: { error?: string } } })
        .response?.data?.error ?? 'Report generation failed. Try again.')
    : null;

  return (
    <div className="lg-v2">
      <Masthead active="reports" />
      <div className="lg-rp grain">

        {/* ── Left editorial panel ───────────────────────────────── */}
        <div className="lg-rp__left">
          <div className="lg-rp__bgnum">R</div>
          <div className="lg-rp__eye">— Section 03 · Monthly reports</div>
          <h1 className="lg-rp__h1">A month <em>on paper.</em></h1>
          <div className="lg-rp__dek">
            One PDF per client per month — every ticket opened, closed, acknowledged
            or escalated, with response times and a short summary at the top.
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
              <span className="what">Select a year and month</span>
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
            <div className="lg-f">
              <div className="lg-f__lbl"><span>Client</span><span className="req">Required</span></div>
              <input
                className="lg-f__inp"
                placeholder="Search by name or email…"
                value={clientSearch}
                onChange={(e) => {
                  setClientSearch(e.target.value);
                  setSelectedClient(null);
                }}
              />
              {clientSearch && !selectedClient && filtered.length > 0 && (
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

            {/* Year picker */}
            <div className="lg-f">
              <div className="lg-f__lbl"><span>Year</span></div>
              <div className="lg-rp__pickyr">
                {years.map((y) => (
                  <button
                    key={y}
                    type="button"
                    className={`yr${year === y ? ' on' : ''}`}
                    onClick={() => setYear(y)}
                  >
                    {y}
                  </button>
                ))}
              </div>
            </div>

            {/* Month picker */}
            <div className="lg-f">
              <div className="lg-f__lbl"><span>Month</span><span className="opt">{year}</span></div>
              <div className="lg-rp__months">
                {MONTH_NAMES.map((name, i) => {
                  const future = isFutureMonth(year, i);
                  return (
                    <button
                      key={name}
                      type="button"
                      className={`lg-rp__mo${month === i ? ' on' : ''}${future ? ' dis' : ''}`}
                      disabled={future}
                      onClick={() => !future && setMonth(i)}
                    >
                      <span className="n">{name}</span>
                      <span className="c">{future ? '·' : '—'}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Preview box */}
            {selectedClient && (
              <div className="lg-rp__preview">
                <div className="ttl">— Preview</div>
                <div className="row">
                  <span className="k">File</span>
                  <span className="v">
                    lodgr-report-{selectedClient.id.slice(0, 8)}-{year}-{String(month + 1).padStart(2, '0')}.pdf
                  </span>
                </div>
                <div className="row">
                  <span className="k">Client</span>
                  <span className="v">{selectedClient.name}</span>
                </div>
                <div className="row">
                  <span className="k">Period</span>
                  <span className="v">{MONTH_NAMES[month]} {year}</span>
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
                disabled={!selectedClient || downloadM.isPending}
                onClick={() => downloadM.mutate()}
              >
                {downloadM.isPending ? 'Generating…' : 'Download PDF'} <span className="arr">↓</span>
              </button>
            </div>
          </div>
        </div>
      </div>
      <BottomTabBar active="reports" />
    </div>
  );
}
