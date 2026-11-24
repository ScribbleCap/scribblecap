import { beforeEach, describe, expect, it, vi } from 'vitest';
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));
vi.mock('./bridge', () => ({ desktop: true }));
import { transformImage } from './canvas-transform';
const image = { name: 'test.png', width: 1, height: 1, pixels: new Uint8ClampedArray([1, 2, 3, 4]) };
beforeEach(() => invoke.mockReset());
describe('canvas native boundary', () => {
  it('passes explicit pixels and bounds; handles WebKit byte-array responses', async () => {
    const packet = new Uint8Array(25); packet.set([83,67,73,77]);
    const header = new DataView(packet.buffer);
    [1,1,1,1].forEach((value, i) => header.setUint32(4 + i * 4, value, true));
    packet.set([120,4,3,2,1],20);
    invoke.mockResolvedValue(Array.from(packet));
    const result = await transformImage(image, { operation: 'crop', outputWidth: 1, outputHeight: 1, x: 0, y: 0 });
    expect(result.pixels).toEqual(new Uint8ClampedArray([4,3,2,1]));
    expect(invoke).toHaveBeenCalledWith('shell_transform_image', { name: 'test.png', width: 1, height: 1, pixels: [1,2,3,4], operation: 'crop', outputWidth: 1, outputHeight: 1, x: 0, y: 0 });
  });
  it('blocks editable images before invoking a flattening operation', async () => {
    let blocked = false;
    try { await transformImage({ ...image, editable: true }, { operation: 'horizontal', outputWidth: 1, outputHeight: 1, x: 0, y: 0 }); } catch { blocked = true; }
    expect(blocked).toBe(true);
    expect(invoke).not.toHaveBeenCalled();
  });
});
