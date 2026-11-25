import { MAX_IMAGE_BYTES, type LoadedImage } from './image-packet';

/** SCOP v1: magic, version, JSON byte length (u32 LE), UTF-8 metadata, raw RGBA8. */
export function pixelRequest(image: LoadedImage, fields: Record<string, unknown>): Uint8Array<ArrayBuffer> {
  const length = image.width * image.height * 4;
  if (!Number.isInteger(image.width) || !Number.isInteger(image.height) || image.width <= 0 || image.height <= 0 || length > MAX_IMAGE_BYTES || image.pixels.byteLength !== length) {
    throw new Error('The image request has invalid pixels.');
  }
  const metadata = new TextEncoder().encode(JSON.stringify({ ...fields, width: image.width, height: image.height }));
  if (metadata.byteLength > 16 * 1024 * 1024) throw new Error('The image request metadata is too large.');
  const bytes = new Uint8Array(12 + metadata.byteLength + length);
  bytes.set([83, 67, 79, 80]);
  const header = new DataView(bytes.buffer);
  header.setUint32(4, 1, true);
  header.setUint32(8, metadata.byteLength, true);
  bytes.set(metadata, 12);
  bytes.set(image.pixels, 12 + metadata.byteLength);
  return bytes;
}
