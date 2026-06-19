// ─────────────────────────────────────────────────────────────────────────────
// MagicLandingPage.tsx — handles /magic?token=…
//
// Desk generates a ticket-scoped link via POST /tickets/:id/magic-link OR a
// full-session client link via POST /admin/clients/:id/magic-link. Either way
// the client lands here with `?token=<one-time>`. We POST it to /auth/magic,
// which returns an access token (no refresh cookie — magic sessions are
// stateless and short-lived, see handoff).
//
// On success we route to the ticket list (or to the scoped ticket if the JWT
// carries a ticket_scope claim).
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useRef, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { useAuth } from '../auth/AuthContext';
import { jwtDecode } from 'jwt-decode';
import type { JwtPayload } from '../api/types';
import '../styles/login.css';

type State = 'redeeming' | 'ok' | 'bad';

export function MagicLandingPage() {
  const [params] = useSearchParams();
  const token = params.get('token');
  const nav = useNavigate();
  const { redeemMagicLink } = useAuth();
  const [state, setState] = useState<State>('redeeming');
  const [errMsg, setErrMsg] = useState<string>('');

  // We use a ref to guard against StrictMode double-invoke in dev, which
  // would otherwise consume the one-shot token twice.
  const consumed = useRef(false);

  useEffect(() => {
    if (!token) {
      setState('bad');
      setErrMsg('No token in the URL. Ask the desk to send you a fresh link.');
      return;
    }
    if (consumed.current) return;
    consumed.current = true;

    (async () => {
      try {
        await redeemMagicLink(token);

        // Peek at the JWT to decide where to land. Scoped tokens jump straight
        // to that one ticket; full sessions land on the list.
        // (No verification needed client-side — the server already validated.)
        try {
          const payload = jwtDecode<JwtPayload>(token);
          if (payload.ticket_scope) {
            setState('ok');
            nav(`/tickets/${payload.ticket_scope}`, { replace: true });
            return;
          }
        } catch {
          /* fall through — we'll just send them to the list */
        }

        setState('ok');
        nav('/tickets', { replace: true });
      } catch (e) {
        setState('bad');
        setErrMsg(
          (e as { message?: string })?.message ||
            'That link didn\'t work — it may have expired or been used already.'
        );
      }
    })();
  }, [token, redeemMagicLink, nav]);

  // ── UI ─────────────────────────────────────────────────────────────────
  if (state === 'redeeming') {
    return (
      <div className="lg-magic-land grain">
        <div className="lg-magic-land__card">
          <div className="lg-magic-land__logo"><i>Lodgr</i><span className="dot">.</span></div>
          <div className="lg-magic-land__sub">— Signing you in —</div>
          <p className="lg-magic-land__lede">
            One moment while we exchange your link for a session.
          </p>
        </div>
      </div>
    );
  }

  // state === 'bad'
  return (
    <div className="lg-magic-land grain">
      <div className="lg-magic-land__card">
        <div className="lg-magic-land__logo"><i>Lodgr</i><span className="dot">.</span></div>
        <div className="lg-magic-land__sub">— Link didn't work —</div>
        <h1 className="lg-magic-land__h1">That <em>link</em> can't be used.</h1>
        <p className="lg-magic-land__lede">{errMsg}</p>
        <Link to="/login" className="lg-magic-land__back">Go to sign-in</Link>
      </div>
    </div>
  );
}
