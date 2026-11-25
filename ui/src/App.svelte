<script lang="ts">
  import fontLicense from './licenses/NotoSans-OFL.txt?raw';
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import * as bridge from './lib/bridge';
  import AnnotationControls from './lib/AnnotationControls.svelte';
  import PrivacyControls from './lib/PrivacyControls.svelte';
  import StyleControls from './lib/StyleControls.svelte';
  import CanvasControls from './lib/CanvasControls.svelte';
  import CaptureMenu from './lib/CaptureMenu.svelte';
  import SettingsPanel from './lib/SettingsPanel.svelte';
  import UnsavedPrompt from './lib/UnsavedPrompt.svelte';
  import * as capture from './lib/capture';
  import { createDirtyTracker } from './lib/dirty';
  import { loadCore, type Core, type Pt, type Shape, type WirePath } from './lib/core';
  import type { LoadedImage } from './lib/image-packet';
  import { create, dragHandle, handles, topmostAt, translate, DEFAULT_STYLE, restyle, resizeShape, boxFrom, TOOL_KEYS, type Handle, type Tool } from './lib/shapes';
  import { fitViewport, actualSize, zoomAt, panBy, MAX_ZOOM, MIN_ZOOM, type Viewport, type Size } from './lib/viewport';

  interface Item { id: string; shape: Shape; paths: WirePath[] }
  type Gesture =
    | { kind: 'privacy'; id: number; from: Pt; to: Pt; operation: 'pixelate' | 'redact' }
    | { kind: 'pan'; id: number; x: number; y: number }
    | { kind: 'create'; id: number; element: string; tool: Exclude<Tool, 'select'>; from: Pt; sx: number; sy: number; moved: boolean }
    | { kind: 'move'; id: number; element: string; from: Pt; original: Shape }
    | { kind: 'resize'; id: number; element: string; handle: Handle; original: Shape };

  const mac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
  const mod = mac ? '⌘' : 'Ctrl+';
  const TOOLS: { tool: Tool; label: string; key: string; icon: string }[] = [
    { tool: 'stamp', label: 'Stamp', key: 'S', icon: 'M5 20h14M7 16v-5l3-3V4h4v4l3 3v5z' },
    { tool: 'step', label: 'Step counter', key: 'N', icon: 'M12 3a9 9 0 100 18 9 9 0 000-18M10 9l2-2v10M10 17h4' },
    { tool: 'marker', label: 'Marker', key: 'M', icon: 'M3 19l5 2L21 8l-5-5zM12 7l5 5' },
    { tool: 'highlighter', label: 'Highlighter', key: 'H', icon: 'M3 21h10M5 17l6-12 8 4-6 12z' },
    { tool: 'select', label: 'Select', key: 'V', icon: 'M5 3l13 8-6 1.5L9 19z' },
    { tool: 'arrow', label: 'Arrow', key: 'A', icon: 'M4 20L18 6M10 5h9v9' },
    { tool: 'line', label: 'Line', key: 'L', icon: 'M4 20L20 4' },
    { tool: 'rectangle', label: 'Rectangle', key: 'R', icon: 'M4 6h16v12H4z' },
    { tool: 'roundedRectangle', label: 'Rounded rectangle', key: 'U', icon: 'M8 6h8a4 4 0 014 4v4a4 4 0 01-4 4H8a4 4 0 01-4-4v-4a4 4 0 014-4z' },
    { tool: 'ellipse', label: 'Ellipse', key: 'O', icon: 'M12 6c4.4 0 8 2.7 8 6s-3.6 6-8 6-8-2.7-8-6 3.6-6 8-6z' },
  ];
  /** Shape tools fold into one Shapes menu on narrow windows. */
  const SHAPES: Tool[] = ['line', 'rectangle', 'roundedRectangle', 'ellipse'];
  const PRIVACY_TOOLS: { mode: 'pixelate' | 'redact'; label: string; key: string; icon: string }[] = [
    { mode: 'pixelate', label: 'Pixelate', key: 'P', icon: 'M4 4h5v5H4zM9 9h6v6H9zM15 4h5v5h-5zM4 15h5v5H4zM15 15h5v5h-5z' },
    { mode: 'redact', label: 'Redact', key: 'X', icon: 'M3 9h18v6H3z' },
  ];
  let shapesOpen = $state(false);
  /** Screen pixels: how close the pointer must be to a shape or handle. */
  const HIT = 6, HANDLE = 8, CLICK = 3;

  let image = $state.raw<LoadedImage | null>(null);
  let source = $state.raw<HTMLCanvasElement | null>(null);
  let view = $state<Viewport>({ scale: 1, x: 0, y: 0 });
  let area = $state<Size>({ width: 800, height: 600 });
  let stage: HTMLDivElement;
  let canvas = $state<HTMLCanvasElement>();
  let fileInput: HTMLInputElement;
  let notices: HTMLDialogElement;
  let busy = $state(false);
  let error = $state('');
  let outputStatus = $state('');
  let thumbnail = $state('');
  let shareGesture: { id: number | null; cancelled: boolean } | null = null;
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
  let style = $state({ ...DEFAULT_STYLE });
  let stamp = $state('signal-check');
  let privacyMode = $state<'pixelate' | 'redact' | null>(null);
  let items = $state.raw<Item[]>([]);
  let selected = $state<string | null>(null);
  let gesture = $state.raw<Gesture | null>(null);
  let spaceHeld = $state(false);
  let hovering = $state(false);
  let canUndo = $state(false);
  let canRedo = $state(false);
  // Unsaved work: any change since the last load or Save (see lib/dirty.ts).
  const tracker = createDirtyTracker();
  let dirty = $state(false);
  let prompt = $state.raw<((choice: 'save' | 'discard' | 'cancel') => void) | null>(null);
  let settingsOpen = $state(false);
  const modal = $derived(!!prompt || settingsOpen);
  const syncDirty = () => { dirty = tracker.dirty; };
  const paths2d = new WeakMap<WirePath, Path2D>();
  const zoomLabel = $derived(view.scale * 100 < 1 ? `${(view.scale * 100).toPrecision(2)}%` : `${Math.round(view.scale * 100)}%`);
  const transformSource = $derived(image && items.length ? { ...image, editable: true } : image);
  const cursor = $derived(
    gesture?.kind === 'pan' ? 'grabbing'
      : spaceHeld ? 'grab'
      : gesture?.kind === 'move' || (tool === 'select' && hovering) ? 'move'
      : tool === 'select' ? 'default' : 'crosshair');

  /** `changed`: the loaded image is an edit of the open one (a canvas transform), not a new document. */
  async function load(action: () => Promise<LoadedImage | null>, changed = false) {
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
      items = []; selected = null; gesture = null; canUndo = false; canRedo = false;
      tracker.reset(); if (changed) tracker.changed(); syncDirty();
      source = nextSource; image = next;
      view = fitViewport(next, area); fitted = true;
      document.title = `${next.name} — ScribbleCap`;
    } catch (cause) {
      if (alive) error = cause instanceof Error ? cause.message : String(cause);
    } finally { if (alive) busy = false; }
  }
  function rasterCanvas(next: LoadedImage): HTMLCanvasElement {
    const result = document.createElement('canvas'); result.width = next.width; result.height = next.height;
    const context = result.getContext('2d'); if (!context) throw new Error('The image canvas could not be created.');
    context.putImageData(new ImageData(next.pixels,next.width,next.height),0,0); return result;
  }
  async function privacyEdit(rect: [number, number, number, number], operation: 'pixelate' | 'redact') {
    if (!bridge.desktop || !image || !core || busy || gesture) return;
    busy = true; error = '';
    const previous = image.privacyRevision ?? 0;
    let changed = false;
    try {
      const next = await bridge.privacyImage(image, rect, operation); changed = true;
      if (!alive) return;
      const nextSource = rasterCanvas(next);
      core.setBackgroundRevision(next.privacyRevision!); commit();
      source = nextSource; image = next;
      outputStatus = `${operation} applied to background. Annotations remain editable; Undo restores the previous background.`;
    } catch (cause) {
      if (changed) await bridge.restorePrivacy(previous).catch(report);
      report(cause);
    } finally { if (alive) busy = false; }
  }
  /** Resolves true when the open image may be replaced: nothing unsaved, or the person chose Save… (and it saved) or Discard. */
  async function confirmReplace(): Promise<boolean> {
    if (!dirty) return true;
    if (prompt) return false;
    const choice = await new Promise<'save' | 'discard' | 'cancel'>(resolve => { prompt = resolve; });
    prompt = null;
    if (choice === 'save') return saveImage();
    return choice === 'discard';
  }
  /** Ask first, then replace the open image with whatever `action` brings in. */
  async function replaceWith(action: () => Promise<LoadedImage | null>) {
    if (busy || !(await confirmReplace()) || !alive) return;
    await load(action);
  }
  function openImage() {
    if (bridge.desktop) void replaceWith(bridge.openImage);
    else void confirmReplace().then(ok => { if (ok && alive) fileInput.click(); });
  }
  function startCapture(mode: capture.CaptureMode) { void replaceWith(() => capture.captureImage(mode)); }
  function startTimer(seconds: number) { void replaceWith(() => capture.captureTimer(seconds)); }
  async function clipboardImage() {
    if (bridge.desktop) return bridge.pasteImage();
    if (!navigator.clipboard?.read) throw new Error('Use your browser’s Paste command after copying an image.');
    for (const item of await navigator.clipboard.read()) {
      const type = item.types.find(type => type.startsWith('image/'));
      if (type) return bridge.browserImage(await item.getType(type), 'Clipboard image');
    }
    throw new Error('There is no image on the clipboard. Copy an image and try again.');
  }
  function onDrop(event: DragEvent) {
    event.preventDefault();
    if (bridge.desktop || busy || modal) return;
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length !== 1) { error = 'Drop one image at a time.'; return; }
    void replaceWith(() => bridge.browserImage(files[0], files[0].name));
  }
  function pasteImage() { void replaceWith(clipboardImage); }
  function onPaste(event: ClipboardEvent) {
    if (modal) { event.preventDefault(); return; }
    if (editableTarget(event.target)) return;
    event.preventDefault();
    if (bridge.desktop) pasteImage();
    else {
      const file = Array.from(event.clipboardData?.files ?? []).find(file => file.type.startsWith('image/'));
      if (file) void replaceWith(() => bridge.browserImage(file, 'Clipboard image'));
      else error = 'There is no image on the clipboard. Copy an image and try again.';
    }
  }
  /** Runs a share or save action; true when it ran without failing. */
  async function output(action: (current: LoadedImage, document: string) => Promise<void>): Promise<boolean> {
    if (!image || !core || busy || gesture) return false;
    busy = true; error = ''; outputStatus = '';
    try {
      if (!bridge.desktop) throw new Error('Image sharing and saving are available in the desktop app.');
      await action(image, core.document());
      return true;
    } catch (cause) { if (alive) report(cause); return false; }
    finally { if (alive) busy = false; }
  }
  function copyImage() { void output(async (image, document) => { await bridge.copyImage(image, document); if (alive) outputStatus = 'Image copied'; }); }
  /** True only when a file was written; a cancelled dialog or a failure is false. Export is not a Save. */
  async function saveImage(saveAs = false, flat = false): Promise<boolean> {
    let saved = false;
    await output(async (current, document) => {
      const name = await bridge.saveImage(current, document, saveAs, flat);
      if (!name || !alive) return;
      saved = true;
      outputStatus = flat ? `Exported ${name}` : `Saved ${name}`;
      if (!flat) { image = { ...current, name }; window.document.title = `${name} — ScribbleCap`; tracker.saved(); syncDirty(); }
    });
    return saved && !flat;
  }
  function dragImage() {
    const current = { id: null as number | null, cancelled: false };
    shareGesture = current;
    void output(async (image, document) => {
      const id = await bridge.beginDrag();
      current.id = id;
      if (current.cancelled || !alive) { await bridge.cancelDrag(id); return; }
      await bridge.dragImage(image, document, id);
    });
  }
  function cancelShare() {
    if (!shareGesture || !bridge.desktop) return;
    shareGesture.cancelled = true;
    if (shareGesture.id !== null) void bridge.cancelDrag(shareGesture.id).catch(() => {});
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
    if (busy || !core) return false;
    try {
      const paths = core.update(id, shape);
      items = items.map(item => item.id === id ? { id, shape, paths } : item);
      return true;
    } catch (cause) { report(cause); return false; }
  }
  function removeSelected() {
    if (busy || !core || !selected) return;
    try {
      core.remove(selected); items = items.filter(item => item.id !== selected); selected = null;
      let number = 0;
      for (const item of items) if (item.shape.kind === 'step') apply(item.id, { ...item.shape, number: ++number });
    }
    catch (cause) { report(cause); }
  }
  /** End an undoable step. Every finished edit calls this; unchanged steps are dropped. */
  function commit() {
    if (!core) return;
    try {
      const result = core.commit();
      ({ canUndo, canRedo } = result);
      if (result.changed) { tracker.step(); syncDirty(); }
    } catch (cause) { report(cause); }
  }
  async function travel(direction: 'undo' | 'redo') {
    if (!core || !image || gesture || busy) return;
    busy = true;
    let changed = false;
    const previousRevision = image.privacyRevision ?? 0;
    let backgroundChanged = false;
    try {
      const restored = direction === 'undo' ? core.undo() : core.redo();
      changed = !!restored.elements;
      if (!restored.elements) return;
      if (restored.backgroundRevision !== (image.privacyRevision ?? 0)) {
        const next = await bridge.restorePrivacy(restored.backgroundRevision);
        backgroundChanged = true;
        if (!alive) return;
        source = rasterCanvas(next); image = next;
      }
      ({ canUndo, canRedo } = restored); items = restored.elements;
      if (direction === 'undo') tracker.undo(); else tracker.redo();
      syncDirty();
      outputStatus = direction === 'undo' && backgroundChanged
        ? 'Undo restored earlier background pixels. Save/export/copy include them; redo the privacy edit before sharing the cleaned version.'
        : `${direction === 'undo' ? 'Undid' : 'Redid'} edit. ${items.length} annotations`;
      if (!items.some(item => item.id === selected)) selected = null;
    } catch (cause) {
      if (backgroundChanged) await bridge.restorePrivacy(previousRevision).catch(report);
      if (changed) { const rolledBack = direction === 'undo' ? core.redo() : core.undo(); ({ canUndo,canRedo } = rolledBack); }
      report(cause);
    } finally { if (alive) busy = false; }
  }
  function nudge(dx: number, dy: number) {
    const item = items.find(item => item.id === selected);
    if (item) apply(item.id, translate(item.shape, dx, dy));
  }
  function chooseTool(next: Tool) { if (!gesture && !busy) { privacyMode = null; tool = next; outputStatus = `${next} tool selected`; } }
  function changeStyle(next: typeof style) {
    if (gesture || busy) return;
    style = next;
    const item = items.find(item => item.id === selected);
    if (item) { apply(item.id, restyle(item.shape, style)); commit(); }
  }

  function newShape(chosen: Exclude<Tool, 'select'>, from: Pt, to: Pt): Shape {
    const shape = create(chosen, from, to, style);
    if (shape.kind === 'stamp') return { ...shape, stamp };
    if (shape.kind === 'step') return { ...shape, number: 1 + items.filter(item => item.shape.kind === 'step').length };
    return shape;
  }
  function addByKeyboard(from: Pt, to: Pt) {
    if (!core || !image || busy || gesture || tool === 'select') return;
    try {
      let shape = newShape(tool, from, to);
      if (shape.kind === 'freehand') shape = { ...shape, points: core.filterPoints(shape.points) };
      const added = core.add(shape); items = [...items, { ...added, shape }]; selected = added.id;
      commit(); outputStatus = `${tool} annotation added`; stage.focus();
    } catch (cause) { report(cause); }
  }
  function selectByKeyboard(id: string) { selected = id; outputStatus = `Selected ${items.findIndex(item => item.id === id) + 1}`; stage.focus(); }
  function resizeByKeyboard(width: number, height: number) {
    const item = items.find(item => item.id === selected);
    if (item && Number.isFinite(width) && Number.isFinite(height) && width > 0 && height > 0) { apply(item.id, resizeShape(item.shape, width, height)); commit(); stage.focus(); }
  }
  function toolbarKey(event: KeyboardEvent) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault(); event.stopPropagation();
    const buttons = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('button:not(:disabled)')].filter(b => getComputedStyle(b).display !== 'none');
    const index = buttons.indexOf(event.target as HTMLButtonElement);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus(); buttons[next]?.click();
  }
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
    if (modal) {
      // A modal dialog owns the keyboard; Escape still cancels the unsaved-work prompt if focus slipped out of it.
      if (event.key === 'Escape' && prompt) { event.preventDefault(); prompt('cancel'); }
      return;
    }
    if (event.key === 'Escape' && event.target instanceof Element) {
      const details = event.target.closest('details');
      if (details?.open) { details.open = false; details.querySelector('summary')?.focus(); event.preventDefault(); return; }
    }
    if (editableTarget(event.target) || event.altKey) return;
    const key = event.key.toLowerCase();
    if (event.metaKey || event.ctrlKey) {
      if (['o', 'v', 'c', 's', 'z', 'y', '+', '=', '-', '0', '1'].includes(key)) event.preventDefault();
      if (key === 'z') travel(event.shiftKey ? 'redo' : 'undo');
      if (key === 'y' && !mac) travel('redo');
      if (key === 'o' && !event.repeat) openImage();
      if (key === 'v' && !event.repeat) pasteImage();
      if (key === 'c' && !event.repeat) copyImage();
      if (key === 's' && !event.repeat) void saveImage(event.shiftKey);
      if (key === '+' || key === '=') zoom(1.25);
      if (key === '-') zoom(0.8);
      if (key === '0') fit();
      if (key === '1') actual();
      return;
    }
    if (event.target instanceof Element && event.target.closest('button, summary, a')) return;
    if (!image || busy) return;
    if (key === 'p' || key === 'x') { event.preventDefault(); privacyMode = key === 'p' ? 'pixelate' : 'redact'; selected = null; return; }
    if (event.key === '[' || event.key === ']') {
      event.preventDefault(); const sizes = [2, 4, 6, 10, 16]; const index = sizes.indexOf(style.width);
      changeStyle({ ...style, width: sizes[Math.max(0, Math.min(4, index + (event.key === ']' ? 1 : -1)))] }); return;
    }
    if (event.key === ' ') { event.preventDefault(); spaceHeld = true; return; }
    if (event.key === 'Escape') { selected = null; return; }
    if ((event.key === 'Delete' || event.key === 'Backspace') && selected) { event.preventDefault(); removeSelected(); commit(); return; }
    if (!event.shiftKey && TOOL_KEYS[key]) { event.preventDefault(); chooseTool(TOOL_KEYS[key]); return; }
    const delta: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const d = delta[event.key];
    if (!d) return;
    event.preventDefault();
    if (selected) { const step = event.shiftKey ? 10 : 1; nudge(d[0] * step, d[1] * step); }
    else { view = panBy(view, -d[0] * 32, -d[1] * 32); fitted = false; }
  }
  function onKeyUp(event: KeyboardEvent) {
    if (editableTarget(event.target)) return;
    if (event.key === ' ') spaceHeld = false;
    // Nudges held down or repeated end as one step when the arrow key is released.
    if (event.key.startsWith('Arrow')) commit();
  }
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
    if (busy || !image || !core || !event.isPrimary || (event.button !== 0 && event.button !== 1)) return;
    stage.focus(); event.preventDefault(); stage.setPointerCapture(event.pointerId);
    const id = event.pointerId;
    if (event.button === 1 || spaceHeld) { gesture = { kind: 'pan', id, x: event.clientX, y: event.clientY }; return; }
    const p = toImage(event);
    if (privacyMode) { gesture = { kind: 'privacy', id, from: p, to: p, operation: privacyMode }; return; }
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
      const shape = newShape(tool, p, p);
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
    if (g.kind === 'privacy') { gesture = { ...g, to: p }; return; }
    if (g.kind === 'create') {
      if (!g.moved && Math.hypot(event.clientX - g.sx, event.clientY - g.sy) < CLICK) return;
      gesture = { ...g, moved: true };
      const current = items.find(item => item.id === g.element)?.shape;
      apply(g.element, current?.kind === 'freehand' && current.points.length >= 8192 ? current : current?.kind === 'freehand' ? { ...current, points: [...current.points, p] } : newShape(g.tool, g.from, p));
    } else if (g.kind === 'move') apply(g.element, translate(g.original, p[0] - g.from[0], p[1] - g.from[1]));
    else apply(g.element, dragHandle(g.original, g.handle, p));
  }
  function up(event: PointerEvent) {
    if (gesture?.id !== event.pointerId) return;
    const g = gesture;
    gesture = null;
    if (stage.hasPointerCapture(event.pointerId)) stage.releasePointerCapture(event.pointerId);
    if (event.type === 'pointercancel' || event.type === 'lostpointercapture') {
      if (g.kind === 'create') removeSelected();
      else if (g.kind === 'move' || g.kind === 'resize') apply(g.element, g.original);
      return;
    }
    if (g.kind === 'privacy' && image) {
      const [left, top, width, height] = boxFrom(g.from, g.to);
      const x = Math.max(0, Math.floor(left)), y = Math.max(0, Math.floor(top));
      const right = Math.min(image.width, Math.ceil(left + width)), bottom = Math.min(image.height, Math.ceil(top + height));
      if (right > x && bottom > y && width * view.scale >= CLICK && height * view.scale >= CLICK) void privacyEdit([x, y, right - x, bottom - y], g.operation);
      return;
    }
    // A click without a drag draws nothing.
    if (g.kind === 'create' && !g.moved && g.tool !== 'stamp' && g.tool !== 'step') removeSelected();
    else if (g.kind === 'create' && core) {
      const item = items.find(item => item.id === g.element);
      if (item?.shape.kind === 'freehand') apply(item.id, { ...item.shape, points: core.filterPoints(item.shape.points) });
    }
    if (g.kind !== 'pan') commit();
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let unmenu: (() => void) | undefined;
    let mounted = true;
    const stops: (() => void)[] = [];
    if (bridge.desktop) {
      void listen<string>('shell-menu', ({ payload }) => {
        if (modal) return; // the unsaved prompt or settings dialog owns input
        if (editableTarget(document.activeElement) && ['undo','redo','copy','paste'].includes(payload)) { document.execCommand(payload); return; }
        const actions: Record<string, () => void> = { open: openImage, paste: pasteImage, copy: copyImage, save: () => saveImage(), 'save-as': () => saveImage(true), export: () => saveImage(true,true), undo: () => travel('undo'), redo: () => travel('redo'), fit, actual, 'zoom-in': () => zoom(1.25), 'zoom-out': () => zoom(.8) };
        actions[payload]?.();
      }).then(stop => { if (mounted) unmenu = stop; else stop(); }).catch(report);
      void listen('shell-file-drop', () => { if (!modal) void replaceWith(bridge.droppedImage); }).then(stop => {
        if (mounted) unlisten = stop; else stop();
      }).catch(cause => { if (alive) error = String(cause); });
      // The capture's image is fetched from Rust once the overlays finish (M4-5).
      const subscribe = (name: string, handler: (payload: unknown) => void) => {
        void listen(name, event => handler(event.payload)).then(stop => {
          if (mounted) stops.push(stop); else stop();
        }).catch(cause => { if (alive) error = String(cause); });
      };
      subscribe('capture-ready', () => { void load(capture.captureResult); });
      subscribe('capture-save-failed', message => { error = String(message); });
    }
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
    return () => { mounted = false; unlisten?.(); unmenu?.(); stops.forEach(stop => stop()); observer.disconnect(); scheme?.removeEventListener?.('change', onScheme); };
  });
  onDestroy(() => { alive = false; cancelShare(); });

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
    const background = source, annotations = items;
    if (!background) { thumbnail = ''; return; }
    const timer = window.setTimeout(() => {
      const preview = window.document.createElement('canvas');
      const scale = Math.min(80 / background.width, 56 / background.height);
      preview.width = Math.max(1, Math.round(background.width * scale));
      preview.height = Math.max(1, Math.round(background.height * scale));
      const context = preview.getContext('2d');
      if (!context) return;
      context.scale(scale, scale); context.drawImage(background, 0, 0);
      for (const item of annotations) for (const path of item.paths) paint(context, path);
      thumbnail = preview.toDataURL('image/png');
    }, 120);
    return () => window.clearTimeout(timer);
  });

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
    if (gesture?.kind === 'privacy') {
      const [x,y,w,h] = boxFrom(gesture.from,gesture.to);
      context.fillStyle = 'rgba(0,0,0,0.45)'; context.fillRect(x,y,w,h);
      context.strokeStyle = token('--accent', '#0a64d8'); context.lineWidth = 2 / view.scale; context.strokeRect(x,y,w,h);
    }
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

