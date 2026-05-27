// ─────────────────────────────────────────────────────────────────────────────
// MagicLandingPage.tsx — handles /auth/magic?token=…
//
// Desk generates a ticket-scoped link via POST /tickets/:id/magic-link OR a
// full-session client link via POST /admin/clients/:id/magic-link. Either way
// the client lands here with `?token=<one-time>`. We POST it to /auth/magic,
// which returns a short-lived access token (no refresh cookie — magic sessions
// are stateless, see handoff).
//
// On success we route to the ticket list or, for scoped sessions, directly to
// the one ticket the client was shared.
//
// NOTE: the URL token is a server-side one-time UUID, NOT a JWT. We decode
// the ACCESS TOKEN returned by the server (via redeemMagicLink) to read the
// ticket_scope claim — never the URL token.
// ─────────────────────────────────────────────────────────────────────────────

import { useEffect, useRef, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { useAuth, safeDecode } from '../auth/AuthContext';
import '../styles/login.css';

type State =
  | { status: 'redeeming' }
  | { status: 'bad'; message: string };

export function MagicLandingPage() {
  const [params] = useSearchParams();
  const token = params.get('token');
  const nav = useNavigate();
  const { redeemMagicLink } = useAuth();
  const [state, setState] = useState<State>({ status: 'redeeming' });

  // Guard against StrictMode double-invoke in dev, which would otherwise
  // consume the one-shot token twice.
  const consumed = useRef(false);

  useEffect(() => {
    if (!token) {
      setState({ status: 'bad', message: 'No token in the URL. Ask the desk to send you a fresh link.' });
      return;
    }
    if (consumed.current) return;
    consumed.current = true;

    (async () => {
      try {
        const accessJwt = await redeemMagicLink(token);
        const payload = safeDecode(accessJwt);
        nav(
          payload?.ticket_scope ? `/tickets/${payload.ticket_scope}` : '/tickets',
          { replace: true }
        );
      } catch (e) {
        setState({
          status: 'bad',
          message:
            (e as { message?: string })?.message ||
            "That link didn't work — it may have expired or been used already.",
        });
      }
    })();
  }, [token, redeemMagicLink, nav]);

  // ── UI ─────────────────────────────────────────────────────────────────
  if (state.status === 'redeeming') {
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

  return (
    <div className="lg-magic-land grain">
      <div className="lg-magic-land__card">
        <div className="lg-magic-land__logo"><i>Lodgr</i><span className="dot">.</span></div>
        <div className="lg-magic-land__sub">— Link didn't work —</div>
        <h1 className="lg-magic-land__h1">That <em>link</em> can't be used.</h1>
        <p className="lg-magic-land__lede">{state.message}</p>
        <Link to="/login" className="lg-magic-land__back">Go to sign-in</Link>
      </div>
    </div>
  );
}
