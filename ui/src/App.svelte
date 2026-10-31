<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import appIcon from '../../app/icons/128x128.png';
  import * as bridge from './lib/bridge';
  import type { LoadedImage } from './lib/image-packet';
  import { fitViewport, actualSize, zoomAt, panBy, MAX_ZOOM, MIN_ZOOM, type Viewport, type Size } from './lib/viewport';

  let image = $state.raw<LoadedImage | null>(null);
  let source = $state.raw<HTMLCanvasElement | null>(null);
  let view = $state<Viewport>({ scale: 1, x: 0, y: 0 });
  let area = $state<Size>({ width: 800, height: 600 });
  let stage: HTMLDivElement;
  let canvas = $state<HTMLCanvasElement>();
  let fileInput: HTMLInputElement;
  let busy = $state(false);
  let error = $state('');
  let dragging = $state(false);
  let pointer: { id: number; x: number; y: number } | null = null;
  let alive = true;
  let ratio = $state(1);
  const zoomLabel = $derived(view.scale * 100 < 1 ? `${(view.scale * 100).toPrecision(2)}%` : `${Math.round(view.scale * 100)}%`);

  async function load(action: () => Promise<LoadedImage | null>) {
    if (busy) return;
    busy = true; error = '';
    try {
      const next = await action();
      if (!next || !alive) return;
      const nextSource = document.createElement('canvas');
      nextSource.width = next.width; nextSource.height = next.height;
      const context = nextSource.getContext('2d');
      if (!context) throw new Error('The image canvas could not be created.');
      context.putImageData(new ImageData(next.pixels, next.width, next.height), 0, 0);
      source = nextSource; image = next;
      view = fitViewport(next, area);
      document.title = `${next.name} — ScribbleCap`;
    } catch (cause) {
      if (alive) error = cause instanceof Error ? cause.message : String(cause);
    } finally { if (alive) busy = false; }
  }
  function openImage() {
    if (bridge.desktop) void load(bridge.openImage);
    else fileInput.click();
  }
  async function clipboardImage() {
    if (bridge.desktop) return bridge.pasteImage();
    if (!navigator.clipboard?.read) throw new Error('Use your browser’s Paste command after copying an image.');
    for (const item of await navigator.clipboard.read()) {
      const type = item.types.find(type => type.startsWith('image/'));
      if (type) return bridge.browserImage(await item.getType(type), 'Clipboard image');
    }
    throw new Error('There is no image on the clipboard. Copy an image and try again.');
  }
  function pasteImage() { void load(clipboardImage); }
  function onPaste(event: ClipboardEvent) {
    if (editableTarget(event.target)) return;
    event.preventDefault();
    if (bridge.desktop) pasteImage();
    else {
      const file = Array.from(event.clipboardData?.files ?? []).find(file => file.type.startsWith('image/'));
      if (file) void load(() => bridge.browserImage(file, 'Clipboard image'));
      else error = 'There is no image on the clipboard. Copy an image and try again.';
    }
  }
  function fit() { if (image) view = fitViewport(image, area); }
  function actual() { if (image) view = actualSize(image, area); }
  function zoom(factor: number) { if (image) view = zoomAt(view, view.scale * factor, { x: area.width / 2, y: area.height / 2 }); }
  function editableTarget(target: EventTarget | null) {
    return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
  }
  function onKey(event: KeyboardEvent) {
    if (editableTarget(event.target) || event.altKey) return;
    if (event.metaKey || event.ctrlKey) {
      const key = event.key.toLowerCase();
      if (['o', 'v', '+', '=', '-', '0', '1'].includes(key)) event.preventDefault();
      if (key === 'o' && !event.repeat) openImage();
      if (key === 'v' && !event.repeat) pasteImage();
      if (key === '+' || key === '=') zoom(1.25);
      if (key === '-') zoom(0.8);
      if (key === '0') fit();
      if (key === '1') actual();
    } else if (image) {
      const delta: Record<string, [number, number]> = { ArrowLeft: [32, 0], ArrowRight: [-32, 0], ArrowUp: [0, 32], ArrowDown: [0, -32] };
      if (delta[event.key]) { event.preventDefault(); view = panBy(view, ...delta[event.key]); }
    }
  }
  function onWheel(event: WheelEvent) {
    if (!image) return;
    event.preventDefault();
    const delta = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? area.height : 1;
    if (event.ctrlKey || event.metaKey) {
      const bounds = stage.getBoundingClientRect();
      view = zoomAt(view, view.scale * Math.exp(-event.deltaY * delta * 0.01), { x: event.clientX - bounds.left, y: event.clientY - bounds.top });
    } else view = panBy(view, -event.deltaX * delta, -event.deltaY * delta);
  }
  function down(event: PointerEvent) {
    if (!image || event.button !== 0 || !event.isPrimary) return;
    stage.focus(); event.preventDefault(); stage.setPointerCapture(event.pointerId);
    pointer = { id: event.pointerId, x: event.clientX, y: event.clientY }; dragging = true;
  }
  function move(event: PointerEvent) {
    if (!pointer || pointer.id !== event.pointerId) return;
    view = panBy(view, event.clientX - pointer.x, event.clientY - pointer.y);
    pointer = { id: event.pointerId, x: event.clientX, y: event.clientY };
  }
  function up(event: PointerEvent) {
    if (pointer?.id !== event.pointerId) return;
    pointer = null; dragging = false;
    if (stage.hasPointerCapture(event.pointerId)) stage.releasePointerCapture(event.pointerId);
  }
  onMount(() => {
    ratio = window.devicePixelRatio || 1;
    const observer = new ResizeObserver(entries => {
      const bounds = entries[0].contentRect;
      if (!bounds.width || !bounds.height) return;
      view = panBy(view, (bounds.width - area.width) / 2, (bounds.height - area.height) / 2);
      area = { width: bounds.width, height: bounds.height }; ratio = window.devicePixelRatio || 1;
    });
    observer.observe(stage);
    return () => observer.disconnect();
  });
  onDestroy(() => { alive = false; });
  $effect(() => {
    if (!canvas || !source) return;
    const width = Math.max(1, Math.round(area.width * ratio)), height = Math.max(1, Math.round(area.height * ratio));
    if (canvas.width !== width) canvas.width = width;
    if (canvas.height !== height) canvas.height = height;
    const context = canvas.getContext('2d');
    if (!context) return;
    context.setTransform(1, 0, 0, 1, 0, 0); context.clearRect(0, 0, canvas.width, canvas.height);
    context.setTransform(ratio * view.scale, 0, 0, ratio * view.scale, view.x * ratio, view.y * ratio);
    context.imageSmoothingEnabled = view.scale < 1;
    context.fillStyle = '#ffffff';
    const tile = document.createElement('canvas'); tile.width = 16; tile.height = 16;
    const tileContext = tile.getContext('2d');
    if (tileContext) {
      tileContext.fillStyle = '#fff'; tileContext.fillRect(0, 0, 16, 16);
      tileContext.fillStyle = '#e4e5e8'; tileContext.fillRect(0, 0, 8, 8); tileContext.fillRect(8, 8, 8, 8);
      context.fillStyle = context.createPattern(tile, 'repeat') ?? '#fff';
    }
    context.fillRect(0, 0, source.width, source.height); context.drawImage(source, 0, 0);
  });
