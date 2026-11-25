import { invoke } from '@tauri-apps/api/core';
import { decodeImagePacket, type LoadedImage } from './image-packet';
import type { Rect } from './overlay-geometry';

export interface WindowOutline { id: number; title: string; appName: string; bounds: Rect }
export interface OverlayInfo { bounds: Rect; windows: WindowOutline[]; mode: 'region' | 'window' | 'screen' | 'previous' }

export function overlayInfo(display: number): Promise<OverlayInfo> {
  return invoke<OverlayInfo>('shell_overlay_info', { display });
}
export async function overlayFrame(display: number): Promise<LoadedImage> {
  const response = await invoke<ArrayBuffer | number[]>('shell_overlay_frame', { display });
  const image = decodeImagePacket(response instanceof ArrayBuffer ? response : new Uint8Array(response).buffer);
  if (!image) throw new Error('The frozen screen is not available.');
  return image;
}
export async function overlaySelect(display: number, rect: Rect): Promise<void> {
  await invoke('shell_overlay_select', { display, rect });
}
/** Never throws: cancelling is the way out of every failure. */
export async function cancelCapture(): Promise<void> {
  try { await invoke('shell_capture_cancel'); } catch { /* the window is closing anyway */ }
}
