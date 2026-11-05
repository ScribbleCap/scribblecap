// Editing operations on shape parameters (start/end points, rectangles). The
// painted geometry always comes from the core; these functions only decide which
// parameters a drag produces and whether a point is on a shape.
import type { Box, Pt, Shape } from './core';

export type Tool = 'select' | 'arrow' | 'line' | 'rectangle' | 'roundedRectangle' | 'ellipse';
export type Handle = 'start' | 'end' | 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

/** One key per tool; also listed in docs/editor.md. */
export const TOOL_KEYS: Record<string, Tool> = { v: 'select', a: 'arrow', l: 'line', r: 'rectangle', u: 'roundedRectangle', o: 'ellipse' };

export interface Style { color: string; width: number }
export const DEFAULT_STYLE: Style = { color: '#e5194a', width: 6 };
const ARROW_WIDTH_FACTOR = 2; // arrows read best a bit heavier than outlines
const CORNER_RADIUS_FACTOR = 2.5;

/** Shape a drag from `from` to `to` produces with a drawing tool. */
export function create(tool: Exclude<Tool, 'select'>, from: Pt, to: Pt, style: Style): Shape {
  switch (tool) {
    case 'arrow': return { kind: 'arrow', start: from, end: to, color: style.color, width: style.width * ARROW_WIDTH_FACTOR };
    case 'line': return { kind: 'line', start: from, end: to, color: style.color, width: style.width };
    case 'rectangle': return { kind: 'rectangle', rect: boxFrom(from, to), stroke: style.color, width: style.width };
    case 'roundedRectangle': return { kind: 'rectangle', rect: boxFrom(from, to), stroke: style.color, width: style.width, cornerRadius: style.width * CORNER_RADIUS_FACTOR };
    case 'ellipse': return { kind: 'ellipse', rect: boxFrom(from, to), stroke: style.color, width: style.width };
  }
}

/** A rectangle spanning two corners, in any drag direction. */
export function boxFrom(a: Pt, b: Pt): Box {
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1])];
}

export function translate(shape: Shape, dx: number, dy: number): Shape {
  if ('rect' in shape) { const [x, y, w, h] = shape.rect; return { ...shape, rect: [x + dx, y + dy, w, h] }; }
  return { ...shape, start: [shape.start[0] + dx, shape.start[1] + dy], end: [shape.end[0] + dx, shape.end[1] + dy] };
}

export function handles(shape: Shape): { handle: Handle; at: Pt }[] {
  if (!('rect' in shape)) return [{ handle: 'start', at: shape.start }, { handle: 'end', at: shape.end }];
  const [x, y, w, h] = shape.rect, cx = x + w / 2, cy = y + h / 2;
  return [
    { handle: 'nw', at: [x, y] }, { handle: 'n', at: [cx, y] }, { handle: 'ne', at: [x + w, y] }, { handle: 'e', at: [x + w, cy] },
    { handle: 'se', at: [x + w, y + h] }, { handle: 's', at: [cx, y + h] }, { handle: 'sw', at: [x, y + h] }, { handle: 'w', at: [x, cy] },
  ];
}

/** Move one handle to `to`. Boxes stay normalised when dragged past the far edge. */
export function dragHandle(shape: Shape, handle: Handle, to: Pt): Shape {
  if (!('rect' in shape)) return handle === 'start' ? { ...shape, start: to } : handle === 'end' ? { ...shape, end: to } : shape;
  const [x, y, w, h] = shape.rect;
  let x0 = x, y0 = y, x1 = x + w, y1 = y + h;
  if (handle.includes('w')) x0 = to[0];
  if (handle.includes('e')) x1 = to[0];
  if (handle === 'n' || handle === 'nw' || handle === 'ne') y0 = to[1];
  if (handle === 's' || handle === 'sw' || handle === 'se') y1 = to[1];
  return { ...shape, rect: boxFrom([x0, y0], [x1, y1]) };
}

/** Whether `p` is on the shape, within `tolerance` image pixels of its outline. */
export function hitTest(shape: Shape, p: Pt, tolerance: number): boolean {
  if (!('rect' in shape)) return distanceToSegment(p, shape.start, shape.end) <= shape.width / 2 + tolerance;
  const [x, y, w, h] = shape.rect, band = shape.width / 2 + tolerance;
  if (shape.kind === 'ellipse') {
    const rx = w / 2, ry = h / 2, cx = x + rx, cy = y + ry;
    const inside = (ax: number, ay: number) => ax > 0 && ay > 0 && ((p[0] - cx) / ax) ** 2 + ((p[1] - cy) / ay) ** 2 <= 1;
    const nearOutline = inside(rx + band, ry + band) && !inside(rx - band, ry - band);
    return nearOutline || (!!shape.fill && inside(rx, ry));
  }
  const within = (m: number) => p[0] >= x - m && p[0] <= x + w + m && p[1] >= y - m && p[1] <= y + h + m;
  const nearOutline = within(band) && !(w > 2 * band && h > 2 * band && within(-band));
  return nearOutline || (!!shape.fill && within(0));
}

/** Topmost element under `p` (elements are bottom-most first). */
export function topmostAt(elements: { id: string; shape: Shape }[], p: Pt, tolerance: number): string | null {
  for (let i = elements.length - 1; i >= 0; i--) if (hitTest(elements[i].shape, p, tolerance)) return elements[i].id;
  return null;
}

function distanceToSegment(p: Pt, a: Pt, b: Pt): number {
  const dx = b[0] - a[0], dy = b[1] - a[1], length = dx * dx + dy * dy;
  const t = length === 0 ? 0 : Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length));
  return Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy);
}
