<script lang="ts">
  import type { LoadedImage } from './image-packet';
  import { transformImage, type CanvasOperation } from './canvas-transform';
  let { image, busy, load }: { image: LoadedImage | null; busy: boolean; load: (action: () => Promise<LoadedImage | null>) => Promise<void> } = $props();
  let operation = $state<CanvasOperation>('crop');
  let width = $state(1), height = $state(1), x = $state(0), y = $state(0);
  $effect(() => { if (image) { width = image.width; height = image.height; x = 0; y = 0; } });
  const sized = $derived(['crop', 'resize', 'expand'].includes(operation));
  function apply() {
    const current = image;
    if (current && !current.editable) void load(() => transformImage(current, { operation, outputWidth: width, outputHeight: height, x, y }));
  }
</script>
<details class="canvas-controls">
  <summary>Canvas</summary>
  <form onsubmit={(event) => { event.preventDefault(); apply(); }}>
    <label>Operation <select bind:value={operation} disabled={!image || busy || image.editable}>
      <option value="crop">Crop</option><option value="resize">Resize image</option><option value="expand">Expand canvas</option>
      <option value="clockwise">Rotate clockwise</option><option value="counterclockwise">Rotate counterclockwise</option>
      <option value="horizontal">Flip horizontally</option><option value="vertical">Flip vertically</option>
    </select></label>
    {#if sized}
      <label>Width <input type="number" min="1" step="1" required bind:value={width} /></label>
      <label>Height <input type="number" min="1" step="1" required bind:value={height} /></label>
      {#if operation !== 'resize'}
        <label>{operation === 'crop' ? 'Crop left' : 'Image left'} <input type="number" min="0" step="1" required bind:value={x} /></label>
        <label>{operation === 'crop' ? 'Crop top' : 'Image top'} <input type="number" min="0" step="1" required bind:value={y} /></label>
      {/if}
    {/if}
    {#if image?.editable}<p>Canvas transforms for editable annotations are not available yet.</p>{/if}
    <button type="submit" disabled={!image || busy || image.editable}>Apply</button>
  </form>
</details>
<style>
  .canvas-controls { position: relative; }
  summary { cursor: pointer; padding: .5rem; }
  form { position: absolute; z-index: 5; top: 100%; right: 0; display: grid; gap: .6rem; padding: 1rem; min-width: 16rem; background: var(--surface, Canvas); color: CanvasText; border: 1px solid GrayText; border-radius: .5rem; box-shadow: 0 4px 16px #0003; }
  label { display: flex; align-items: center; justify-content: space-between; gap: .7rem; }
  input { width: 6rem; }
  input, select { font: inherit; padding: .3rem; }
</style>
