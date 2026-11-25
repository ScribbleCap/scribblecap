import { describe, expect, it } from 'vitest';
import { pixelRequest } from './pixel-request';

describe('raw output packet', () => {
  const image = { name: '雪.png', width: 1, height: 1, pixels: new Uint8ClampedArray([0, 128, 255, 4]) };
  it('frames UTF-8 metadata separately from exact RGBA bytes, including subviews', () => {
    const storage = new Uint8ClampedArray([99, 0, 128, 255, 4, 99]);
    const bytes = pixelRequest({ ...image, pixels: storage.subarray(1, 5) }, { name: image.name, document: '✓', width: 100 });
    const header = new DataView(bytes.buffer);
    expect([...bytes.subarray(0, 4)]).toEqual([83, 67, 79, 80]);
    expect(header.getUint32(4, true)).toBe(1);
    const offset = 12 + header.getUint32(8, true);
    expect(JSON.parse(new TextDecoder().decode(bytes.subarray(12, offset)))).toEqual({ name: '雪.png', document: '✓', width: 1, height: 1 });
    expect([...bytes.subarray(offset)]).toEqual([0, 128, 255, 4]);
  });
  it('rejects invalid bounds, missing pixels and oversized metadata', () => {
    for (const change of [{ width: 0 }, { height: -1 }, { width: 0.5 }, { width: 1e9 }, { pixels: new Uint8ClampedArray(3) }]) {
      expect(() => pixelRequest({ ...image, ...change }, {})).toThrow();
    }
    expect(() => pixelRequest(image, { document: 'x'.repeat(16 * 1024 * 1024) })).toThrow('metadata');
  });
});
