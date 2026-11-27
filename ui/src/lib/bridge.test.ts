import { beforeEach, describe, expect, it, vi } from 'vitest';
import { pixelRequest } from './pixel-request';
import { invoke } from '@tauri-apps/api/core';
import { openImage, pasteImage, copyImage, saveImage, dragImage, shareImage, privacyImage, restorePrivacy } from './bridge';

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

it('passes annotation snapshots and save intent without any frontend filesystem path', async () => {
  const image = { name: 'test.png', width: 1, height: 1, pixels: new Uint8ClampedArray([1, 2, 3, 255]) };
  const document = '{"format":"scribblecap/1"}';
  vi.mocked(invoke).mockResolvedValue(null);
  await copyImage(image, document);
  expect(invoke).toHaveBeenLastCalledWith('shell_copy_image', pixelRequest(image, { document }));
  await saveImage(image, document, true, false);
  expect(invoke).toHaveBeenLastCalledWith('shell_save_image', pixelRequest(image, { name: 'test.png', document, saveAs: true, flat: false }));
  const anchor = { x: 40, y: 8, width: 56, height: 28 };
  await shareImage(image, document, anchor);
  expect(invoke).toHaveBeenLastCalledWith('shell_share_image', pixelRequest(image, { document, anchor }));
  await dragImage(image, document, 7);
  expect(invoke).toHaveBeenLastCalledWith('shell_drag_image', pixelRequest(image, { document, gesture: 7 }));
});

it('privacy sends only background pixels and restores versioned native snapshots', async () => {
  const image = { name: 'test.png', width: 1, height: 1, pixels: new Uint8ClampedArray([7,8,9,255]), privacyRevision: 3 };
  const reply = new Uint8Array(28), header = new DataView(reply.buffer);
  header.setUint32(0,4,true); reply.set([83,67,73,77],4);
  [1,1,1,0].forEach((value,i) => header.setUint32(8+i*4,value,true)); reply.set([0,0,0,255],24);
  vi.mocked(invoke).mockResolvedValueOnce(reply.buffer).mockResolvedValueOnce(Array.from(reply));
  expect((await privacyImage(image,[0,0,1,1],'redact')).privacyRevision).toBe(4);
  expect(invoke).toHaveBeenLastCalledWith('shell_privacy_image',pixelRequest(image,{name:'test.png',rect:[0,0,1,1],operation:'redact',currentRevision:3}));
  expect((await restorePrivacy(4)).pixels).toEqual(new Uint8ClampedArray([0,0,0,255]));
  expect(invoke).toHaveBeenLastCalledWith('shell_restore_privacy',{revision:4});
});
