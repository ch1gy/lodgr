import { describe, it, expect } from 'vitest';
import { extractApiError } from './format';

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
