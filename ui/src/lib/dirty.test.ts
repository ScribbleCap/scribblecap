import { expect, it } from 'vitest';
import { createDirtyTracker } from './dirty';

it('is clean after load and dirty after a recorded step', () => {
  const t = createDirtyTracker();
  expect(t.dirty).toBe(false);
  t.step(); expect(t.dirty).toBe(true);
});
it('is clean again after undo back to the saved state, dirty past it', () => {
  const t = createDirtyTracker();
  t.step(); t.saved(); expect(t.dirty).toBe(false);
  t.undo(); expect(t.dirty).toBe(true);
  t.redo(); expect(t.dirty).toBe(false);
});
it('a new edit after undoing past the save can never match the saved state', () => {
  const t = createDirtyTracker();
  t.step(); t.saved(); t.undo(); t.step();
  expect(t.dirty).toBe(true);
  t.undo(); t.undo(); expect(t.dirty).toBe(true);
});
it('a changed document is dirty until loaded or saved', () => {
  const t = createDirtyTracker();
  t.changed(); expect(t.dirty).toBe(true);
  t.saved(); expect(t.dirty).toBe(false);
  t.changed(); t.reset(); expect(t.dirty).toBe(false);
});
