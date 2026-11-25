import { describe, expect, it } from 'vitest';
import { dragRect, MIN_DRAG, toDesktop, windowAt } from './overlay-geometry';

describe('overlay geometry', () => {
  it('normalizes drags in any direction', () => {
    expect(dragRect({ x: 50, y: 40 }, { x: 10, y: 20 })).toEqual({ x: 10, y: 20, width: 40, height: 20 });
    expect(dragRect({ x: 10, y: 40 }, { x: 50, y: 20 })).toEqual({ x: 10, y: 20, width: 40, height: 20 });
  });
  it('treats drags under 3 CSS pixels as clicks', () => { expect(MIN_DRAG).toBe(3); });
  it('picks the frontmost window under the pointer', () => {
    const w = [{ bounds: { x: 0, y: 0, width: 50, height: 50 } }, { bounds: { x: 0, y: 0, width: 100, height: 100 } }];
    expect(windowAt(w, { x: 10, y: 10 })).toBe(0);
    expect(windowAt(w, { x: 80, y: 80 })).toBe(1);
    expect(windowAt(w, { x: 200, y: 0 })).toBeNull();
  });
  it('offsets local rectangles by the display origin, including negative ones', () => {
    expect(toDesktop({ x: 5, y: 6, width: 7, height: 8 }, { x: -1440, y: 0 })).toEqual({ x: -1435, y: 6, width: 7, height: 8 });
  });
});
