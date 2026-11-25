<script lang="ts">
  import type { LoadedImage } from './image-packet';
  let { image, disabled, apply }: { image: LoadedImage | null; disabled: boolean; apply: (rect: [number, number, number, number], operation: 'pixelate' | 'redact') => void } = $props();
  let operation = $state<'pixelate' | 'redact'>('redact');
  let x = $state(0), y = $state(0), width = $state(1), height = $state(1);
</script>
<details class="privacy-controls"><summary>Privacy</summary>
  <form onsubmit={(event) => { event.preventDefault(); apply([x, y, width, height], operation); }}>
    <p>Apply replaces background pixels while keeping annotations editable. Undo and redo remain available for this session; saved files contain only the current background. Existing files are unchanged.</p>
    <label>Privacy operation <select bind:value={operation}><option value="redact">Redact (solid black)</option><option value="pixelate">Pixelate (12 px blocks)</option></select></label>
    <p>Pixelation obscures detail; use solid redaction for sensitive information.</p>
    <label>Region left <input type="number" min="0" max={image ? image.width - 1 : 0} step="1" required bind:value={x} /></label>
    <label>Region top <input type="number" min="0" max={image ? image.height - 1 : 0} step="1" required bind:value={y} /></label>
    <label>Region width <input type="number" min="1" max={image ? image.width - x : 1} step="1" required bind:value={width} /></label>
    <label>Region height <input type="number" min="1" max={image ? image.height - y : 1} step="1" required bind:value={height} /></label>
    <button disabled={disabled || !image} type="submit">Apply privacy edit</button>
  </form>
</details>
<style>
  details { position: relative; } summary { cursor: pointer; padding: .5rem; }
  form { position: absolute; right: 0; top: 100%; z-index: 6; width: min(22rem, 90vw); max-height: 75vh; overflow: auto; padding: .8rem; display: grid; gap: .6rem; background: var(--surface); border: 1px solid var(--line); border-radius: .5rem; }
  p { margin: 0; font-size: .85rem; } label { display: flex; justify-content: space-between; gap: .5rem; } input { width: 5rem; }
</style>
