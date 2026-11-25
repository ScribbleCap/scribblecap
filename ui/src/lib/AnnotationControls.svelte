<script lang="ts">
  import type { Shape, Pt } from './core';
  import type { Tool } from './shapes';
  let { items, selected, disabled, tool, add, select, resize }: { items: { id: string; shape: Shape }[]; selected: string | null; disabled: boolean; tool: Tool; add: (from: Pt, to: Pt) => void; select: (id: string) => void; resize: (width: number, height: number) => void } = $props();
  let x = $state(0), y = $state(0), endX = $state(100), endY = $state(60);
  let width = $state(100), height = $state(60);
</script>
<details class="annotation-controls"><summary>Annotations ({items.length})</summary>
  <div class="popover">
    <p id="annotation-help">Select an annotation, then use arrow keys to move it (Shift for 10 pixels), Delete to remove it, and Escape to deselect. Use the dimensions below to resize. Draw without a pointer using coordinates.</p>
    <ul aria-label="Annotations">{#each items as item, index (item.id)}<li>
      <button disabled={disabled} aria-pressed={selected === item.id} onclick={() => select(item.id)}>{index + 1}. {item.shape.kind === 'freehand' ? item.shape.tool : item.shape.kind}{item.shape.kind === 'step' ? ` ${item.shape.number}` : ''}</button>
    </li>{/each}</ul>
    <form onsubmit={(event) => { event.preventDefault(); add([x, y], [endX, endY]); }}>
      <fieldset disabled={disabled || tool === 'select'}><legend>Add with {tool}</legend>
        <label>Start X <input type="number" required bind:value={x} /></label><label>Start Y <input type="number" required bind:value={y} /></label>
        <label>End X <input type="number" required bind:value={endX} /></label><label>End Y <input type="number" required bind:value={endY} /></label>
        <button type="submit">Add annotation</button>
      </fieldset>
    </form>
    <form onsubmit={(event) => { event.preventDefault(); resize(width, height); }}>
      <fieldset disabled={disabled || !selected}><legend>Resize selection</legend>
        <label>Selection width <input type="number" min="1" required bind:value={width} /></label>
        <label>Selection height <input type="number" min="1" required bind:value={height} /></label>
        <button type="submit">Resize annotation</button>
      </fieldset>
    </form>
  </div>
</details>
<style>
  details { position: relative; } summary { cursor: pointer; padding: .5rem; }
  .popover { position: absolute; z-index: 6; top: 100%; right: 0; width: min(23rem,90vw); max-height: 75vh; overflow: auto; padding: .8rem; background: var(--surface); border: 1px solid var(--line); border-radius: .5rem; }
  p { margin: 0 0 .5rem; font-size: .85rem; } ul { list-style: none; padding: 0; max-height: 10rem; overflow: auto; } li button { width: 100%; text-align: left; }
  fieldset { display: grid; gap: .5rem; border: 1px solid var(--line); margin: .5rem 0; } label { display: flex; justify-content: space-between; gap: .5rem; } input { width: 6rem; }
</style>
