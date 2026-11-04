import { describe, expect, it } from 'vitest';
import type { Shape } from './core';
import { boxFrom, create, dragHandle, handles, hitTest, topmostAt, translate, DEFAULT_STYLE } from './shapes';

describe('shape editing', () => {
  it('creates every tool shape from a drag in any direction', () => {
    expect(boxFrom([50, 40], [10, 10])).toEqual([10, 10, 40, 30]);
    expect(create('arrow', [0, 0], [10, 5], DEFAULT_STYLE)).toMatchObject({ kind: 'arrow', start: [0, 0], end: [10, 5] });
    expect(create('line', [0, 0], [10, 5], DEFAULT_STYLE)).toMatchObject({ kind: 'line', width: DEFAULT_STYLE.width });
    expect(create('rectangle', [20, 20], [0, 0], DEFAULT_STYLE)).toMatchObject({ kind: 'rectangle', rect: [0, 0, 20, 20] });
    expect(create('roundedRectangle', [0, 0], [9, 9], DEFAULT_STYLE)).toMatchObject({ kind: 'rectangle', cornerRadius: 15 });
    expect(create('ellipse', [0, 0], [9, 9], DEFAULT_STYLE)).toMatchObject({ kind: 'ellipse', rect: [0, 0, 9, 9] });
  });
  it('moves shapes without changing their size', () => {
    expect(translate(create('line', [0, 0], [10, 0], DEFAULT_STYLE), 5, 2)).toMatchObject({ start: [5, 2], end: [15, 2] });
    expect(translate(create('ellipse', [0, 0], [10, 8], DEFAULT_STYLE), -1, 1)).toMatchObject({ rect: [-1, 1, 10, 8] });
  });
  it('resizes with endpoint and box handles, flipping past the far edge', () => {
    const line = create('arrow', [0, 0], [10, 0], DEFAULT_STYLE);
    expect(handles(line).map(h => h.handle)).toEqual(['start', 'end']);
    expect(dragHandle(line, 'end', [20, 5])).toMatchObject({ start: [0, 0], end: [20, 5] });
    const box = create('rectangle', [0, 0], [10, 10], DEFAULT_STYLE);
    expect(handles(box)).toHaveLength(8);
    expect(dragHandle(box, 'se', [30, 20])).toMatchObject({ rect: [0, 0, 30, 20] });
    expect(dragHandle(box, 'e', [-5, 99])).toMatchObject({ rect: [-5, 0, 5, 10] });
    expect(dragHandle(box, 'n', [99, 4])).toMatchObject({ rect: [0, 4, 10, 6] });
  });
  it('hits outlines within tolerance, but not the empty middle of unfilled shapes', () => {
    const line = create('line', [0, 0], [100, 0], DEFAULT_STYLE);
    expect(hitTest(line, [50, 6], 3)).toBe(true);
    expect(hitTest(line, [50, 7], 3)).toBe(false);
    const box = create('rectangle', [0, 0], [100, 100], DEFAULT_STYLE);
    expect(hitTest(box, [0, 50], 2)).toBe(true);
    expect(hitTest(box, [50, 50], 2)).toBe(false);
    expect(hitTest({ ...box, fill: '#ffffff' } as Shape, [50, 50], 2)).toBe(true);
    const ellipse = create('ellipse', [0, 0], [100, 50], DEFAULT_STYLE);
    expect(hitTest(ellipse, [50, 1], 2)).toBe(true);
    expect(hitTest(ellipse, [50, 25], 2)).toBe(false);
    expect(hitTest(ellipse, [1, 1], 2)).toBe(false);
  });
  it('finds the topmost element under a point', () => {
    const elements = [
      { id: 'a', shape: create('rectangle', [0, 0], [100, 100], DEFAULT_STYLE) },
      { id: 'b', shape: create('line', [0, 50], [100, 50], DEFAULT_STYLE) },
    ];
    expect(topmostAt(elements, [0, 50], 2)).toBe('b');
    expect(topmostAt(elements, [0, 20], 2)).toBe('a');
    expect(topmostAt(elements, [50, 20], 2)).toBeNull();
  });
});
