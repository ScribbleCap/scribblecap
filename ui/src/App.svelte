<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as bridge from './lib/bridge';
  import { loadCore, type Core, type Pt, type Shape, type WirePath } from './lib/core';
  import type { LoadedImage } from './lib/image-packet';
  import { create, dragHandle, handles, topmostAt, translate, DEFAULT_STYLE, TOOL_KEYS, type Handle, type Tool } from './lib/shapes';
  import { fitViewport, actualSize, zoomAt, panBy, MAX_ZOOM, MIN_ZOOM, type Viewport, type Size } from './lib/viewport';

  interface Item { id: string; shape: Shape; paths: WirePath[] }
  type Gesture =
    | { kind: 'pan'; id: number; x: number; y: number }
    | { kind: 'create'; id: number; element: string; tool: Exclude<Tool, 'select'>; from: Pt; sx: number; sy: number; moved: boolean }
    | { kind: 'move'; id: number; element: string; from: Pt; original: Shape }
    | { kind: 'resize'; id: number; element: string; handle: Handle; original: Shape };

  const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  const mod = mac ? '⌘' : 'Ctrl+';
  const TOOLS: { tool: Tool; label: string; key: string; icon: string }[] = [
    { tool: 'select', label: 'Select', key: 'V', icon: 'M5 3l13 8-6 1.5L9 19z' },
    { tool: 'arrow', label: 'Arrow', key: 'A', icon: 'M4 20L18 6M10 5h9v9' },
    { tool: 'line', label: 'Line', key: 'L', icon: 'M4 20L20 4' },
    { tool: 'rectangle', label: 'Rectangle', key: 'R', icon: 'M4 6h16v12H4z' },
    { tool: 'roundedRectangle', label: 'Rounded rectangle', key: 'U', icon: 'M8 6h8a4 4 0 014 4v4a4 4 0 01-4 4H8a4 4 0 01-4-4v-4a4 4 0 014-4z' },
    { tool: 'ellipse', label: 'Ellipse', key: 'O', icon: 'M12 6c4.4 0 8 2.7 8 6s-3.6 6-8 6-8-2.7-8-6 3.6-6 8-6z' },
  ];
  /** Screen pixels: how close the pointer must be to a shape or handle. */
  const HIT = 6, HANDLE = 8, CLICK = 3;

  let image = $state.raw<LoadedImage | null>(null);
  let source = $state.raw<HTMLCanvasElement | null>(null);
  let view = $state<Viewport>({ scale: 1, x: 0, y: 0 });
  let area = $state<Size>({ width: 800, height: 600 });
  let stage: HTMLDivElement;
  let canvas = $state<HTMLCanvasElement>();
  let fileInput: HTMLInputElement;
  let busy = $state(false);
  let error = $state('');
  let alive = true;
  let ratio = $state(1);
  // While fitted, resizing the window re-fits the image (like native image viewers);
  // any manual zoom or pan ends fitting until Fit is chosen again.
  let fitted = $state(false);
  let theme = $state(0);
  let tile: HTMLCanvasElement | null = null;
  let tileColor = '';
  let core: Core | null = null;
  let tool = $state<Tool>('arrow');
  let items = $state.raw<Item[]>([]);
  let selected = $state<string | null>(null);
  let gesture = $state.raw<Gesture | null>(null);
  let spaceHeld = $state(false);
  let hovering = $state(false);
  const paths2d = new WeakMap<WirePath, Path2D>();
  const zoomLabel = $derived(view.scale * 100 < 1 ? `${(view.scale * 100).toPrecision(2)}%` : `${Math.round(view.scale * 100)}%`);
  const cursor = $derived(
    gesture?.kind === 'pan' ? 'grabbing'
      : spaceHeld ? 'grab'
      : gesture?.kind === 'move' || (tool === 'select' && hovering) ? 'move'
      : tool === 'select' ? 'default' : 'crosshair');

  async function load(action: () => Promise<LoadedImage | null>) {
    if (busy) return;
    busy = true; error = '';
    try {
      const next = await action();
      if (!next || !alive) return;
      core ??= await loadCore();
      const nextSource = document.createElement('canvas');
      nextSource.width = next.width; nextSource.height = next.height;
      const context = nextSource.getContext('2d');
      if (!context) throw new Error('The image canvas could not be created.');
      context.putImageData(new ImageData(next.pixels, next.width, next.height), 0, 0);
      core.reset(next.width, next.height);
      items = []; selected = null; gesture = null;
      source = nextSource; image = next;
      view = fitViewport(next, area); fitted = true;
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
  function fit() { if (image) { view = fitViewport(image, area); fitted = true; } }
  function actual() { if (image) { view = actualSize(image, area); fitted = false; } }
  function zoom(factor: number) { if (image) { view = zoomAt(view, view.scale * factor, { x: area.width / 2, y: area.height / 2 }); fitted = false; } }
  function editableTarget(target: EventTarget | null) {
    return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
  }

  // ----- Editing -----
  function report(cause: unknown) { error = cause instanceof Error ? cause.message : String(cause); }
  /** Send a changed shape to the core and repaint with the paths it returns. */
  function apply(id: string, shape: Shape): boolean {
    if (!core) return false;
    try {
      const paths = core.update(id, shape);
      items = items.map(item => item.id === id ? { id, shape, paths } : item);
      return true;
    } catch (cause) { report(cause); return false; }
  }
  function removeSelected() {
    if (!core || !selected) return;
    try { core.remove(selected); items = items.filter(item => item.id !== selected); selected = null; }
    catch (cause) { report(cause); }
  }
  function nudge(dx: number, dy: number) {
    const item = items.find(item => item.id === selected);
    if (item) apply(item.id, translate(item.shape, dx, dy));
  }
  function chooseTool(next: Tool) { tool = next; }

  function toImage(event: PointerEvent): Pt {
    const bounds = stage.getBoundingClientRect();
    return [(event.clientX - bounds.left - view.x) / view.scale, (event.clientY - bounds.top - view.y) / view.scale];
  }
  function handleAt(event: PointerEvent): Handle | null {
    const item = items.find(item => item.id === selected);
    if (!item) return null;
    const bounds = stage.getBoundingClientRect();
    const sx = event.clientX - bounds.left, sy = event.clientY - bounds.top;
    for (const { handle, at } of handles(item.shape)) {
      if (Math.abs(at[0] * view.scale + view.x - sx) <= HANDLE && Math.abs(at[1] * view.scale + view.y - sy) <= HANDLE) return handle;
    }
    return null;
  }

  function onKey(event: KeyboardEvent) {
    if (editableTarget(event.target) || event.altKey) return;
    const key = event.key.toLowerCase();
    if (event.metaKey || event.ctrlKey) {
      if (['o', 'v', '+', '=', '-', '0', '1'].includes(key)) event.preventDefault();
      if (key === 'o' && !event.repeat) openImage();
      if (key === 'v' && !event.repeat) pasteImage();
      if (key === '+' || key === '=') zoom(1.25);
      if (key === '-') zoom(0.8);
      if (key === '0') fit();
      if (key === '1') actual();
      return;
    }
    if (!image) return;
    if (event.key === ' ') { event.preventDefault(); spaceHeld = true; return; }
    if (event.key === 'Escape') { selected = null; return; }
    if ((event.key === 'Delete' || event.key === 'Backspace') && selected) { event.preventDefault(); removeSelected(); return; }
    if (!event.shiftKey && TOOL_KEYS[key]) { event.preventDefault(); chooseTool(TOOL_KEYS[key]); return; }
    const delta: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const d = delta[event.key];
    if (!d) return;
    event.preventDefault();
    if (selected) { const step = event.shiftKey ? 10 : 1; nudge(d[0] * step, d[1] * step); }
    else { view = panBy(view, -d[0] * 32, -d[1] * 32); fitted = false; }
  }
  function onKeyUp(event: KeyboardEvent) { if (event.key === ' ') spaceHeld = false; }
  function onWheel(event: WheelEvent) {
    if (!image) return;
    event.preventDefault();
    const delta = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? area.height : 1;
    if (event.ctrlKey || event.metaKey) {
      const bounds = stage.getBoundingClientRect();
      view = zoomAt(view, view.scale * Math.exp(-event.deltaY * delta * 0.01), { x: event.clientX - bounds.left, y: event.clientY - bounds.top });
    } else view = panBy(view, -event.deltaX * delta, -event.deltaY * delta);
    fitted = false;
  }
  function down(event: PointerEvent) {
    if (!image || !core || !event.isPrimary || (event.button !== 0 && event.button !== 1)) return;
    stage.focus(); event.preventDefault(); stage.setPointerCapture(event.pointerId);
    const id = event.pointerId;
    if (event.button === 1 || spaceHeld) { gesture = { kind: 'pan', id, x: event.clientX, y: event.clientY }; return; }
    const p = toImage(event);
    const handle = handleAt(event);
    if (handle && selected) {
      gesture = { kind: 'resize', id, element: selected, handle, original: items.find(item => item.id === selected)!.shape };
      return;
    }
    if (tool === 'select') {
      const hit = topmostAt(items, p, HIT / view.scale);
      selected = hit;
      gesture = hit
        ? { kind: 'move', id, element: hit, from: p, original: items.find(item => item.id === hit)!.shape }
        : { kind: 'pan', id, x: event.clientX, y: event.clientY };
      return;
    }
    try {
      const shape = create(tool, p, p, DEFAULT_STYLE);
      const added = core.add(shape);
      items = [...items, { id: added.id, shape, paths: added.paths }];
      selected = added.id;
      gesture = { kind: 'create', id, element: added.id, tool, from: p, sx: event.clientX, sy: event.clientY, moved: false };
    } catch (cause) { report(cause); }
  }
  function move(event: PointerEvent) {
    if (!gesture) { if (tool === 'select' && image) hovering = topmostAt(items, toImage(event), HIT / view.scale) !== null; return; }
    if (gesture.id !== event.pointerId) return;
    const g = gesture;
    if (g.kind === 'pan') {
      if (event.clientX !== g.x || event.clientY !== g.y) fitted = false;
      view = panBy(view, event.clientX - g.x, event.clientY - g.y);
      gesture = { ...g, x: event.clientX, y: event.clientY };
      return;
    }
    const p = toImage(event);
    if (g.kind === 'create') {
      if (!g.moved && Math.hypot(event.clientX - g.sx, event.clientY - g.sy) < CLICK) return;
      gesture = { ...g, moved: true };
      apply(g.element, create(g.tool, g.from, p, DEFAULT_STYLE));
    } else if (g.kind === 'move') apply(g.element, translate(g.original, p[0] - g.from[0], p[1] - g.from[1]));
    else apply(g.element, dragHandle(g.original, g.handle, p));
  }
  function up(event: PointerEvent) {
    if (gesture?.id !== event.pointerId) return;
    const g = gesture;
    gesture = null;
    if (stage.hasPointerCapture(event.pointerId)) stage.releasePointerCapture(event.pointerId);
    // A click without a drag draws nothing.
    if (g.kind === 'create' && !g.moved) removeSelected();
  }

  onMount(() => {
    ratio = window.devicePixelRatio || 1;
    const observer = new ResizeObserver(entries => {
      const bounds = entries[0].contentRect;
      if (!bounds.width || !bounds.height) return;
      const next = { width: bounds.width, height: bounds.height };
      view = fitted && image ? fitViewport(image, next) : panBy(view, (next.width - area.width) / 2, (next.height - area.height) / 2);
      area = next; ratio = window.devicePixelRatio || 1;
    });
    observer.observe(stage);
    // Repaint the canvas when the system switches between light and dark.
    const scheme = window.matchMedia?.('(prefers-color-scheme: dark)');
    const onScheme = () => { theme++; };
    scheme?.addEventListener?.('change', onScheme);
    return () => { observer.disconnect(); scheme?.removeEventListener?.('change', onScheme); };
  });
  onDestroy(() => { alive = false; });

  function paint(context: CanvasRenderingContext2D, path: WirePath) {
    if (typeof Path2D === 'undefined') return;
    let shape = paths2d.get(path);
    if (!shape) { shape = new Path2D(path.d); paths2d.set(path, shape); }
    if (path.fill) { context.fillStyle = path.fill; context.fill(shape, 'nonzero'); return; }
    context.strokeStyle = path.stroke ?? '#000'; context.lineWidth = path.width ?? 1;
    context.lineCap = path.round ? 'round' : 'butt'; context.lineJoin = path.round ? 'round' : 'miter'; context.miterLimit = 4;
    context.stroke(shape);
  }

  $effect(() => {
    if (!canvas || !source) return;
    const width = Math.max(1, Math.round(area.width * ratio)), height = Math.max(1, Math.round(area.height * ratio));
    if (canvas.width !== width) canvas.width = width;
    if (canvas.height !== height) canvas.height = height;
    const context = canvas.getContext('2d');
    if (!context) return;
    context.setTransform(1, 0, 0, 1, 0, 0); context.clearRect(0, 0, canvas.width, canvas.height);
    context.setTransform(ratio * view.scale, 0, 0, ratio * view.scale, view.x * ratio, view.y * ratio);
    void theme;
    const styles = getComputedStyle(stage);
    const token = (name: string, fallback: string) => styles.getPropertyValue(name).trim() || fallback;
    const checker = token('--checker', '#e4e5e8');
    if (!tile || tileColor !== checker) {
      // Built once per theme rather than on every redraw.
      tile = document.createElement('canvas'); tile.width = 16; tile.height = 16; tileColor = checker;
      const tileContext = tile.getContext('2d');
      if (tileContext) {
        tileContext.fillStyle = '#fff'; tileContext.fillRect(0, 0, 16, 16);
        tileContext.fillStyle = checker; tileContext.fillRect(0, 0, 8, 8); tileContext.fillRect(8, 8, 8, 8);
      }
    }
    // Shadow under the image, then the image, then a hairline edge so dark images
    // stay distinct from a dark canvas. Shadow and edge are in screen pixels.
    context.shadowColor = token('--image-shadow', 'rgba(0,0,0,.22)');
    context.shadowBlur = 14 * ratio; context.shadowOffsetY = 2 * ratio;
    context.fillStyle = context.createPattern(tile, 'repeat') ?? '#fff';
    context.fillRect(0, 0, source.width, source.height);
    context.shadowColor = 'transparent'; context.shadowBlur = 0; context.shadowOffsetY = 0;
    context.imageSmoothingEnabled = view.scale < 1;
    context.drawImage(source, 0, 0);
    // Annotations, clipped to the image like the flat export.
    context.save();
    context.beginPath(); context.rect(0, 0, source.width, source.height); context.clip();
    for (const item of items) for (const path of item.paths) paint(context, path);
    context.restore();
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.strokeStyle = token('--image-edge', 'rgba(0,0,0,.18)'); context.lineWidth = 1 / ratio;
    const half = 0.5 / ratio;
    context.strokeRect(view.x - half, view.y - half, source.width * view.scale + 2 * half, source.height * view.scale + 2 * half);
    // Selection handles in screen pixels, in the system accent colour.
    const chosen = items.find(item => item.id === selected);
    if (chosen && gesture?.kind !== 'create') {
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.lineWidth = 1.5; context.strokeStyle = token('--accent', '#0a64d8'); context.fillStyle = '#fff';
      for (const { at } of handles(chosen.shape)) {
        const x = at[0] * view.scale + view.x, y = at[1] * view.scale + view.y;
        context.fillRect(x - 4, y - 4, 8, 8); context.strokeRect(x - 4, y - 4, 8, 8);
      }
    }
  });
</script>

<svelte:window onkeydown={onKey} onkeyup={onKeyUp} onpaste={onPaste} onblur={() => spaceHeld = false} />
<main aria-busy={busy} class:mac={mac && bridge.desktop}>
  <header class="titlebar" data-tauri-drag-region>
    <!-- Capture (M4-5) takes this place; until then Open and Paste sit here. -->
    <div class="input-actions">
      <button class="quiet" onclick={openImage} disabled={busy} title={`Open image (${mod}O)`}>Open image</button>
      <button class="quiet" onclick={pasteImage} disabled={busy} title={`Paste image (${mod}V)`}>Paste image</button>
    </div>
    <div class="tools" role="toolbar" aria-label="Tools">
      {#each TOOLS as t (t.tool)}
        <button class="tool" aria-label={t.label} aria-pressed={tool === t.tool} title={`${t.label} (${t.key})`} disabled={!image} onclick={() => chooseTool(t.tool)}>
          <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d={t.icon} fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
      {/each}
    </div>
  </header>
  {#if error}<div class="error" role="alert"><span>{error}</span><button onclick={() => error = ''} aria-label="Dismiss error">×</button></div>{/if}
  <div class="workspace">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (The viewport is focusable for keyboard editing and panning; toolbar buttons provide zoom.) -->
    <div class="viewport" bind:this={stage} role="region" aria-label="Image viewport" aria-describedby="viewport-help" tabindex="0" style:cursor={image ? cursor : undefined} onwheel={onWheel} onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} onlostpointercapture={up} onpointerleave={() => hovering = false}>
      {#if image}<canvas bind:this={canvas} aria-label={`${image.name}, ${image.width} by ${image.height} pixels`}></canvas>
      {:else}
        <div class="empty-state">
          <div class="empty-art" aria-hidden="true"><svg width="104" height="92" viewBox="0 0 104 92"><path d="M20 32V13h20M64 13h20v19M84 60v19H64M40 79H20V60" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"/><path d="M35 59 67 32M52 33l17-3-4 18" fill="none" stroke="var(--mark)" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"/></svg></div>
          <h1>A picture is a good place to start.</h1>
          <p>Open an image or paste a screenshot<br />to bring it onto your canvas.</p>
          <button class="primary large" onclick={openImage} disabled={busy}>Open an image</button>
          <span class="formats">Opens PNG, JPEG, WebP, TIFF, BMP and GIF.</span>
        </div>
      {/if}
    </div>
    {#if busy}<div class="loading" role="status">Loading image…</div>{/if}
  </div>
  <footer class="statusbar">
    <span class="file-info" title={image?.name}>{image ? image.name : 'No image open'}</span>
    {#if image}<span class="dimensions">{image.width.toLocaleString()} × {image.height.toLocaleString()} px</span>{/if}
    <span id="viewport-help" class="visually-hidden">{image ? 'Drag to draw with the current tool. Hold Space or use the middle button to pan. Arrow keys move the selection.' : `${mod}O to open, ${mod}V to paste.`}</span>
    <div class="zoom-actions" aria-label="Zoom controls">
      <button onclick={fit} disabled={!image} title={`Fit image (${mod}0)`}>Fit</button>
      <button class="square" onclick={() => zoom(0.8)} disabled={!image || view.scale <= MIN_ZOOM} aria-label="Zoom out">−</button>
      <button class="zoom-value" onclick={actual} disabled={!image} title={`Actual size (${mod}1)`}>{image ? zoomLabel : '100%'}</button>
      <button class="square" onclick={() => zoom(1.25)} disabled={!image || view.scale >= MAX_ZOOM} aria-label="Zoom in">＋</button>
    </div>
  </footer>
  <input class="file-input" bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/webp,image/tiff,image/bmp,image/gif" aria-label="Choose an image" onchange={() => { const file = fileInput.files?.[0]; if (file) void load(() => bridge.browserImage(file, file.name)); fileInput.value = ''; }} />
</main>
