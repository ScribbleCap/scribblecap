export interface Size { width: number; height: number }
export interface Point { x: number; y: number }
export interface Viewport extends Point { scale: number }
export const MIN_ZOOM = 0.000001;
export const MAX_ZOOM = 16;

export function fitViewport(image: Size, area: Size): Viewport {
  const scale = Math.min(1, Math.max(1, area.width - 40) / image.width, Math.max(1, area.height - 40) / image.height);
  return { scale, x: (area.width - image.width * scale) / 2, y: (area.height - image.height * scale) / 2 };
}
export function actualSize(image: Size, area: Size): Viewport {
  return { scale: 1, x: (area.width - image.width) / 2, y: (area.height - image.height) / 2 };
}
export function zoomAt(view: Viewport, scale: number, point: Point): Viewport {
  if (!Number.isFinite(scale) || scale <= 0) return view;
  scale = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, scale));
  const ratio = scale / view.scale;
  return { scale, x: point.x - (point.x - view.x) * ratio, y: point.y - (point.y - view.y) * ratio };
}
export function panBy(view: Viewport, dx: number, dy: number): Viewport {
  return Number.isFinite(dx) && Number.isFinite(dy) ? { ...view, x: view.x + dx, y: view.y + dy } : view;
}
