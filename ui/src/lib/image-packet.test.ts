import { describe, expect, it } from 'vitest';
import { decodeImagePacket } from './image-packet';

function packet(name = 'résumé.png') {
  const label = new TextEncoder().encode(name);
  const data = new ArrayBuffer(20 + label.length + 8);
  const bytes = new Uint8Array(data);
  bytes.set([83, 67, 73, 77]);
  const header = new DataView(data);
  [1, 2, 1, label.length].forEach((v, i) => header.setUint32(4 + i * 4, v, true));
  bytes.set(label, 20);
  bytes.set([255, 0, 0, 255, 0, 0, 255, 128], 20 + label.length);
  return data;
}

describe('native binary image packet', () => {
  it('reads Unicode names, dimensions and exact straight RGBA without a JSON pixel array', () => {
    const result = decodeImagePacket(packet());
    expect(result?.name).toBe('résumé.png');
    expect([result?.width, result?.height]).toEqual([2, 1]);
    expect(Array.from(result!.pixels)).toEqual([255, 0, 0, 255, 0, 0, 255, 128]);
  });
  it('treats an empty response as a cancelled dialog', () => {
    expect(decodeImagePacket(new ArrayBuffer(0))).toBeNull();
  });
  it('rejects invalid headers, impossible dimensions and truncated pixel data', () => {
    for (const [offset, value] of [[0, 0], [4, 99], [8, 0], [8, 0xffffffff], [16, 0xffffffff]]) {
      const data = packet();
      new DataView(data).setUint32(offset, value, true);
      expect(() => decodeImagePacket(data)).toThrow();
    }
    expect(() => decodeImagePacket(packet().slice(0, -1))).toThrow();
  });
});

it('decodes editable flags and rejects unknown flags in version 2', () => {
  const old = new Uint8Array(packet());
  const bytes = new Uint8Array(old.length + 4);
  bytes.set(old.subarray(0, 20)); bytes.set(old.subarray(20), 24);
  const header = new DataView(bytes.buffer);
  header.setUint32(4, 2, true); header.setUint32(20, 1, true);
  expect(decodeImagePacket(bytes.buffer)?.editable).toBe(true);
  header.setUint32(20, 2, true);
  expect(() => decodeImagePacket(bytes.buffer)).toThrow();
});
