// ─────────────────────────────────────────────────────────────────────────────
// MagicLinkModal.tsx — QR code + copy URL modal for magic link delivery.
//
// Used in two contexts:
//   • TicketDetailPage  — scope="ticket", desk shares a scoped link with client
//   • ClientsPage       — scope="full",   desk issues a full-session link
//
// Both contexts deliver the same backend URL — only the JWT claim inside it
// differs. The modal doesn't know or care about the JWT contents.
//
// Props:
//   url          — The full magic link URL (e.g. https://…/magic?token=…)
//   scope        — 'ticket' | 'full' — drives the descriptive copy only
//   ticketId     — Optional short ticket ID shown in scoped copy
//   onClose      — Dismiss callback
//   onRegenerate — Optional async callback to mint a new link. When provided,
//                  shows the "Regenerate" button.
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useRef, useState } from 'react';
import QRCode from 'react-qr-code';
import '../styles/v2.css';

interface Props {
  url: string;
  scope: 'ticket' | 'full';
  ticketId?: string;
  onClose: () => void;
  onRegenerate?: () => Promise<void>;
}

export function MagicLinkModal({ url, scope, ticketId, onClose, onRegenerate }: Props) {
  const [copied, setCopied]       = useState(false);
  const [regen, setRegen]         = useState(false);
  const qrRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handle = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', handle);
    return () => document.removeEventListener('keydown', handle);
  }, [onClose]);

  // QR assemble stagger (§6.5) — animate each module rect on mount
  useEffect(() => {
    const container = qrRef.current;
    if (!container) return;
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const rects = Array.from(container.querySelectorAll('svg rect'));
    rects.forEach((rect, i) => {
      const el = rect as SVGRectElement;
      // Detect finder-pattern squares by checking if they're near the corners of the QR
      const svgWidth = el.ownerSVGElement?.viewBox?.baseVal?.width ?? 200;
      const x = parseFloat(el.getAttribute('x') ?? '0');
      const y = parseFloat(el.getAttribute('y') ?? '0');
      const cornerSize = svgWidth * 0.4;
      const isFinderArea =
        (x < cornerSize && y < cornerSize) ||               // top-left
        (x > svgWidth - cornerSize && y < cornerSize) ||    // top-right
        (x < cornerSize && y > svgWidth - cornerSize);      // bottom-left
      if (isFinderArea) return;

      el.style.opacity = '0';
      el.style.transform = 'scale(0.3)';
      const cx = parseFloat(el.getAttribute('x') ?? '0') + parseFloat(el.getAttribute('width') ?? '4') / 2;
      const cy = parseFloat(el.getAttribute('y') ?? '0') + parseFloat(el.getAttribute('height') ?? '4') / 2;
      el.style.transformOrigin = `${cx}px ${cy}px`;
      el.style.transition = `opacity 120ms ease-out ${i * 3}ms, transform 180ms cubic-bezier(0.34, 1.56, 0.64, 1) ${i * 3}ms`;

      requestAnimationFrame(() => {
        el.style.opacity = '1';
        el.style.transform = 'scale(1)';
      });
    });
  }, [url]);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch { /* clipboard unavailable — the url-box is still selectable */ }
  }

  async function handleRegenerate() {
    if (!onRegenerate) return;
    setRegen(true);
    try { await onRegenerate(); } finally { setRegen(false); }
  }

  return (
    <div className="lg-ov" role="dialog" aria-modal aria-label="Magic link">
      <div className="lg-mdl magic">
        {/* ── Header ──────────────────────────────────────────────── */}
        <div className="lg-mdl__top">
          <span className="lg-mdl__eye">
            — {scope === 'ticket' ? 'Section 02 — Ticket access link' : 'Section 02 — Client access link'}
          </span>
          <span className="lg-mdl__no"><i>Lodgr</i><span className="dot">.</span></span>
          <button type="button" className="lg-mdl__x" onClick={onClose}>Close ✕</button>
        </div>

        {/* ── Body ────────────────────────────────────────────────── */}
        <div className="lg-mdl__body">
          {/* QR code — always light-on-cream regardless of theme; scanners need contrast. */}
          <div className="qr-frame" ref={qrRef}>
            <QRCode
              value={url}
              size={200}
              bgColor="#f2ede4"
              fgColor="#0d0d0d"
              level="M"
            />
          </div>

          <div className="h1">
            {scope === 'ticket'
              ? <>Hand them <em>the desk.</em></>
              : <>One-tap <em>full session.</em></>}
          </div>

          <div className="note">
            {scope === 'ticket' && ticketId
              ? <>Scoped to <b style={{ color: 'var(--ink)' }}>{ticketId}</b><span className="sep">/</span></>
              : null}
            {scope === 'full'
              ? <>Full session<span className="sep">/</span></>
              : null}
            Single use<span className="sep">/</span>Link expires in 24h
          </div>

          {/* Copy URL box */}
          <button
            type="button"
            className={`url-box${copied ? ' copied' : ''}`}
            onClick={() => { void handleCopy(); }}
            title="Click to copy URL"
          >
            {url}
          </button>
          <div className="url-hint">
            {copied ? '✓ Copied to clipboard' : 'Click to copy URL'}
          </div>

          {/* Usage hints */}
          <div className="why">
            <div className="it">
              <span className="k">Scan</span>
              <span className="v">In-person or video call</span>
            </div>
            <div className="it">
              <span className="k">Copy</span>
              <span className="v">SMS, email, anything else</span>
            </div>
          </div>
        </div>

        {/* ── Footer ──────────────────────────────────────────────── */}
        <div className="lg-mdl__foot">
          <span className="meta">Single-use · link expires 24h from generation · session lasts 24h</span>
          <div className="lg-mdl__btns">
            {onRegenerate && (
              <button
                type="button"
                className="lg-bt lg-bt--text"
                disabled={regen}
                onClick={() => { void handleRegenerate(); }}
              >
                {regen ? 'Generating…' : 'Regenerate ⟳'}
              </button>
            )}
            <button type="button" className="lg-bt lg-bt--solid" onClick={onClose}>
              Close <span className="arr">↗</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
