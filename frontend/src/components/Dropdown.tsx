// Dropdown.tsx — scales open from its trigger; outside-click + Esc close (§6.6c, §9.6)
// Menu is portalled to <body> so overflow:hidden/auto parents don't clip it.

import { useEffect, useLayoutEffect, useRef, useState, ReactNode } from 'react';
import { createPortal } from 'react-dom';
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
  const triggerRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [menuPos, setMenuPos] = useState({ top: 0, left: 0, right: 'auto' as string | number });

  // Position the portalled menu below the trigger.
  useLayoutEffect(() => {
    if (!open || !triggerRef.current) return;
    const r = triggerRef.current.getBoundingClientRect();
    if (align === 'right') {
      setMenuPos({ top: r.bottom + 6, left: 'auto' as unknown as number, right: window.innerWidth - r.right });
    } else {
      setMenuPos({ top: r.bottom + 6, left: r.left, right: 'auto' });
    }
  }, [open, align]);

  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      const t = e.target as Node;
      if (
        triggerRef.current && !triggerRef.current.contains(t) &&
        menuRef.current && !menuRef.current.contains(t)
      ) {
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

  const menuStyle: React.CSSProperties =
    align === 'right'
      ? { position: 'fixed', top: menuPos.top, right: menuPos.right, zIndex: 9999 }
      : { position: 'fixed', top: menuPos.top, left: menuPos.left, zIndex: 9999 };

  return (
    <div ref={triggerRef} className="lg-dd" style={{ position: 'relative', display: 'inline-block' }}>
      <div onClick={() => setOpen((v) => !v)}>{trigger}</div>
      {open && createPortal(
        <div ref={menuRef} className={`lg-dd__menu lg-dd__menu--${align}`} role="menu" style={menuStyle}>
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
        </div>,
        document.body
      )}
    </div>
  );
}
