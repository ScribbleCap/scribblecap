export interface Pt { x: number; y: number }
export interface Rect { x: number; y: number; width: number; height: number }

/** Drags shorter than this many CSS pixels (in both axes) are clicks, not selections. */
export const MIN_DRAG = 3;

/** The normalized rectangle between two corners, whichever way the drag went. */
export function dragRect(a: Pt, b: Pt): Rect {
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) };
}

/** Index of the frontmost window containing `p`; `windows` is ordered front to back. */
export function windowAt(windows: { bounds: Rect }[], p: Pt): number | null {
  const i = windows.findIndex(({ bounds: b }) => p.x >= b.x && p.x < b.x + b.width && p.y >= b.y && p.y < b.y + b.height);
  return i < 0 ? null : i;
}

/** Overlay-local CSS pixels (desktop points) to desktop points. */
export function toDesktop(local: Rect, displayOrigin: Pt): Rect {
  return { ...local, x: local.x + displayOrigin.x, y: local.y + displayOrigin.y };
}