</script>

<svelte:window onkeydown={onKey} onpaste={onPaste} />
<main aria-busy={busy}>
  <header class="toolbar">
    <div class="brand" aria-label="ScribbleCap"><img class="brand-mark" src={appIcon} alt="" width="28" height="28" /><strong>ScribbleCap</strong></div>
    <div class="input-actions">
      <button class="primary" onclick={openImage} disabled={busy} title="Open image (Ctrl/⌘ O)"><span aria-hidden="true">＋</span> Open image</button>
      <button onclick={pasteImage} disabled={busy} title="Paste image (Ctrl/⌘ V)">Paste image</button>
    </div>
    <div class="zoom-actions" aria-label="Zoom controls">
      <button onclick={fit} disabled={!image} title="Fit image (Ctrl/⌘ 0)">Fit</button><span class="divider"></span>
      <button class="square" onclick={() => zoom(0.8)} disabled={!image || view.scale <= MIN_ZOOM} aria-label="Zoom out">−</button>
      <button class="zoom-value" onclick={actual} disabled={!image} title="Actual size (Ctrl/⌘ 1)">{image ? zoomLabel : '100%'}</button>
      <button class="square" onclick={() => zoom(1.25)} disabled={!image || view.scale >= MAX_ZOOM} aria-label="Zoom in">＋</button>
    </div>
  </header>
  {#if error}<div class="error" role="alert"><span>{error}</span><button onclick={() => error = ''} aria-label="Dismiss error">×</button></div>{/if}
  <div class="workspace" class:has-image={!!image}>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (The viewport is focusable for keyboard panning; toolbar buttons provide zoom.) -->
    <div class="viewport" class:dragging bind:this={stage} role="region" aria-label="Image viewport" aria-describedby="viewport-help" tabindex="0" onwheel={onWheel} onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} onlostpointercapture={up}>
      {#if image}<canvas bind:this={canvas} aria-label={`${image.name}, ${image.width} by ${image.height} pixels`}></canvas>
      {:else}
        <div class="empty-state">
          <div class="empty-art" aria-hidden="true"><svg width="104" height="92" viewBox="0 0 104 92"><path d="M20 32V13h20M64 13h20v19M84 60v19H64M40 79H20V60" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"/><path d="M35 59 67 32M52 33l17-3-4 18" fill="none" stroke="#e13b66" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"/></svg></div>
          <h1>A picture is a good place to start.</h1>
          <p>Open an image or paste a screenshot<br />to bring it onto your canvas.</p>
          <button class="primary large" onclick={openImage} disabled={busy}>Open an image</button>
          <span class="formats">PNG · JPEG · WebP · TIFF · BMP · GIF</span>
        </div>
      {/if}
    </div>
    {#if busy}<div class="loading" role="status">Loading image…</div>{/if}
  </div>
  <footer>
    <span class="file-info" title={image?.name}>{image ? image.name : 'Your canvas, ready when you are'}</span>
    {#if image}<span class="dimensions">{image.width.toLocaleString()} × {image.height.toLocaleString()} px</span>{/if}
    <span id="viewport-help">{image ? 'Drag or use arrow keys to pan · Ctrl/⌘ + scroll to zoom' : 'Ctrl/⌘ O to open · Ctrl/⌘ V to paste'}</span>
  </footer>
  <input class="file-input" bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/webp,image/tiff,image/bmp,image/gif" aria-label="Choose an image" onchange={() => { const file = fileInput.files?.[0]; if (file) void load(() => bridge.browserImage(file, file.name)); fileInput.value = ''; }} />
</main>
