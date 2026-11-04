import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { createCore, CoreError } from './core';

// Runs the real core WebAssembly module (built by `pnpm test`).
// Vitest runs from ui/.
const bytes = readFileSync('src/generated/core.wasm');

describe('core WebAssembly module', () => {
  it('adds, updates and removes shapes and returns paintable paths', async () => {
    const core = await createCore(bytes);
    core.reset(200, 100);
    const arrow = core.add({ kind: 'arrow', start: [10, 10], end: [120, 80], color: '#e5194a', width: 12 });
    expect(arrow.id).toBe('el-1');
    expect(arrow.paths[0].fill).toBeTruthy();
    expect(arrow.paths[0].d).toMatch(/^M /);
    const rect = core.add({ kind: 'rectangle', rect: [5, 5, 50, 30], stroke: '#e5194a', width: 4, cornerRadius: 6 });
    expect(rect.paths[0]).toMatchObject({ width: 4 });
    const moved = core.update(arrow.id, { kind: 'arrow', start: [20, 20], end: [130, 90], color: '#e5194a', width: 12 });
    expect(moved[0].d).not.toBe(arrow.paths[0].d);
    core.remove(rect.id);
    const saved = JSON.parse(core.document());
    expect(saved.format).toBe('scribblecap/1');
    expect(saved.elements).toHaveLength(1);
  });
  it('rejects invalid edits without changing the document', async () => {
    const core = await createCore(bytes);
    core.reset(200, 100);
    const { id } = core.add({ kind: 'line', start: [0, 0], end: [10, 10], color: '#e5194a', width: 4 });
    expect(() => core.update(id, { kind: 'line', start: [0, 0], end: [10, 10], color: '#e5194a', width: -1 })).toThrow(CoreError);
    expect(() => core.remove('missing')).toThrow(/no element missing/);
    expect(JSON.parse(core.document()).elements[0].width).toBe(4);
  });
  it('handles large requests and responses across memory growth', async () => {
    const core = await createCore(bytes);
    core.reset(10000, 10000);
    for (let i = 0; i < 300; i++) core.add({ kind: 'ellipse', rect: [i, i, 500, 300], stroke: '#e5194a', width: 4 });
    expect(JSON.parse(core.document()).elements).toHaveLength(300);
  });
});
