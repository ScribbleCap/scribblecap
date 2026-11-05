// The tests read the built core WebAssembly module from disk. Declares only the
// Node API they use, instead of depending on @types/node.
declare module 'node:fs' {
  export function readFileSync(path: string): Uint8Array<ArrayBuffer>;
}
