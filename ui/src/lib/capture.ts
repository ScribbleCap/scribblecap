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

export interface CaptureAvailability { wayland: boolean; previous: boolean }
export type CaptureMode = 'region' | 'window' | 'screen' | 'previous';
export interface CaptureSettings { captureFolder: string | null; autoSave: boolean; captureHotkey?: string; keepRunning?: boolean; hideDockIcon?: boolean }

export function captureState(): Promise<CaptureAvailability> {
  return invoke<CaptureAvailability>('shell_capture_state');
}
async function packet(command: string, args?: Record<string, unknown>): Promise<LoadedImage | null> {
  const response = await invoke<ArrayBuffer | number[]>(command, args);
  return decodeImagePacket(response instanceof ArrayBuffer ? response : new Uint8Array(response).buffer);
}
/** The image, or null when overlays opened (the image follows `capture-ready`) or the timer was cancelled. */
export function captureImage(mode: CaptureMode): Promise<LoadedImage | null> { return packet('shell_capture', { mode }); }
export function captureTimer(seconds: number): Promise<LoadedImage | null> { return packet('shell_capture_timer', { seconds }); }
export function captureResult(): Promise<LoadedImage | null> { return packet('shell_capture_result'); }

export function getSettings(): Promise<CaptureSettings> { return invoke<CaptureSettings>('shell_get_settings'); }
export function setAutoSave(enabled: boolean): Promise<CaptureSettings> { return invoke<CaptureSettings>('shell_set_auto_save', { enabled }); }
export function chooseCaptureFolder(): Promise<CaptureSettings> { return invoke<CaptureSettings>('shell_choose_capture_folder'); }
