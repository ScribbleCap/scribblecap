// Reproducible frontend preparation benchmark; excludes native IPC/render/encode.
// Run from the repo root: node ui/scripts/benchmark-pixel-request.mjs
import { readFile } from 'node:fs/promises';
import { Buffer } from 'node:buffer';
import console from 'node:console';
import process from 'node:process';
import { URL } from 'node:url';
import { stripTypeScriptTypes } from 'node:module';
import { performance } from 'node:perf_hooks';

// Load the production encoder, replacing only its constants/types import so Node
// can execute the TypeScript module without a frontend bundler.
const source = await readFile(new URL('../src/lib/pixel-request.ts', import.meta.url), 'utf8');
const javascript = stripTypeScriptTypes(source.replace(/^import .*;$/m, 'const MAX_IMAGE_BYTES = 256 * 1024 * 1024;'));
const { pixelRequest } = await import(`data:text/javascript;base64,${Buffer.from(javascript).toString('base64')}`);
const image = { name: '4k.png', width: 3840, height: 2160, pixels: new Uint8ClampedArray(3840 * 2160 * 4) };
for (let i = 0; i < image.pixels.length; i++) image.pixels[i] = i % 256;
const document = '{"format":"scribblecap/1"}';
function measure(name, encode) {
  const results = [];
  let bytes = 0;
  for (let i = 0; i < 4; i++) {
    const start = performance.now();
    const result = encode();
    const duration = performance.now() - start;
    bytes = typeof result === 'string' ? Buffer.byteLength(result) : result.byteLength;
    if (i) results.push(duration); // First pass warms the encoder.
  }
  results.sort((a, b) => a - b);
  return { name, bytes, medianMilliseconds: Number(results[1].toFixed(2)) };
}
console.log(JSON.stringify({ node: process.version, platform: `${process.platform}/${process.arch}`, width: image.width, height: image.height, measurements: [
  measure('JSON Array.from + stringify', () => JSON.stringify({ width: image.width, height: image.height, pixels: Array.from(image.pixels), document })),
  measure('SCOP raw packet', () => pixelRequest(image, { document })),
] }, null, 2));
