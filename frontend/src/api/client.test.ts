import { describe, it, expect } from 'vitest';
import { AxiosError } from 'axios';
import { unwrapBlobError } from './client';
import { extractApiError } from '../utils/format';

function blobError(body: string, type: string): AxiosError {
  const err = new AxiosError('Request failed with status code 400');
  err.response = {
    data: new Blob([body], { type }),
    status: 400,
    statusText: 'Bad Request',
    headers: {},
    config: {} as never,
  };
  return err;
}

describe('unwrapBlobError', () => {
  it('turns a JSON error Blob into an object so the server message is readable', async () => {
    const err = blobError('{"error":"no tickets in that range"}', 'application/json');
    expect(extractApiError(err, 'fallback')).toBe('fallback'); // unreadable while it is a Blob
    await unwrapBlobError(err);
    expect(extractApiError(err, 'fallback')).toBe('no tickets in that range');
  });

  it('leaves a non-JSON Blob alone', async () => {
    const err = blobError('%PDF-1.4', 'application/pdf');
    await unwrapBlobError(err);
    expect(err.response?.data).toBeInstanceOf(Blob);
  });

  it('leaves a Blob that claims to be JSON but is not', async () => {
    const err = blobError('<html>', 'application/json');
    await unwrapBlobError(err);
    expect(err.response?.data).toBeInstanceOf(Blob);
  });

  it('ignores errors that have no response', async () => {
    await expect(unwrapBlobError(new AxiosError('Network Error'))).resolves.toBeUndefined();
  });
});