<svelte:window ondrop={onDrop} ondragover={(event) => event.preventDefault()} onkeydown={onKey} onkeyup={onKeyUp} onpaste={onPaste} onblur={() => spaceHeld = false} onpointerup={cancelShare} onpointercancel={cancelShare} />
<main aria-busy={busy} class:mac={mac && bridge.desktop}>
  <header class="titlebar" data-tauri-drag-region>
    {#if bridge.desktop}<CaptureMenu {busy} capture={startCapture} timer={startTimer} settings={() => settingsOpen = true} />{/if}
    <!-- Left: Capture, then history. Open and Paste are in the File menu, the shortcuts and the empty state. -->
    <div class="history-actions">
      <button class="tool" aria-label="Undo" title={`Undo (${mod}Z)`} disabled={!canUndo || !!gesture} onclick={() => travel('undo')}>
        <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 14L4 9l5-5M4 9h10a6 6 0 010 12h-3" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
      <button class="tool" aria-label="Redo" title={mac ? 'Redo (⇧⌘Z)' : 'Redo (Ctrl+Y)'} disabled={!canRedo || !!gesture} onclick={() => travel('redo')}>
        <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 14l5-5-5-5M20 9H10a6 6 0 000 12h3" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
    </div>
    <!-- Right: tools, then colour and size, then more, then share (design review D4). -->
    <div class="right-actions">
      {#if tool === 'stamp'}<label class="stamp-choice">Stamp <select bind:value={stamp}><option value="signal-check">Signal check</option><option value="signal-alert">Signal alert</option><option value="signal-spark">Signal spark</option></select></label>{/if}
      <div class="tools" role="toolbar" tabindex="-1" aria-label="Tools" onkeydown={toolbarKey}>
        {#each TOOLS as t (t.tool)}
          <button class="tool" class:shape-tool={SHAPES.includes(t.tool)} tabindex={!privacyMode && tool === t.tool ? 0 : -1} aria-keyshortcuts={t.key} aria-label={t.label} aria-pressed={!privacyMode && tool === t.tool} title={`${t.label} (${t.key})`} disabled={!image || busy} onclick={() => chooseTool(t.tool)}>
            <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d={t.icon} fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </button>
        {/each}
        <details class="shape-menu" bind:open={shapesOpen}>
          <summary class="tool" aria-label="Shapes" title="Shapes" class:active={!privacyMode && SHAPES.includes(tool)}>
            <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d={(TOOLS.find(t => t.tool === tool && SHAPES.includes(tool)) ?? TOOLS.find(t => t.tool === 'rectangle'))!.icon} fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </summary>
          {#if shapesOpen}<div class="shape-list">
            {#each TOOLS.filter(t => SHAPES.includes(t.tool)) as t (t.tool)}
              <button class="tool" tabindex="-1" aria-label={t.label} aria-pressed={!privacyMode && tool === t.tool} title={`${t.label} (${t.key})`} disabled={!image || busy} onclick={() => { chooseTool(t.tool); shapesOpen = false; }}>
                <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d={t.icon} fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
              </button>
            {/each}
          </div>{/if}
        </details>
        {#each PRIVACY_TOOLS as p (p.mode)}
          <button class="tool" tabindex={privacyMode === p.mode ? 0 : -1} aria-keyshortcuts={p.key} aria-label={p.label} aria-pressed={privacyMode === p.mode} title={`${p.label} (${p.key})`} disabled={!image || busy || !bridge.desktop} onclick={() => { if (!gesture && !busy) { privacyMode = p.mode; selected = null; } }}>
            <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true"><path d={p.icon} fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" /></svg>
          </button>
        {/each}
      </div>
      <label class="compact-tools">Tool <select value={tool} disabled={!image || busy} onchange={(event) => chooseTool(event.currentTarget.value as Tool)}>{#each TOOLS as t (t.tool)}<option value={t.tool}>{t.label} ({t.key})</option>{/each}</select></label>
      <StyleControls {style} disabled={!image || busy || !!gesture} change={changeStyle} />
      <details class="more-actions">
        <summary aria-label="More: canvas, annotations and privacy" title="More">⋯</summary>
        <div class="more-panel">
          <CanvasControls image={transformSource} {busy} load={(action) => load(action, true)} />
          <AnnotationControls {items} {selected} {tool} disabled={!image || busy || !!gesture} add={addByKeyboard} select={selectByKeyboard} resize={resizeByKeyboard} />
          <PrivacyControls {image} disabled={busy || !!gesture || !bridge.desktop} apply={privacyEdit} />
        </div>
      </details>
      <details class="share-actions"><summary>Share</summary><div>
        <button onclick={copyImage} disabled={!image || busy} title={`Copy image (${mod}C)`}>Copy image</button>
        <button onclick={() => saveImage()} disabled={!image || busy} title={`Save (${mod}S)`}>Save image</button>
        <button onclick={() => saveImage(true)} disabled={!image || busy} title={`Save as (${mod}Shift+S)`}>Save as…</button>
        <button onclick={() => saveImage(true, true)} disabled={!image || busy}>Export flat…</button>
      </div></details>
    </div>
  </header>
  {#if privacyMode}<p class="privacy-notice">{privacyMode === 'redact' ? 'Redact' : 'Pixelate'}: drag a region to replace background pixels. Annotations stay editable and session undo remains available. Save the current cleaned version; existing files are unchanged.</p>{/if}
  {#if error}<div class="error" role="alert"><span>{error}</span><button onclick={() => error = ''} aria-label="Dismiss error">×</button></div>{/if}
  <div class="workspace">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (The viewport is focusable for keyboard editing and panning; toolbar buttons provide zoom.) -->
    <div class="viewport" bind:this={stage} role="region" aria-label="Image viewport" aria-describedby="viewport-help annotation-help" tabindex="0" style:cursor={image ? cursor : undefined} onwheel={onWheel} onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} onlostpointercapture={up} onpointerleave={() => hovering = false}>
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
    <div class:output-status={!!outputStatus} role="status" aria-live="polite">{outputStatus}</div>
    {#if image && bridge.desktop}<button class="drag-out" disabled={busy} style:left={`${Math.max(8, Math.min(area.width - 120, view.x + image.width * view.scale - 112))}px`} style:top={`${Math.max(8, Math.min(area.height - 48, view.y + image.height * view.scale))}px`} onpointerdown={(event) => { if (event.button === 0) { event.preventDefault(); dragImage(); } }} onclick={(event) => { if (event.detail === 0) copyImage(); }} aria-label="Drag image out; Enter copies the image" title="Drag this image into another app; Enter copies"><img src={thumbnail} alt="" draggable="false" /><span>Drag out</span></button>{/if}
    <span class="visually-hidden" aria-live="polite">{items.length} annotations. {selected ? `Selected annotation ${items.findIndex(item => item.id === selected) + 1}` : 'No annotation selected'}.</span>
    {#if busy}<div class="loading" role="status">Loading image…</div>{/if}
  </div>
  <footer class="statusbar">
    <span class="file-info" title={image?.name}>{image ? image.name : 'No image open'}</span>
    {#if image}<span class="dimensions">{image.width.toLocaleString()} × {image.height.toLocaleString()} px</span>{/if}
    <span id="viewport-help" class="visually-hidden">{image ? 'Drag to draw. Privacy tools edit the background and keep session undo. Use Annotations for keyboard creation, selection and resizing; Privacy for numeric regions. Hold Space to pan. Arrow keys move the selection.' : `${mod}O to open, ${mod}V to paste.`}</span>
    <button class="notices-link" onclick={() => notices.showModal()}>Third-party notices</button>
    <div class="zoom-actions" aria-label="Zoom controls">
      <button onclick={fit} disabled={!image} title={`Fit image (${mod}0)`}>Fit</button>
      <button class="square" onclick={() => zoom(0.8)} disabled={!image || view.scale <= MIN_ZOOM} aria-label="Zoom out">−</button>
      <button class="zoom-value" onclick={actual} disabled={!image} title={`Actual size (${mod}1)`}>{image ? zoomLabel : '100%'}</button>
      <button class="square" onclick={() => zoom(1.25)} disabled={!image || view.scale >= MAX_ZOOM} aria-label="Zoom in">＋</button>
    </div>
  </footer>
  <dialog bind:this={notices} aria-labelledby="notices-title">
    <h2 id="notices-title">Third-party notices</h2>
    <p>Noto Sans Regular is bundled under the SIL Open Font License 1.1. ScribbleCap code is MIT OR Apache-2.0; the font retains its separate licence.</p>
    <pre>{fontLicense}</pre>
    <form method="dialog"><button>Close notices</button></form>
  </dialog>
  {#if prompt}<UnsavedPrompt choose={prompt} />{/if}
  {#if settingsOpen}<SettingsPanel close={() => settingsOpen = false} />{/if}
  <input class="file-input" bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/webp,image/tiff,image/bmp,image/gif" aria-label="Choose an image" onchange={() => { const file = fileInput.files?.[0]; if (file) void load(() => bridge.browserImage(file, file.name)); fileInput.value = ''; }} />
</main>

<style>
  dialog { max-width: min(42rem, 92vw); max-height: 85vh; color: var(--ink); background: var(--surface); border: 1px solid var(--line); border-radius: .6rem; }
  dialog pre { white-space: pre-wrap; font-size: 12px; }
  .compact-tools { display: none; }
  .right-actions { display: flex; align-items: center; gap: 6px; margin-left: auto; }
  .shape-menu { display: none; position: relative; }
  .shape-menu summary { list-style: none; cursor: pointer; }
  .shape-menu summary::-webkit-details-marker { display: none; }
  .shape-menu summary.active { color: var(--accent); background: var(--surface); border: 1px solid var(--line); border-radius: 6px; }
  .shape-list { position: absolute; top: 100%; left: 0; z-index: 6; display: flex; gap: 2px; padding: 4px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; }
  @media (max-width: 960px) { .tools .shape-tool { display: none; } .shape-menu { display: block; } }
  @media (max-width: 720px) { .tools { display: none; } .compact-tools { display: flex; gap: .3rem; align-items: center; } }
  .more-actions { position: relative; }
  .more-actions > summary { cursor: pointer; padding: .5rem; list-style: none; font-size: 18px; line-height: 1; }
  .more-panel { position: absolute; top: 100%; right: 0; z-index: 6; display: grid; gap: .2rem; min-width: 12rem; padding: .4rem; background: var(--surface); border: 1px solid var(--line); border-radius: .5rem; }
  /* The panels open in place inside More rather than as their own popovers. */
  .more-panel :global(details > :not(summary)) { position: static; width: auto; max-width: min(23rem, 90vw); box-shadow: none; }
  .share-actions { position: relative; }
  .share-actions summary { cursor: pointer; padding: .5rem; }
  .share-actions > div { position: absolute; top: 100%; right: 0; z-index: 5; display: grid; gap: .4rem; min-width: 9rem; padding: .5rem; background: var(--surface); border: 1px solid var(--line); border-radius: .5rem; }
  .output-status { position: absolute; z-index: 4; bottom: 1rem; left: 1rem; max-width: calc(100% - 2rem); background: var(--surface); padding: .4rem .7rem; border-radius: .4rem; }
  .drag-out { position: absolute; z-index: 3; display: flex; align-items: center; gap: .4rem; padding: .2rem .5rem; cursor: grab; box-shadow: 0 2px 10px #0003; }
  .drag-out img { width: 40px; height: 28px; object-fit: contain; }
</style>
