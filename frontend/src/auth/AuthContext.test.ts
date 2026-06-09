import { describe, it, expect } from 'vitest';
import { safeDecode } from './AuthContext';

// A minimal JWT with known payload — not cryptographically valid but structurally correct.
// Header: {"alg":"HS256","typ":"JWT"}
// Payload: {"sub":"user-1","role":"desk","exp":9999999999,"session_type":"full","jti":null}
const DESK_TOKEN =
  'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.' +
  'eyJzdWIiOiJ1c2VyLTEiLCJyb2xlIjoiZGVzayIsImV4cCI6OTk5OTk5OTk5OSwic2Vzc2lvbl90eXBlIjoiZnVsbCIsImp0aSI6bnVsbH0.' +
  'SIGNATURE';

// Payload: {"sub":"user-2","role":"client","exp":9999999999,"session_type":"scoped","ticket_scope":"ticket-99","jti":"some-jti"}
const SCOPED_CLIENT_TOKEN =
  'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.' +
  'eyJzdWIiOiJ1c2VyLTIiLCJyb2xlIjoiY2xpZW50IiwiZXhwIjo5OTk5OTk5OTk5LCJzZXNzaW9uX3R5cGUiOiJzY29wZWQiLCJ0aWNrZXRfc2NvcGUiOiJ0aWNrZXQtOTkiLCJqdGkiOiJzb21lLWp0aSJ9.' +
  'SIGNATURE';

describe('safeDecode', () => {
  it('returns null for null input', () => {
    expect(safeDecode(null)).toBeNull();
  });

  it('returns null for a malformed token', () => {
    expect(safeDecode('not.a.token')).toBeNull();
  });

  it('returns null for an empty string', () => {
    expect(safeDecode('')).toBeNull();
  });

  it('decodes a desk full-session token', () => {
    const payload = safeDecode(DESK_TOKEN);
    expect(payload).not.toBeNull();
    expect(payload!.sub).toBe('user-1');
    expect(payload!.role).toBe('desk');
    expect(payload!.session_type).toBe('full');
  });

  it('decodes a scoped client token and exposes ticket_scope', () => {
    const payload = safeDecode(SCOPED_CLIENT_TOKEN);
    expect(payload).not.toBeNull();
    expect(payload!.role).toBe('client');
    expect(payload!.session_type).toBe('scoped');
    expect(payload!.ticket_scope).toBe('ticket-99');
    expect(payload!.jti).toBe('some-jti');
  });
});
