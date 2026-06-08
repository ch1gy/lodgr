// CommandPalette.tsx — ⌘K command palette (§8)
//
// Opens on Cmd+K / Ctrl+K. Searches tickets (from cache) and provides
// quick actions. Arrow keys navigate, Enter selects, Esc closes.
//
// Context is provided here so any component can open it programmatically.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  ReactNode,
} from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { tickets as ticketsApi } from '../api/tickets';
import { admin } from '../api/admin';
import { useAuth } from '../auth/AuthContext';
import type { TicketResponse, Client } from '../api/types';
import '../styles/palette.css';

interface PaletteCtxShape {
  open: () => void;
  close: () => void;
}

const PaletteCtx = createContext<PaletteCtxShape>({ open: () => {}, close: () => {} });

export function useCommandPalette() {
  return useContext(PaletteCtx);
}

interface ResultItem {
  id: string;
  title: string;
  meta: string;
  av: string;
  tag?: string;
  action: () => void;
}

function clientInitials(id: string) {
  return id.slice(0, 2).toUpperCase();
}

export function CommandPaletteProvider({ children }: { children: ReactNode }) {
  const [isOpen, setIsOpen] = useState(false);
  const [query, setQuery]   = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();
  const { isDesk } = useAuth();

  const open  = useCallback(() => { setIsOpen(true);  setQuery(''); setActive(0); }, []);
  const close = useCallback(() => { setIsOpen(false); setQuery(''); }, []);

  // Global keyboard shortcut
  useEffect(() => {
    function handle(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setIsOpen((v) => { if (!v) { setQuery(''); setActive(0); } return !v; });
      }
    }
    document.addEventListener('keydown', handle);
    return () => document.removeEventListener('keydown', handle);
  }, []);

  // Focus input when open
  useEffect(() => {
    if (isOpen) setTimeout(() => inputRef.current?.focus(), 50);
  }, [isOpen]);

  const ticketsQ = useQuery({
    queryKey: ['tickets'],
    queryFn: () => ticketsApi.list(),
    enabled: isOpen,
    staleTime: 30_000,
  });

  const clientsQ = useQuery({
    queryKey: ['clients'],
    queryFn: () => admin.listClients(),
    enabled: isOpen && isDesk,
    staleTime: 60_000,
  });

  const allTickets: TicketResponse[] = (ticketsQ.data as { tickets?: TicketResponse[] } | TicketResponse[] | undefined)
    ? Array.isArray(ticketsQ.data) ? ticketsQ.data : ((ticketsQ.data as { tickets?: TicketResponse[] }).tickets ?? [])
    : [];

  const allClients: Client[] = (clientsQ.data ?? []).filter((c) => !c.deleted_at);

  const q = query.toLowerCase().trim();

  const ticketResults: ResultItem[] = (q
    ? allTickets.filter((t) =>
        t.title.toLowerCase().includes(q) ||
        t.id.toLowerCase().includes(q) ||
        (t.category ?? '').toLowerCase().includes(q)
      )
    : allTickets.slice(0, 5)
  )
    .slice(0, 6)
    .map((t) => ({
      id: `t-${t.id}`,
      title: t.title,
      meta: `${t.status} · ${t.id.slice(0, 8)}`,
      av: clientInitials(t.client_id),
      tag: t.status,
      action: () => { navigate(`/tickets/${t.id}`); close(); },
    }));

  const clientResults: ResultItem[] = isDesk && q
    ? allClients
        .filter((c) => c.name.toLowerCase().includes(q) || c.email.toLowerCase().includes(q))
        .slice(0, 4)
        .map((c) => ({
          id: `c-${c.id}`,
          title: c.name,
          meta: c.email,
          av: c.name.slice(0, 2).toUpperCase(),
          tag: 'client',
          action: () => { navigate('/clients'); close(); },
        }))
    : [];

  const staticActions: ResultItem[] = [
    { id: 'a-queue',    title: 'Open ticket queue', meta: '/tickets',  av: '↗', action: () => { navigate('/tickets'); close(); } },
    { id: 'a-clients',  title: 'Clients roster',    meta: '/clients',  av: '↗', action: () => { navigate('/clients'); close(); } },
    { id: 'a-reports',  title: 'Monthly reports',   meta: '/reports',  av: '↗', action: () => { navigate('/reports'); close(); } },
    { id: 'a-settings', title: 'Settings',          meta: '/settings', av: '↗', action: () => { navigate('/settings'); close(); } },
  ].filter((a) => {
    if (!q) return true;
    return a.title.toLowerCase().includes(q) || a.meta.toLowerCase().includes(q);
  });

  // Flatten for keyboard nav
  const allResults = [...ticketResults, ...clientResults, ...staticActions];

  function handleKey(e: React.KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActive((v) => Math.min(v + 1, allResults.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive((v) => Math.max(v - 1, 0));
    } else if (e.key === 'Enter') {
      allResults[active]?.action();
    } else if (e.key === 'Escape') {
      close();
    }
  }

  // Reset active when results change
  useEffect(() => { setActive(0); }, [query]);

  return (
    <PaletteCtx.Provider value={{ open, close }}>
      {children}
      {isOpen && (
        <div className="lg-pal-ov" onMouseDown={(e) => { if (e.target === e.currentTarget) close(); }}>
          <div className="lg-pal" role="dialog" aria-label="Command palette" aria-modal>
            {/* Search row */}
            <div className="lg-pal__search">
              <span className="lg-pal__icon">⌘</span>
              <input
                ref={inputRef}
                className="lg-pal__inp"
                placeholder="Search tickets, clients, actions…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={handleKey}
                autoComplete="off"
                spellCheck={false}
              />
              {allResults.length > 0 && (
                <span className="lg-pal__count">{allResults.length}</span>
              )}
            </div>

            {/* Results */}
            <div className="lg-pal__results" role="listbox">
              {allResults.length === 0 && q && (
                <div className="lg-pal__empty">No results for "{query}"</div>
              )}

              {ticketResults.length > 0 && (
                <>
                  <div className="lg-pal__sec">Tickets</div>
                  {ticketResults.map((item, i) => (
                    <button
                      key={item.id}
                      type="button"
                      role="option"
                      aria-selected={active === i}
                      className={`lg-pal__item${active === i ? ' is-active' : ''}`}
                      onClick={item.action}
                      onMouseEnter={() => setActive(i)}
                    >
                      <span className="lg-pal__item__av">{item.av}</span>
                      <span className="lg-pal__item__body">
                        <span className="lg-pal__item__title">{item.title}</span>
                        <span className="lg-pal__item__meta">{item.meta}</span>
                      </span>
                      {item.tag && <span className="lg-pal__item__tag">{item.tag}</span>}
                    </button>
                  ))}
                </>
              )}

              {clientResults.length > 0 && (
                <>
                  <div className="lg-pal__sec">Clients</div>
                  {clientResults.map((item, i) => {
                    const idx = ticketResults.length + i;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        role="option"
                        aria-selected={active === idx}
                        className={`lg-pal__item${active === idx ? ' is-active' : ''}`}
                        onClick={item.action}
                        onMouseEnter={() => setActive(idx)}
                      >
                        <span className="lg-pal__item__av">{item.av}</span>
                        <span className="lg-pal__item__body">
                          <span className="lg-pal__item__title">{item.title}</span>
                          <span className="lg-pal__item__meta">{item.meta}</span>
                        </span>
                        {item.tag && <span className="lg-pal__item__tag">{item.tag}</span>}
                      </button>
                    );
                  })}
                </>
              )}

              {staticActions.length > 0 && (
                <>
                  <div className="lg-pal__sec">Actions</div>
                  {staticActions.map((item, i) => {
                    const idx = ticketResults.length + clientResults.length + i;
                    return (
                      <button
                        key={item.id}
                        type="button"
                        role="option"
                        aria-selected={active === idx}
                        className={`lg-pal__item${active === idx ? ' is-active' : ''}`}
                        onClick={item.action}
                        onMouseEnter={() => setActive(idx)}
                      >
                        <span className="lg-pal__item__av">{item.av}</span>
                        <span className="lg-pal__item__body">
                          <span className="lg-pal__item__title">{item.title}</span>
                          <span className="lg-pal__item__meta">{item.meta}</span>
                        </span>
                      </button>
                    );
                  })}
                </>
              )}
            </div>

            {/* Footer hints */}
            <div className="lg-pal__foot">
              <span className="lg-pal__hint"><kbd>↑↓</kbd> navigate</span>
              <span className="lg-pal__hint"><kbd>↵</kbd> open</span>
              <span className="lg-pal__hint"><kbd>Esc</kbd> close</span>
            </div>
          </div>
        </div>
      )}
    </PaletteCtx.Provider>
  );
}
