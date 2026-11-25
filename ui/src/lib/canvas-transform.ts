import { pixelRequest } from './pixel-request';
import { invoke } from '@tauri-apps/api/core';
import { desktop } from './bridge';
import { decodeImagePacket, MAX_IMAGE_BYTES, type LoadedImage } from './image-packet';
export type CanvasOperation = 'crop' | 'resize' | 'expand' | 'clockwise' | 'counterclockwise' | 'horizontal' | 'vertical';
export interface CanvasTransform { operation: CanvasOperation; outputWidth: number; outputHeight: number; x: number; y: number }
export async function transformImage(image: LoadedImage, request: CanvasTransform): Promise<LoadedImage> {
  if (image.editable) throw new Error('Canvas transforms for editable annotations are not available yet.');
  if (desktop) {
    const result = await invoke<ArrayBuffer | number[]>('shell_transform_image', pixelRequest(image, { ...request, name: image.name }));
    const decoded = decodeImagePacket(result instanceof ArrayBuffer ? result : new Uint8Array(result).buffer);
    if (!decoded) throw new Error('The canvas transform returned no image.');
    return decoded;
  }
  // Browser development preview only. Desktop pixel transforms run in core.
  const { operation, x, y } = request;
  const turns = operation === 'clockwise' || operation === 'counterclockwise';
  const width = turns ? image.height : ['horizontal', 'vertical'].includes(operation) ? image.width : request.outputWidth;
  const height = turns ? image.width : ['horizontal', 'vertical'].includes(operation) ? image.height : request.outputHeight;
  if (![width, height, x, y].every(Number.isSafeInteger) || width <= 0 || height <= 0 || x < 0 || y < 0 || width * height * 4 > MAX_IMAGE_BYTES) throw new Error('Invalid canvas dimensions.');
  if (operation === 'crop' && (x + width > image.width || y + height > image.height)) throw new Error('Crop must fit inside the image.');
  if (operation === 'expand' && (x + image.width > width || y + image.height > height)) throw new Error('The expanded canvas must contain the image.');
  const source = document.createElement('canvas'); source.width = image.width; source.height = image.height;
  source.getContext('2d')!.putImageData(new ImageData(image.pixels, image.width, image.height), 0, 0);
  const output = document.createElement('canvas'); output.width = width; output.height = height;
  const context = output.getContext('2d');
  if (!context) throw new Error('The image canvas could not be created.');
  context.imageSmoothingEnabled = false;
  if (operation === 'crop') context.drawImage(source, -x, -y);
  else if (operation === 'expand') context.drawImage(source, x, y);
  else if (operation === 'resize') context.drawImage(source, 0, 0, width, height);
  else {
    if (operation === 'clockwise') { context.translate(width, 0); context.rotate(Math.PI / 2); }
    if (operation === 'counterclockwise') { context.translate(0, height); context.rotate(-Math.PI / 2); }
    if (operation === 'horizontal') { context.translate(width, 0); context.scale(-1, 1); }
    if (operation === 'vertical') { context.translate(0, height); context.scale(1, -1); }
    context.drawImage(source, 0, 0);
  }
  return { name: image.name, width, height, pixels: context.getImageData(0, 0, width, height).data };
}
