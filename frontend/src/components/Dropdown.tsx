// Dropdown.tsx — scales open from its trigger; outside-click + Esc close (§6.6c, §9.6)

import { useEffect, useRef, useState, ReactNode } from 'react';
import '../styles/dropdown.css';

export interface DropdownItem {
  label: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
}

interface Props {
  trigger: ReactNode;
  items: DropdownItem[];
  align?: 'left' | 'right';
}

export function Dropdown({ trigger, items, align = 'left' }: Props) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === 'Escape') setOpen(false);
    }
    document.addEventListener('mousedown', handleClick);
    document.addEventListener('keydown', handleKey);
    return () => {
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="lg-dd" style={{ position: 'relative', display: 'inline-block' }}>
      <div onClick={() => setOpen((v) => !v)}>{trigger}</div>
      {open && (
        <div className={`lg-dd__menu lg-dd__menu--${align}`} role="menu">
          {items.map((item, i) => (
            <button
              key={i}
              type="button"
              role="menuitem"
              disabled={item.disabled}
              className={`lg-dd__item${item.danger ? ' danger' : ''}`}
              onClick={() => { item.onClick(); setOpen(false); }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
