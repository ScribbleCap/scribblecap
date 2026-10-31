import { describe, expect, it } from 'vitest';
import { fitViewport, actualSize, zoomAt, panBy, MIN_ZOOM, MAX_ZOOM } from './viewport';

describe('viewport transforms', () => {
  it('fits a large image with padding and centers it without enlarging small images', () => {
    expect(fitViewport({ width: 2000, height: 1000 }, { width: 1040, height: 540 })).toEqual({ scale: 0.5, x: 20, y: 20 });
    expect(fitViewport({ width: 100, height: 100 }, { width: 1040, height: 540 })).toEqual({ scale: 1, x: 470, y: 220 });
  });
  it('keeps the image point under the zoom anchor fixed', () => {
    const before = { scale: 0.5, x: -20, y: 10 };
    const anchor = { x: 130, y: 80 };
    const after = zoomAt(before, 2, anchor);
    expect((anchor.x - after.x) / after.scale).toBe((anchor.x - before.x) / before.scale);
    expect((anchor.y - after.y) / after.scale).toBe((anchor.y - before.y) / before.scale);
  });
  it('clamps zoom without changing the anchor and rejects nonfinite changes', () => {
    const view = { scale: 1, x: 0, y: 0 };
    expect(zoomAt(view, 1e20, { x: 0, y: 0 }).scale).toBe(MAX_ZOOM);
    expect(zoomAt(view, 1e-20, { x: 0, y: 0 }).scale).toBe(MIN_ZOOM);
    expect(zoomAt(view, NaN, { x: 0, y: 0 })).toEqual(view);
    expect(panBy(view, Infinity, 0)).toEqual(view);
  });
  it('actual size centers at one image pixel per CSS pixel and panning preserves scale', () => {
    const view = actualSize({ width: 200, height: 100 }, { width: 100, height: 100 });
    expect(view).toEqual({ scale: 1, x: -50, y: 0 });
    expect(panBy(view, 20, -10)).toEqual({ scale: 1, x: -30, y: -10 });
  });
});
