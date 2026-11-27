import { pixelRequest } from './pixel-request';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { decodeImagePacket, MAX_IMAGE_BYTES, type LoadedImage } from './image-packet';

export async function openImage(): Promise<LoadedImage | null> {
  return nativeImage('shell_open_image');
}
export async function pasteImage(): Promise<LoadedImage | null> {
  return nativeImage('shell_paste_image');
}
async function nativeImage(command: string): Promise<LoadedImage | null> {
  // WebKit's IPC fallback returns bytes as an array; the fetch transport uses
  // ArrayBuffer. Keep one packet format and accept both documented transports.
  const response = await invoke<ArrayBuffer | number[]>(command);
  return decodeImagePacket(response instanceof ArrayBuffer ? response : new Uint8Array(response).buffer);
}
export const desktop = isTauri();
// Tauri's desktop WebViews identify macOS/Windows here. Rust also rejects other OSes.
export const systemShareSupported = desktop && /Mac|Win/.test(navigator.platform);
export async function shareImage(image: LoadedImage, document: string, anchor: { x: number; y: number; width: number; height: number }): Promise<void> {
  await invoke('shell_share_image', pixelRequest(image, { document, anchor }));
}

/** Browser-only development preview; desktop files always use Rust codecs. */
export async function browserImage(file: Blob, name: string): Promise<LoadedImage> {
  const bitmap = await createImageBitmap(file);
  try {
    if (bitmap.width * bitmap.height * 4 > MAX_IMAGE_BYTES) throw new Error('This image is too large to open.');
    const canvas = document.createElement('canvas');
    canvas.width = bitmap.width; canvas.height = bitmap.height;
    const context = canvas.getContext('2d');
    if (!context) throw new Error('The image canvas could not be created.');
    context.drawImage(bitmap, 0, 0);
    return { name, width: bitmap.width, height: bitmap.height, pixels: context.getImageData(0, 0, bitmap.width, bitmap.height).data };
  } finally { bitmap.close(); }
}

/** The native event authorizes the path; JavaScript cannot supply one. */
export async function droppedImage(): Promise<LoadedImage | null> {
  return nativeImage('shell_drop_image');
}

export async function copyImage(image: LoadedImage, document: string): Promise<void> {
  await invoke('shell_copy_image', pixelRequest(image, { document }));
}
export async function saveImage(image: LoadedImage, document: string, saveAs = false, flat = false): Promise<string | null> {
  return invoke('shell_save_image', pixelRequest(image, { name: image.name, document, saveAs, flat }));
}
export async function dragImage(image: LoadedImage, document: string, gesture: number): Promise<void> {
  await invoke('shell_drag_image', pixelRequest(image, { document, gesture }));
}
export async function cancelDrag(gesture: number): Promise<void> { await invoke('shell_cancel_drag', { gesture }); }

export async function beginDrag(): Promise<number> { return invoke('shell_begin_drag'); }

function privacyReply(response: ArrayBuffer | number[]): LoadedImage {
  const bytes = response instanceof ArrayBuffer ? response : new Uint8Array(response).buffer;
  if (bytes.byteLength < 4) throw new Error('The background response is invalid.');
  const privacyRevision = new DataView(bytes).getUint32(0, true);
  const result = decodeImagePacket(bytes.slice(4));
  if (!result) throw new Error('The privacy operation returned no image.');
  return { ...result, privacyRevision };
}
export async function privacyImage(image: LoadedImage, rect: [number, number, number, number], operation: 'pixelate' | 'redact'): Promise<LoadedImage> {
  return privacyReply(await invoke<ArrayBuffer | number[]>('shell_privacy_image', pixelRequest(image, { name: image.name, rect, operation, currentRevision: image.privacyRevision ?? 0 })));
}
export async function restorePrivacy(revision: number): Promise<LoadedImage> {
  return privacyReply(await invoke<ArrayBuffer | number[]>('shell_restore_privacy', { revision }));
}
