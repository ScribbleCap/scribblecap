<script lang="ts">
  import type { Style } from './shapes';
  let { style, disabled, change }: { style: Style; disabled: boolean; change: (style: Style) => void } = $props();
  const colours = ['#e5194a', '#ff9500', '#ffcc00', '#34c759', '#007aff', '#af52de', '#ffffff', '#000000'];
</script>
<details class="style-controls">
  <summary aria-label={`Colour and size: ${style.color}, ${style.width} px`} title="Colour and size"><span class="swatch" style:background={style.color}></span><span class="size" style:height={`${Math.min(style.width, 10)}px`}></span></summary>
  <div class="popover">
    <fieldset disabled={disabled}><legend>Annotation colour</legend>
      <div class="swatches">{#each colours as color (color)}
        <button type="button" aria-label={`Colour ${color}`} aria-pressed={style.color === color} style:background={color} onclick={() => change({ ...style, color })}></button>
      {/each}</div>
      <label>Custom colour <input type="color" value={style.color} oninput={(event) => change({ ...style, color: event.currentTarget.value })} /></label>
    </fieldset>
    <label>Stroke size <select disabled={disabled} value={style.width} onchange={(event) => change({ ...style, width: Number(event.currentTarget.value) })}>
      {#each [2, 4, 6, 10, 16] as size (size)}<option value={size}>{size} px</option>{/each}
    </select></label>
  </div>
</details>
<style>
  details { position: relative; } summary { cursor: pointer; padding: .4rem; display: flex; align-items: center; gap: .35rem; list-style: none; } summary::-webkit-details-marker { display: none; }
  .swatch { width: 18px; height: 18px; border-radius: 50%; border: 1px solid var(--line); } .size { width: 16px; min-height: 2px; border-radius: 2px; background: var(--ink-2); }
  .popover { position: absolute; top: 100%; right: 0; z-index: 6; display: grid; gap: .6rem; min-width: 15rem; padding: .8rem; background: var(--surface); border: 1px solid var(--line); border-radius: .5rem; }
  fieldset { margin: 0; border: 1px solid var(--line); } label { display: flex; align-items: center; justify-content: space-between; gap: .5rem; }
  .swatches { display: flex; gap: .2rem; margin: .5rem 0; } .swatches button { width: 24px; min-height: 24px; padding: 0; border: 1px solid GrayText; }
  .swatches button[aria-pressed='true'] { outline: 2px solid var(--accent); outline-offset: 1px; }
</style>
