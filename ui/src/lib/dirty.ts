/**
 * Tracks whether the open document differs from what was last loaded or saved.
 * `position` follows the undo history (a step forward, back or onward), so undoing
 * back to the saved state is clean again; `changed` covers edits outside the undo
 * history (a canvas transform), which stay dirty until the next save or load.
 */
export function createDirtyTracker() {
  let position = 0, savedAt: number | null = 0, forced = false;
  return {
    get dirty() { return forced || savedAt !== position; },
    /** A new undo step was recorded. Redo steps are gone, so a save ahead of here is unreachable. */
    step() { if (savedAt !== null && savedAt > position) savedAt = null; position++; },
    undo() { position--; },
    redo() { position++; },
    /** An edit outside the undo history. */
    changed() { forced = true; },
    saved() { savedAt = position; forced = false; },
    /** A document was loaded: it starts clean with an empty history. */
    reset() { position = 0; savedAt = 0; forced = false; },
  };
}
