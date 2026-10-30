import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { openImage, pasteImage } from './bridge';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
beforeEach(() => vi.mocked(invoke).mockReset());

describe('native image transport', () => {
  it('handles cancellation from both ArrayBuffer and WebKit byte-array responses', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(new ArrayBuffer(0)).mockResolvedValueOnce([]);
    expect(await openImage()).toBeNull();
    expect(await pasteImage()).toBeNull();
  });
  it('decodes the same pixel packet from both transports', async () => {
    const bytes = new Uint8Array(24);
    bytes.set([83, 67, 73, 77]);
    const header = new DataView(bytes.buffer);
    [1, 1, 1, 0].forEach((value, i) => header.setUint32(4 + i * 4, value, true));
    bytes.set([255, 0, 0, 255], 20);
    vi.mocked(invoke).mockResolvedValueOnce(bytes.buffer).mockResolvedValueOnce(Array.from(bytes));
    expect((await openImage())?.pixels).toEqual((await pasteImage())?.pixels);
  });
});
