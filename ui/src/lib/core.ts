// The core library running as WebAssembly (decision 0011). The document lives
// inside the module; the UI sends JSON requests (protocol: docs/editor.md) and
// paints the paths it gets back. No geometry is computed here.

export type Pt = [number, number];
export type Box = [number, number, number, number];

export type Shape =
  | { kind: 'arrow'; start: Pt; end: Pt; color: string; width: number; headAtStart?: boolean }
  | { kind: 'line'; start: Pt; end: Pt; color: string; width: number }
  | { kind: 'rectangle'; rect: Box; stroke: string; fill?: string; width: number; cornerRadius?: number }
  | { kind: 'ellipse'; rect: Box; stroke: string; fill?: string; width: number };

/** A path in image pixels: SVG path data plus how to paint it. */
export interface WirePath { d: string; fill?: string; stroke?: string; width?: number; round?: boolean }

export interface Core {
  reset(width: number, height: number): void;
  add(shape: Shape): { id: string; paths: WirePath[] };
  update(id: string, shape: Shape): WirePath[];
  remove(id: string): void;
  document(): string;
}

interface Exports {
  memory: WebAssembly.Memory;
  sc_alloc(len: number): number;
  sc_free(ptr: number, len: number): void;
  sc_call(ptr: number, len: number): bigint;
}

export class CoreError extends Error {}

/** Instantiate the core from the module's bytes. */
export async function createCore(bytes: BufferSource): Promise<Core> {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const wasm = instance.exports as unknown as Exports;
  const encoder = new TextEncoder(), decoder = new TextDecoder();

  function call(request: object): unknown {
    const input = encoder.encode(JSON.stringify(request));
    const ptr = wasm.sc_alloc(input.length);
    new Uint8Array(wasm.memory.buffer, ptr, input.length).set(input);
    const packed = wasm.sc_call(ptr, input.length);
    wasm.sc_free(ptr, input.length);
    const out = Number(packed >> 32n), len = Number(packed & 0xffffffffn);
    // Read before freeing; memory.buffer may have been replaced if memory grew.
    const text = decoder.decode(new Uint8Array(wasm.memory.buffer, out, len));
    wasm.sc_free(out, len);
    const response = JSON.parse(text) as { ok?: unknown; error?: string };
    if (response.error !== undefined) throw new CoreError(response.error);
    return response.ok;
  }

  return {
    reset: (width, height) => { call({ op: 'reset', canvas: [width, height] }); },
    add: shape => call({ op: 'add', shape }) as { id: string; paths: WirePath[] },
    update: (id, shape) => (call({ op: 'update', id, shape }) as { paths: WirePath[] }).paths,
    remove: id => { call({ op: 'remove', id }); },
    document: () => call({ op: 'document' }) as string,
  };
}

let shared: Promise<Core> | null = null;

/** The app's core instance, loaded once from the bundled module. */
export function loadCore(): Promise<Core> {
  shared ??= import('../generated/core.wasm?url')
    .then(module => fetch(module.default))
    .then(response => response.arrayBuffer())
    .then(createCore);
  return shared;
}
