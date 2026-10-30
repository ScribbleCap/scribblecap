export interface LoadedImage {
  name: string;
  width: number;
  height: number;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

export const MAX_IMAGE_BYTES = 256 * 1024 * 1024;

/** SCIM, version/width/height/name-length (u32 LE), UTF-8 basename, straight RGBA8. */
export function decodeImagePacket(buffer: ArrayBuffer): LoadedImage | null {
  if (buffer.byteLength === 0) return null;
  const invalid = () => new Error('The image could not be loaded. Its pixel data is invalid.');
  if (buffer.byteLength < 20) throw invalid();
  const bytes = new Uint8Array(buffer);
  if (bytes[0] !== 83 || bytes[1] !== 67 || bytes[2] !== 73 || bytes[3] !== 77) throw invalid();
  const header = new DataView(buffer);
  if (header.getUint32(4, true) !== 1) throw invalid();
  const width = header.getUint32(8, true), height = header.getUint32(12, true);
  const nameLength = header.getUint32(16, true);
  const length = width * height * 4;
  if (!width || !height || !Number.isSafeInteger(length) || length > MAX_IMAGE_BYTES || nameLength > 65536 || buffer.byteLength !== 20 + nameLength + length) throw invalid();
  const name = new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(20, 20 + nameLength));
  return { name, width, height, pixels: new Uint8ClampedArray(buffer, 20 + nameLength, length) };
}
