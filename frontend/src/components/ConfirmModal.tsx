// ConfirmModal — replaces window.confirm() throughout the app.
// Supports Escape to cancel and an optional danger variant for destructive actions.

import { useEffect } from 'react';
import '../styles/v2.css';

export interface ConfirmOptions {
  title: string;
  body: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

interface Props extends ConfirmOptions {
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmModal({
  title,
  body,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  danger = false,
  onConfirm,
  onCancel,
}: Props) {
  useEffect(() => {
    const handle = (e: KeyboardEvent) => { if (e.key === 'Escape') onCancel(); };
    document.addEventListener('keydown', handle);
    return () => document.removeEventListener('keydown', handle);
  }, [onCancel]);

  return (
    <div className="lg-ov" role="dialog" aria-modal aria-labelledby="confirm-modal-title">
      <div className="lg-mdl" style={{ maxWidth: 480, width: 'min(480px, 92vw)' }}>
        <div className="lg-mdl__top">
          <span className="lg-mdl__eye">— Confirmation required</span>
          <span className="lg-mdl__no"><i>Lodgr</i><span className="dot">.</span></span>
          <button type="button" className="lg-mdl__x" onClick={onCancel}>Close ✕</button>
        </div>
        <div className="lg-mdl__body">
          <div id="confirm-modal-title" className="lg-mdl__h1">{title}</div>
          <div className="lg-mdl__dek">{body}</div>
        </div>
        <div className="lg-mdl__foot">
          <span className="meta" />
          <div className="lg-mdl__btns">
            <button type="button" className="lg-bt lg-bt--text" onClick={onCancel}>
              {cancelLabel}
            </button>
            <button
              type="button"
              className={`lg-bt ${danger ? 'lg-bt--danger' : 'lg-bt--solid'}`}
              onClick={onConfirm}
            >
              {confirmLabel} <span className="arr">↗</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
