<script lang="ts">
  import { onMount } from 'svelte';
  import { cancelCapture, overlayFrame, overlayInfo, overlaySelect, type OverlayInfo } from '../lib/capture';
  import { dragRect, isSelection, toDesktop, windowAt, type Pt, type Rect } from '../lib/overlay-geometry';

  let { display }: { display: number } = $props();

  let info = $state<OverlayInfo | null>(null);
  let imageWidth = $state(0);
  let canvas = $state<HTMLCanvasElement>();
  let pointer = $state<Pt | null>(null);
  let anchor = $state<Pt | null>(null);
  const ENDED = 'This capture has ended.';
  let done = false;

  const origin = $derived(info ? { x: info.bounds.x, y: info.bounds.y } : { x: 0, y: 0 });
  const scale = $derived(info && info.bounds.width > 0 ? imageWidth / info.bounds.width : 1);
  const windowMode = $derived(info?.mode === 'window');
  const hovered = $derived(windowMode && pointer && info ? windowAt(info.windows, { x: pointer.x + origin.x, y: pointer.y + origin.y }) : null);
  const dragging = $derived(anchor && pointer ? dragRect(anchor, pointer) : null);
  const highlight = $derived<Rect | null>(
    windowMode
      ? hovered !== null && info ? { ...info.windows[hovered].bounds, x: info.windows[hovered].bounds.x - origin.x, y: info.windows[hovered].bounds.y - origin.y } : null
      : dragging && isSelection(dragging) ? dragging : null,
  );
  const sizeLabel = $derived(highlight ? `${Math.round(highlight.width * scale)} × ${Math.round(highlight.height * scale)}` : '');

  async function cancel() {
    if (done) return;
    done = true;
    await cancelCapture();
  }
  async function select(local: Rect) {
    if (done) return;
    done = true;
    try {
      await overlaySelect(display, toDesktop(local, origin));
    } catch (e) {
      // The capture already ended (cancelled elsewhere): Rust has closed the windows, nothing to undo.
      if (String(e instanceof Error ? e.message : e).includes(ENDED)) return;
      done = false;
      await cancel();
    }
  }

  onMount(() => {
    // Both commands count as contact for the Rust watchdog; any failure ends the capture.
    Promise.all([overlayInfo(display), overlayFrame(display)]).then(([i, frame]) => {
      imageWidth = frame.width;
      info = i;
      const ctx = canvas?.getContext?.('2d');
      if (ctx) {
        canvas!.width = frame.width;
        canvas!.height = frame.height;
        ctx.putImageData(new ImageData(frame.pixels, frame.width, frame.height), 0, 0);
      }
    }, () => cancel());
  });

  const local = (e: MouseEvent): Pt => ({ x: e.clientX, y: e.clientY });

  function down(e: PointerEvent) {
    if (e.button !== 0 || done) return;
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
    pointer = local(e);
    anchor = windowMode ? null : pointer;
  }
  function move(e: PointerEvent) { pointer = local(e); }
  function up(e: PointerEvent) {
    if (e.button !== 0 || done) return;
    const end = local(e);
    pointer = end;
    if (windowMode) {
      if (hovered !== null && info) {
        const b = info.windows[hovered].bounds;
        void select({ x: b.x - origin.x, y: b.y - origin.y, width: b.width, height: b.height });
      }
      return;
    }
    const start = anchor;
    anchor = null;
    if (!start) return;
    const r = dragRect(start, end);
    if (!isSelection(r)) return; // a click: stay open
    void select(r);
  }
  function context(e: MouseEvent) { e.preventDefault(); void cancel(); }
  function key(e: KeyboardEvent) { if (e.key === 'Escape') void cancel(); }
</script>

<svelte:window onkeydown={key} />

<canvas class="frame" bind:this={canvas} aria-hidden="true"></canvas>
{#if info}
  <div class="surface" data-testid="overlay" role="application" aria-label="Choose the area to capture. Press Escape to cancel."
    onpointerdown={down} onpointermove={move} onpointerup={up} oncontextmenu={context}>
    <svg width="100%" height="100%" aria-hidden="true">
      <defs>
        <mask id="dim">
          <rect width="100%" height="100%" fill="white" />
          {#if highlight}<rect x={highlight.x} y={highlight.y} width={highlight.width} height={highlight.height} fill="black" />{/if}
        </mask>
      </defs>
      <rect width="100%" height="100%" class="dim" mask="url(#dim)" />
      {#if highlight}
        <rect x={highlight.x} y={highlight.y} width={highlight.width} height={highlight.height} class="outline" />
      {/if}
      {#if pointer && !windowMode}
        <line x1={pointer.x} y1="0" x2={pointer.x} y2="100%" class="hair" />
        <line x1="0" y1={pointer.y} x2="100%" y2={pointer.y} class="hair" />
      {/if}
    </svg>
    {#if highlight && sizeLabel}
      <span class="size" style:left="{highlight.x}px" style:top="{Math.max(highlight.y - 28, 4)}px">{sizeLabel}</span>
    {/if}
  </div>
{/if}

<style>
  .frame, .surface { position: fixed; inset: 0; width: 100vw; height: 100vh; }
  .surface { cursor: crosshair; user-select: none; }
  .dim { fill: rgb(0 0 0 / 0.38); }
  .outline { fill: none; stroke: var(--accent, #0a64d8); stroke-width: 2; }
  .hair { stroke: white; stroke-width: 1; stroke-opacity: 0.85; mix-blend-mode: difference; }
  .size {
    position: fixed; padding: 2px 8px; border-radius: 6px; pointer-events: none;
    font: 13px/20px system-ui, -apple-system, 'Segoe UI Variable', Cantarell, Ubuntu, 'Noto Sans', sans-serif;
    background: var(--chrome, #2a2a2d); color: var(--ink, #ececee); font-variant-numeric: tabular-nums;
  }
</style>
