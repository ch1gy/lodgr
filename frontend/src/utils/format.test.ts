import { describe, it, expect } from 'vitest';
import { addInterval, describeTtl, extractApiError } from './format';

// These cases mirror backend/tests/invoices.rs::advance_recur_date_* so the form's
// default "next date" agrees with what the scheduler will actually produce.
describe('addInterval', () => {
  it('adds one month', () => {
    expect(addInterval('2025-03-15', 'monthly')).toBe('2025-04-15');
  });

  it('clamps to the end of a shorter month', () => {
    expect(addInterval('2025-01-31', 'monthly')).toBe('2025-02-28');
    expect(addInterval('2024-01-31', 'monthly')).toBe('2024-02-29');
  });

  it('adds three months for quarterly', () => {
    expect(addInterval('2025-01-01', 'quarterly')).toBe('2025-04-01');
  });

  it('adds twelve months for yearly, including across a year boundary', () => {
    expect(addInterval('2025-03-15', 'yearly')).toBe('2026-03-15');
    expect(addInterval('2025-11-30', 'quarterly')).toBe('2026-02-28');
  });

  it('clamps a leap day when the next year has none', () => {
    expect(addInterval('2024-02-29', 'yearly')).toBe('2025-02-28');
  });

  it('returns an empty string for input that is not a date', () => {
    expect(addInterval('', 'monthly')).toBe('');
    expect(addInterval('not-a-date', 'monthly')).toBe('');
  });
});

describe('extractApiError', () => {
  it('returns the server error string when present', () => {
    const err = { response: { data: { error: 'Email already in use' } } };
    expect(extractApiError(err)).toBe('Email already in use');
  });

  it('returns the fallback when response.data.error is missing', () => {
    const err = { response: { data: {} } };
    expect(extractApiError(err, 'fallback message')).toBe('fallback message');
  });

  it('returns the fallback when there is no response at all', () => {
    const err = new Error('Network Error');
    expect(extractApiError(err, 'no network')).toBe('no network');
  });

  it('uses the default fallback when none is supplied', () => {
    expect(extractApiError(null)).toBe('Something went wrong');
  });

  it('returns the fallback for undefined', () => {
    expect(extractApiError(undefined, 'custom')).toBe('custom');
  });
});

// Mirrors backend/tests/magic.rs::describe_ttl_uses_the_configured_lifetime.
describe('describeTtl', () => {
  it('words whole hours and minutes the way the email does', () => {
    expect(describeTtl(3600)).toBe('1 hour');
    expect(describeTtl(86400)).toBe('24 hours');
    expect(describeTtl(900)).toBe('15 minutes');
    expect(describeTtl(60)).toBe('1 minute');
    expect(describeTtl(5400)).toBe('90 minutes');
  });

  it('never reports a zero or negative lifetime', () => {
    expect(describeTtl(0)).toBe('1 minute');
    expect(describeTtl(-5)).toBe('1 minute');
  });
});
