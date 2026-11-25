import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import App from './App.svelte';
import { listen } from '@tauri-apps/api/event';
import * as bridge from './lib/bridge';
import * as capture from './lib/capture';
import type { LoadedImage } from './lib/image-packet';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('./lib/bridge', () => ({ droppedImage: vi.fn(), saveImage: vi.fn(), copyImage: vi.fn(), dragImage: vi.fn(), restorePrivacy: vi.fn(), privacyImage: vi.fn(), beginDrag: vi.fn(), cancelDrag: vi.fn(), desktop: true, openImage: vi.fn(), pasteImage: vi.fn(), browserImage: vi.fn() }));
vi.mock('./lib/capture', () => ({ captureState: vi.fn(), captureImage: vi.fn(), captureTimer: vi.fn(), captureResult: vi.fn(), getSettings: vi.fn(), setAutoSave: vi.fn(), chooseCaptureFolder: vi.fn() }));
// The real core WebAssembly module (built by `pnpm test`), loaded from disk; a fresh
// instance per test keeps documents independent.
vi.mock('./lib/core', async original => {
  const actual = await original<typeof import('./lib/core')>();
  const { readFileSync } = await import('node:fs');
  const bytes = readFileSync('src/generated/core.wasm');
  return { ...actual, loadCore: () => actual.createCore(bytes) };
});

function image(name = 'test.png'): LoadedImage { return { name, width: 2, height: 1, pixels: new Uint8ClampedArray([255, 0, 0, 255, 0, 0, 255, 255]) }; }
const transform = vi.fn();
// Each redraw sets the image transform, then a screen-space transform for the image edge.
const viewTransform = () => transform.mock.calls.at(-2);
let resize: ResizeObserverCallback | undefined;
beforeEach(() => {
  vi.mocked(bridge.openImage).mockReset(); vi.mocked(bridge.pasteImage).mockReset(); transform.mockReset();
  vi.mocked(listen).mockReset().mockResolvedValue(vi.fn());
  vi.mocked(bridge.droppedImage).mockReset();
  vi.mocked(capture.captureState).mockReset().mockResolvedValue({ wayland: false, previous: true });
  vi.mocked(capture.captureImage).mockReset().mockResolvedValue(null);
  vi.mocked(capture.captureTimer).mockReset().mockResolvedValue(null);
  vi.mocked(capture.captureResult).mockReset();
  vi.mocked(capture.getSettings).mockReset().mockResolvedValue({ captureFolder: null, autoSave: false });
  vi.mocked(capture.chooseCaptureFolder).mockReset();
  vi.mocked(bridge.saveImage).mockReset();
  vi.mocked(bridge.privacyImage).mockReset(); vi.mocked(bridge.restorePrivacy).mockReset();
  vi.mocked(bridge.copyImage).mockReset().mockResolvedValue();
  vi.mocked(bridge.dragImage).mockReset().mockResolvedValue();
  vi.mocked(bridge.beginDrag).mockReset().mockResolvedValue(27);
  vi.mocked(bridge.cancelDrag).mockReset().mockResolvedValue();
  vi.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockReturnValue('data:image/png;base64,AA==');
  resize = undefined;
  vi.stubGlobal('ResizeObserver', class { constructor(callback: ResizeObserverCallback) { resize = callback; } observe() {} disconnect() {} });
  vi.stubGlobal('ImageData', class { constructor(public data: Uint8ClampedArray, public width: number, public height: number) {} });
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ scale: vi.fn(), putImageData: vi.fn(), setTransform: transform, clearRect: vi.fn(), fillRect: vi.fn(), drawImage: vi.fn(), createPattern: vi.fn(), strokeRect: vi.fn(), save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn(), fill: vi.fn(), stroke: vi.fn() } as unknown as CanvasRenderingContext2D);
});

describe('image shell', () => {
  it('starts with an accessible empty state and disabled zoom', () => {
    render(App);
    expect(screen.getByRole('heading', { name: 'A picture is a good place to start.' })).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Zoom in' }) as HTMLButtonElement).disabled).toBe(true);
  });
  it('opens an image, displays its name and dimensions, and zooms without reloading it', async () => {
    vi.mocked(bridge.openImage).mockResolvedValue(image());
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    expect(screen.getByText('2 × 1 px')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(screen.getByRole('button', { name: '125%' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: '125%' }));
    expect(screen.getByRole('button', { name: '100%' })).toBeTruthy();
    expect(bridge.openImage).toHaveBeenCalledTimes(1);
  });
  it('preserves the current image on cancel and on error, and can dismiss the error', async () => {
    vi.mocked(bridge.openImage).mockResolvedValueOnce(image()).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error('Unsupported image'));
    render(App);
    const open = screen.getByRole('button', { name: 'Open image' });
    await fireEvent.click(open);
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    await fireEvent.click(open);
    await waitFor(() => expect((open as HTMLButtonElement).disabled).toBe(false));
    expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy();
    await fireEvent.click(open);
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Unsupported image'));
    expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Dismiss error' }));
    expect(screen.queryByRole('alert')).toBeNull();
  });
  it('pastes using the platform shortcut and supports keyboard zoom and pan', async () => {
    vi.mocked(bridge.pasteImage).mockResolvedValue(image('Clipboard image'));
    render(App);
    await fireEvent.keyDown(window, { key: 'v', metaKey: true });
    await waitFor(() => expect(screen.getByLabelText('Clipboard image, 2 by 1 pixels')).toBeTruthy());
    await fireEvent.keyDown(window, { key: '=', ctrlKey: true });
    expect(screen.getByRole('button', { name: '125%' })).toBeTruthy();
    const before = viewTransform();
    await fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(viewTransform()).not.toEqual(before);
    await fireEvent.keyDown(window, { key: '0', metaKey: true });
    expect(screen.getByRole('button', { name: '100%' })).toBeTruthy();
  });
  it('ignores duplicate input requests while native input is pending', async () => {
    let resolve!: (value: LoadedImage | null) => void;
    vi.mocked(bridge.openImage).mockReturnValue(new Promise(done => resolve = done));
    render(App);
    await fireEvent.keyDown(window, { key: 'o', ctrlKey: true });
    await fireEvent.keyDown(window, { key: 'o', ctrlKey: true });
    expect(bridge.openImage).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Loading image…')).toBeTruthy();
    resolve(null);
    await waitFor(() => expect(screen.queryByText('Loading image…')).toBeNull());
  });
  it('pans by dragging, releases pointer capture and handles wheel zoom', async () => {
    vi.mocked(bridge.openImage).mockResolvedValue(image());
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    const viewport = screen.getByRole('region', { name: 'Image viewport' });
    await fireEvent.keyDown(window, { key: 'v' }); // select tool: dragging empty space pans
    const capture = vi.fn(), release = vi.fn();
    Object.assign(viewport, { setPointerCapture: capture, hasPointerCapture: () => true, releasePointerCapture: release });
    await fireEvent.pointerDown(viewport, { pointerId: 1, button: 0, isPrimary: true, clientX: 30, clientY: 40 });
    expect(capture).toHaveBeenCalledWith(1);
    const before = viewTransform();
    await fireEvent.pointerMove(viewport, { pointerId: 1, isPrimary: true, clientX: 90, clientY: 70 });
    expect(viewTransform()).not.toEqual(before);
    await fireEvent.pointerUp(viewport, { pointerId: 1 });
    expect(release).toHaveBeenCalledWith(1);
    expect(viewport.classList.contains('dragging')).toBe(false);
    await fireEvent.wheel(viewport, { deltaY: -10, ctrlKey: true, clientX: 100, clientY: 100 });
    expect(screen.getByRole('button', { name: '111%' })).toBeTruthy();
  });
  it('keeps a fitted image fitted when the viewport is resized', async () => {
    vi.mocked(bridge.openImage).mockResolvedValue({ name: 'big.png', width: 2000, height: 1000, pixels: new Uint8ClampedArray(2000 * 1000 * 4) });
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('big.png, 2000 by 1000 pixels')).toBeTruthy());
    resize?.([{ contentRect: { width: 1040, height: 540 } } as ResizeObserverEntry], {} as ResizeObserver);
    await waitFor(() => expect(screen.getByRole('button', { name: '50%' })).toBeTruthy());
    resize?.([{ contentRect: { width: 540, height: 540 } } as ResizeObserverEntry], {} as ResizeObserver);
    await waitFor(() => expect(screen.getByRole('button', { name: '25%' })).toBeTruthy());
  });
  it('keeps zoom when the viewport is resized', async () => {
    vi.mocked(bridge.openImage).mockResolvedValue(image());
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    await fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }));
    const before = viewTransform();
    resize?.([{ contentRect: { width: 1000, height: 800 } } as ResizeObserverEntry], {} as ResizeObserver);
    await waitFor(() => expect(viewTransform()).not.toEqual(before));
    expect(screen.getByRole('button', { name: '125%' })).toBeTruthy();
  });
});

describe('annotation tools', () => {
  const big = (): LoadedImage => ({ name: 'shot.png', width: 400, height: 200, pixels: new Uint8ClampedArray(400 * 200 * 4) });
  async function openBig() {
    vi.mocked(bridge.openImage).mockResolvedValue(big());
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('shot.png, 400 by 200 pixels')).toBeTruthy());
    const viewport = screen.getByRole('region', { name: 'Image viewport' });
    Object.assign(viewport, { setPointerCapture: vi.fn(), hasPointerCapture: () => true, releasePointerCapture: vi.fn(), getBoundingClientRect: () => ({ left: 0, top: 0 }) });
    // jsdom area is 800×600 before any resize: a 400×200 image fits at 100%, offset (200, 200).
    return viewport;
  }
  const drag = async (target: HTMLElement, from: [number, number], to: [number, number]) => {
    await fireEvent.pointerDown(target, { pointerId: 1, button: 0, isPrimary: true, clientX: from[0], clientY: from[1] });
    await fireEvent.pointerMove(target, { pointerId: 1, isPrimary: true, clientX: to[0], clientY: to[1] });
    await fireEvent.pointerUp(target, { pointerId: 1 });
  };
  const pressed = () => screen.getAllByRole('button', { pressed: true }).filter(b => b.closest('[role=toolbar]')).map(b => b.getAttribute('aria-label'));

  it('starts with the arrow tool and switches tools by key and button', async () => {
    await openBig();
    expect(pressed()).toEqual(['Arrow']);
    await fireEvent.keyDown(window, { key: 'r' });
    expect(pressed()).toEqual(['Rectangle']);
    await fireEvent.click(screen.getByRole('button', { name: 'Ellipse' }));
    expect(pressed()).toEqual(['Ellipse']);
    await fireEvent.keyDown(window, { key: 'u' });
    expect(pressed()).toEqual(['Rounded rectangle']);
  });
  it('draws a shape by dragging, ignores a plain click, and paints core paths', async () => {
    const viewport = await openBig();
    const fill = vi.fn();
    vi.stubGlobal('Path2D', class { constructor(public d: string) {} });
    (HTMLCanvasElement.prototype.getContext as unknown as { mockReturnValue(v: unknown): void }).mockReturnValue({ scale: vi.fn(), putImageData: vi.fn(), setTransform: transform, clearRect: vi.fn(), fillRect: vi.fn(), drawImage: vi.fn(), createPattern: vi.fn(), strokeRect: vi.fn(), save: vi.fn(), restore: vi.fn(), beginPath: vi.fn(), rect: vi.fn(), clip: vi.fn(), fill, stroke: vi.fn() });
    await fireEvent.pointerDown(viewport, { pointerId: 1, button: 0, isPrimary: true, clientX: 250, clientY: 250 });
    await fireEvent.pointerUp(viewport, { pointerId: 1 });
    expect(fill).not.toHaveBeenCalled();
    await drag(viewport, [250, 250], [400, 330]);
    await waitFor(() => expect(fill).toHaveBeenCalled());
    const painted = fill.mock.calls.at(-1)![0] as { d: string };
    expect(painted.d).toMatch(/^M /);
  });
  it('draws multi-point marker and translucent highlighter strokes as one undo step', async () => {
    const viewport = await openBig();
    await fireEvent.keyDown(window, { key: 'h' });
    await fireEvent.pointerDown(viewport, { pointerId: 1, button: 0, isPrimary: true, clientX: 220, clientY: 220 });
    await fireEvent.pointerMove(viewport, { pointerId: 1, isPrimary: true, clientX: 260, clientY: 240 });
    await fireEvent.pointerMove(viewport, { pointerId: 1, isPrimary: true, clientX: 300, clientY: 220 });
    await fireEvent.pointerUp(viewport, { pointerId: 1 });
    await fireEvent.click(screen.getByRole('button', { name: 'Copy image' }));
    const doc = JSON.parse(vi.mocked(bridge.copyImage).mock.calls.at(-1)![1]);
    expect(doc.elements[0]).toMatchObject({ kind: 'freehand', tool: 'highlighter', color: '#E5194A66', width: 24 });
    expect(doc.elements[0].points.length).toBeGreaterThan(2);
    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    expect(screen.getByText('Annotations (0)')).toBeTruthy();
  });
  it('applies selected styles and supports keyboard-only creation, selection and resize', async () => {
    const viewport = await openBig();
    await fireEvent.keyDown(window, { key: 'r' });
    await fireEvent.click(screen.getByRole('button', { name: 'Add annotation' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Colour #007aff' }));
    await fireEvent.change(screen.getByLabelText('Stroke size'), { target: { value: '10' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Resize annotation' }));
    await fireEvent.click(screen.getByRole('button', { name: '1. rectangle' }));
    expect(document.activeElement).toBe(viewport);
    await fireEvent.keyDown(viewport, { key: 'ArrowRight' });
    await fireEvent.keyUp(viewport, { key: 'ArrowRight' });
    await fireEvent.click(screen.getByRole('button', { name: 'Copy image' }));
    const doc = JSON.parse(vi.mocked(bridge.copyImage).mock.calls.at(-1)![1]);
    expect(doc.elements[0]).toMatchObject({ stroke: '#007AFFFF', width: 10, rect: { x: 1, y: 0, width: 100, height: 60 } });
    await fireEvent.keyDown(viewport, { key: 'Delete' });
    expect(screen.getByText('Annotations (0)')).toBeTruthy();
  });
  it('places original stamps and renumbers steps after deletion and undo', async () => {
    const viewport = await openBig();
    await fireEvent.keyDown(window, { key: 's' });
    await fireEvent.click(screen.getByRole('button', { name: 'Add annotation' }));
    await fireEvent.keyDown(window, { key: 'n' });
    await fireEvent.click(screen.getByRole('button', { name: 'Add annotation' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Add annotation' }));
    await fireEvent.click(screen.getByRole('button', { name: '2. step 1' }));
    await fireEvent.keyDown(viewport, { key: 'Delete' });
    expect(screen.getByRole('button', { name: '2. step 1' })).toBeTruthy();
    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    expect(screen.getByRole('button', { name: '3. step 2' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Copy image' }));
    const doc = JSON.parse(vi.mocked(bridge.copyImage).mock.calls.at(-1)![1]);
    expect(doc.elements[0].stamp).toBe('signal-check');
  });
  it('keeps annotations editable across background privacy edits, undo and redo', async () => {
    const viewport = await openBig();
    await drag(viewport,[220,220],[300,280]);
    const large = { name: 'test.png', width: 400, height: 200, pixels: new Uint8ClampedArray(400*200*4) };
    vi.mocked(bridge.privacyImage).mockResolvedValue({ ...large, privacyRevision: 1 });
    vi.mocked(bridge.restorePrivacy).mockImplementation(async revision => ({ ...large, privacyRevision: revision }));
    await fireEvent.click(screen.getByRole('button', { name: 'Apply privacy edit' }));
    await waitFor(() => expect(bridge.privacyImage).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByText('Annotations (1)')).toBeTruthy());
    await waitFor(() => expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(false));
    await fireEvent.keyDown(window,{key:'z',ctrlKey:true});
    await waitFor(() => expect(bridge.restorePrivacy).toHaveBeenCalledWith(0));
    await waitFor(() => expect(screen.getByText(/Undo restored earlier background pixels/)).toBeTruthy());
    expect(screen.getByText('Annotations (1)')).toBeTruthy();
    await waitFor(() => expect((screen.getByRole('button', { name: 'Redo' }) as HTMLButtonElement).disabled).toBe(false));
    await fireEvent.keyDown(window,{key:'z',ctrlKey:true,shiftKey:true});
    await waitFor(() => expect(bridge.restorePrivacy).toHaveBeenCalledWith(1));
    await waitFor(() => expect(screen.queryByText(/Undo restored earlier background pixels/)).toBeNull());
    await fireEvent.click(screen.getByRole('button', { name: '1. arrow' }));
    await fireEvent.keyDown(viewport,{key:'ArrowRight'}); await fireEvent.keyUp(viewport,{key:'ArrowRight'});
    await fireEvent.click(screen.getByRole('button', { name: 'Copy image' }));
    expect(JSON.parse(vi.mocked(bridge.copyImage).mock.calls.at(-1)![1]).elements).toHaveLength(1);
  });
  it('navigates tools with the keyboard and preserves native text-field keys', async () => {
    await openBig();
    const arrow = screen.getByRole('button', { name: 'Arrow' });
    arrow.focus();
    await fireEvent.keyDown(arrow, { key: 'ArrowRight' });
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Line' }));
    const input = screen.getByLabelText('Start X');
    await fireEvent.keyDown(input, { key: 'h' });
    expect(pressed()).toEqual(['Line']);
    const details = input.closest('details')!; details.open = true;
    await fireEvent.keyDown(input, { key: 'Escape' });
    expect(details.open).toBe(false);
    expect(document.activeElement).toBe(details.querySelector('summary'));
  });
  it('discards cancelled strokes without an undo step', async () => {
    const viewport = await openBig();
    await fireEvent.keyDown(window, { key: 'm' });
    await fireEvent.pointerDown(viewport, { pointerId: 1, button: 0, isPrimary: true, clientX: 220, clientY: 220 });
    await fireEvent.pointerMove(viewport, { pointerId: 1, isPrimary: true, clientX: 260, clientY: 240 });
    await fireEvent.pointerCancel(viewport, { pointerId: 1 });
    expect(screen.getByText('Annotations (0)')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(true);
  });
  it('selects, moves and deletes a shape', async () => {
    const viewport = await openBig();
    await fireEvent.keyDown(window, { key: 'l' });
    await drag(viewport, [220, 220], [320, 220]);
    await fireEvent.keyDown(window, { key: 'Escape' });
    await fireEvent.keyDown(window, { key: 'v' });
    // Hovering the line in select mode shows the move cursor.
    await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: 270, clientY: 221 });
    expect(viewport.style.cursor).toBe('move');
    await drag(viewport, [270, 221], [270, 261]);
    await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: 270, clientY: 221 });
    expect(viewport.style.cursor).toBe('default');
    await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: 270, clientY: 261 });
    expect(viewport.style.cursor).toBe('move');
    await fireEvent.pointerDown(viewport, { pointerId: 3, button: 0, isPrimary: true, clientX: 270, clientY: 261 });
    await fireEvent.pointerUp(viewport, { pointerId: 3 });
    await fireEvent.keyDown(window, { key: 'Delete' });
    await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: 270, clientY: 261 });
    expect(viewport.style.cursor).toBe('default');
  });
  it('resizes the selected shape by its handle and nudges it with arrow keys', async () => {
    const viewport = await openBig();
    await fireEvent.keyDown(window, { key: 'r' });
    await drag(viewport, [220, 220], [300, 300]);
    // Drag the bottom-right handle further out, then check the outline moved there.
    await drag(viewport, [300, 300], [360, 340]);
    await fireEvent.keyDown(window, { key: 'v' });
    await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: 360, clientY: 320 });
    expect(viewport.style.cursor).toBe('move');
    await fireEvent.pointerDown(viewport, { pointerId: 3, button: 0, isPrimary: true, clientX: 360, clientY: 320 });
    await fireEvent.pointerUp(viewport, { pointerId: 3 });
    await fireEvent.keyDown(window, { key: 'ArrowRight', shiftKey: true });
    await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: 370, clientY: 320 });
    expect(viewport.style.cursor).toBe('move');
    await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: 220, clientY: 260 });
    expect(viewport.style.cursor).toBe('default');
  });
  it('undoes and redoes drawing, moving, deleting and nudging', async () => {
    const viewport = await openBig();
    const undo = screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement;
    const redo = screen.getByRole('button', { name: 'Redo' }) as HTMLButtonElement;
    const hoverCursor = async (x: number, y: number) => {
      await fireEvent.pointerMove(viewport, { pointerId: 2, isPrimary: true, clientX: x, clientY: y });
      return viewport.style.cursor;
    };
    expect(undo.disabled && redo.disabled).toBe(true);
    await fireEvent.keyDown(window, { key: 'l' });
    // A click draws nothing, so there is nothing to undo.
    await fireEvent.pointerDown(viewport, { pointerId: 1, button: 0, isPrimary: true, clientX: 250, clientY: 250 });
    await fireEvent.pointerUp(viewport, { pointerId: 1 });
    expect(undo.disabled).toBe(true);
    await drag(viewport, [220, 220], [320, 220]);
    expect(undo.disabled).toBe(false);
    await fireEvent.keyDown(window, { key: 'v' });
    await drag(viewport, [270, 220], [270, 260]);
    expect(await hoverCursor(270, 260)).toBe('move');

    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    expect(await hoverCursor(270, 260)).toBe('default');
    expect(await hoverCursor(270, 220)).toBe('move');
    expect(redo.disabled).toBe(false);
    await fireEvent.click(undo);
    expect(await hoverCursor(270, 220)).toBe('default');
    expect(undo.disabled).toBe(true);

    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true, shiftKey: true });
    expect(await hoverCursor(270, 220)).toBe('move');
    await fireEvent.keyDown(window, { key: 'y', ctrlKey: true });
    expect(await hoverCursor(270, 260)).toBe('move');
    expect(redo.disabled).toBe(true);

    // Held arrow keys are one step, ended by releasing the key.
    await fireEvent.pointerDown(viewport, { pointerId: 3, button: 0, isPrimary: true, clientX: 270, clientY: 260 });
    await fireEvent.pointerUp(viewport, { pointerId: 3 });
    await fireEvent.keyDown(window, { key: 'ArrowDown', shiftKey: true });
    await fireEvent.keyDown(window, { key: 'ArrowDown', shiftKey: true, repeat: true });
    await fireEvent.keyUp(window, { key: 'ArrowDown' });
    expect(await hoverCursor(270, 280)).toBe('move');
    await fireEvent.click(undo);
    expect(await hoverCursor(270, 260)).toBe('move');

    // Delete is undoable; a new edit clears redo.
    await fireEvent.pointerDown(viewport, { pointerId: 4, button: 0, isPrimary: true, clientX: 270, clientY: 260 });
    await fireEvent.pointerUp(viewport, { pointerId: 4 });
    await fireEvent.keyDown(window, { key: 'Delete' });
    expect(await hoverCursor(270, 260)).toBe('default');
    await fireEvent.click(undo);
    expect(await hoverCursor(270, 260)).toBe('move');
    expect(redo.disabled).toBe(false);
    await fireEvent.keyDown(window, { key: 'l' });
    await drag(viewport, [220, 300], [320, 300]);
    expect(redo.disabled).toBe(true);
  });
  it('starts a new history for each opened image', async () => {
    const viewport = await openBig();
    await drag(viewport, [220, 220], [320, 260]);
    expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(false);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Discard' })); // unsaved annotations: M4-5 prompt
    await waitFor(() => expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(true));
  });
  it('pans with Space held instead of drawing', async () => {
    const viewport = await openBig();
    await fireEvent.keyDown(window, { key: ' ' });
    expect(viewport.style.cursor).toBe('grab');
    const before = viewTransform();
    await drag(viewport, [250, 250], [300, 280]);
    expect(viewTransform()).not.toEqual(before);
    await fireEvent.keyUp(window, { key: ' ' });
    expect(viewport.style.cursor).toBe('crosshair');
  });
});

describe('native file drops', () => {
  it('consumes native drops without accepting a frontend path and removes its listener', async () => {
    const stop = vi.fn();
    vi.mocked(listen).mockResolvedValue(stop);
    vi.mocked(bridge.droppedImage).mockResolvedValue(image('dropped.png'));
    const mounted = render(App);
    await waitFor(() => expect(listen).toHaveBeenCalledWith('shell-file-drop', expect.any(Function)));
    const handler = vi.mocked(listen).mock.calls.find(call => call[0] === 'shell-file-drop')![1];
    handler({ event: 'shell-file-drop', id: 0, payload: null });
    await waitFor(() => expect(screen.getByLabelText('dropped.png, 2 by 1 pixels')).toBeTruthy());
    expect(bridge.droppedImage).toHaveBeenCalledWith();
    mounted.unmount();
    expect(stop).toHaveBeenCalledTimes(4); // menu, drop, capture-ready, capture-save-failed
  });
  it('keeps the current image when a native drop is rejected', async () => {
    vi.mocked(bridge.openImage).mockResolvedValue(image());
    vi.mocked(bridge.droppedImage).mockRejectedValue(new Error('Drop one image at a time.'));
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    vi.mocked(listen).mock.calls.find(call => call[0] === 'shell-file-drop')![1]({ event: 'shell-file-drop', id: 0, payload: null });
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Drop one image at a time.'));
    expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy();
  });
});

describe('file output with the real editing core', () => {
  async function openAndDraw() {
    vi.mocked(bridge.openImage).mockResolvedValue(image());
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    const viewport = screen.getByRole('region', { name: 'Image viewport' });
    Object.assign(viewport, { setPointerCapture: vi.fn(), hasPointerCapture: () => true, releasePointerCapture: vi.fn() });
    await fireEvent.pointerDown(viewport, { pointerId: 10, button: 0, isPrimary: true, clientX: 400, clientY: 300 });
    await fireEvent.pointerMove(viewport, { pointerId: 10, isPrimary: true, clientX: 450, clientY: 320 });
    await fireEvent.pointerUp(viewport, { pointerId: 10 });
  }
  it('blocks raster transforms while new annotations are present', async () => {
    await openAndDraw();
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.keyDown(window, { key: 'Delete' });
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(false);
  });
  it('blocks raster transforms for an imported editable image', async () => {
    vi.mocked(bridge.openImage).mockResolvedValue({ ...image(), editable: true });
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
  });
  it('sends current editable annotations for copy, save, save-as and native drag', async () => {
    vi.mocked(bridge.saveImage).mockResolvedValue('saved.png');
    await openAndDraw();
    await fireEvent.keyDown(window, { key: 'c', ctrlKey: true });
    await waitFor(() => expect(bridge.copyImage).toHaveBeenCalledOnce());
    const document = vi.mocked(bridge.copyImage).mock.calls[0][1];
    expect(JSON.parse(document).elements).toHaveLength(1);
    expect(JSON.parse(document).elements[0].kind).toBe('arrow');
    await waitFor(() => expect(screen.getByRole('status').textContent).toBe('Image copied'));
    await fireEvent.keyDown(window, { key: 's', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(screen.getByLabelText('saved.png, 2 by 1 pixels')).toBeTruthy());
    expect(bridge.saveImage).toHaveBeenCalledWith(image(), document, true, false);
    const handle = screen.getByRole('button', { name: 'Drag image out; Enter copies the image' });
    await fireEvent.pointerDown(handle, { button: 0 });
    await waitFor(() => expect(bridge.dragImage).toHaveBeenCalledWith(image('saved.png'), document, 27));
    await fireEvent.pointerUp(window);
    expect(bridge.cancelDrag).toHaveBeenCalledWith(27);
  });
  it('cancels a released pointer while Rust is still allocating a gesture ID', async () => {
    let allocated!: (id: number) => void;
    vi.mocked(bridge.beginDrag).mockReturnValue(new Promise(resolve => { allocated = resolve; }));
    await openAndDraw();
    await fireEvent.pointerDown(screen.getByRole('button', { name: 'Drag image out; Enter copies the image' }), { button: 0 });
    await waitFor(() => expect(bridge.beginDrag).toHaveBeenCalledOnce());
    await fireEvent.pointerUp(window);
    allocated(81);
    await waitFor(() => expect(bridge.cancelDrag).toHaveBeenCalledWith(81));
    expect(bridge.dragImage).not.toHaveBeenCalled();
  });
  it('preserves the image and annotations after cancelled saves and output failures', async () => {
    vi.mocked(bridge.saveImage).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error('Disk full'));
    await openAndDraw();
    await fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    await waitFor(() => expect(bridge.saveImage).toHaveBeenCalledOnce());
    await waitFor(() => expect((screen.getByRole('button', { name: 'Open image' }) as HTMLButtonElement).disabled).toBe(false));
    expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy();
    await fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Disk full'));
    expect(JSON.parse(vi.mocked(bridge.saveImage).mock.calls[1][1]).elements).toHaveLength(1);
    expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy();
  });
});

describe('capture from the editor', () => {
  const PROMPT = ['Save…', 'Discard', 'Cancel'];
  async function openImage() {
    vi.mocked(bridge.openImage).mockResolvedValueOnce(image());
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    const viewport = screen.getByRole('region', { name: 'Image viewport' });
    Object.assign(viewport, { setPointerCapture: vi.fn(), hasPointerCapture: () => true, releasePointerCapture: vi.fn() });
    return viewport;
  }
  async function draw(viewport: HTMLElement) {
    await fireEvent.pointerDown(viewport, { pointerId: 10, button: 0, isPrimary: true, clientX: 400, clientY: 300 });
    await fireEvent.pointerMove(viewport, { pointerId: 10, isPrimary: true, clientX: 450, clientY: 320 });
    await fireEvent.pointerUp(viewport, { pointerId: 10 });
  }
  const promptButtons = () => Array.from(screen.getByRole('alertdialog').querySelectorAll('button')).map(b => b.textContent?.trim());
  const clickCapture = () => fireEvent.click(screen.getByRole('button', { name: 'Capture' }));
  const dirtyDoc = async () => { render(App); const viewport = await openImage(); await draw(viewport); };

  it('captures without asking when nothing is unsaved', async () => {
    render(App);
    await clickCapture();
    await waitFor(() => expect(capture.captureImage).toHaveBeenCalledWith('region'));
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
  it('captures an untouched open image without asking', async () => {
    render(App);
    await openImage();
    await clickCapture();
    await waitFor(() => expect(capture.captureImage).toHaveBeenCalledWith('region'));
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
  it('asks Save/Discard/Cancel before replacing unsaved annotations', async () => {
    await dirtyDoc();
    await clickCapture();
    expect(promptButtons()).toEqual(PROMPT);
    expect(capture.captureImage).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(capture.captureImage).not.toHaveBeenCalled();
    // the annotation is still there: undo is still possible
    expect((screen.getByRole('button', { name: 'Undo' }) as HTMLButtonElement).disabled).toBe(false);
    await clickCapture();
    await fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
    await waitFor(() => expect(capture.captureImage).toHaveBeenCalledWith('region'));
  });
  it('asks before a timer capture too, and only then starts the countdown', async () => {
    await dirtyDoc();
    await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Timer' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: '5 s' }));
    expect(capture.captureTimer).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
    await waitFor(() => expect(capture.captureTimer).toHaveBeenCalledWith(5));
  });
  it('Save… captures only after a successful save', async () => {
    vi.mocked(bridge.saveImage).mockResolvedValueOnce(null).mockResolvedValueOnce('test.png');
    await dirtyDoc();
    await clickCapture();
    await fireEvent.click(screen.getByRole('button', { name: 'Save…' }));
    await waitFor(() => expect(bridge.saveImage).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(capture.captureImage).not.toHaveBeenCalled();
    await clickCapture();
    await fireEvent.click(screen.getByRole('button', { name: 'Save…' }));
    await waitFor(() => expect(capture.captureImage).toHaveBeenCalledWith('region'));
    expect(bridge.saveImage).toHaveBeenCalledTimes(2);
    // saved work no longer asks
    await clickCapture();
    await waitFor(() => expect(capture.captureImage).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
  it('does not capture when Save… fails', async () => {
    vi.mocked(bridge.saveImage).mockRejectedValueOnce(new Error('Disk full'));
    await dirtyDoc();
    await clickCapture();
    await fireEvent.click(screen.getByRole('button', { name: 'Save…' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Disk full'));
    expect(capture.captureImage).not.toHaveBeenCalled();
  });
  it('the same prompt guards Open, Paste and Drop', async () => {
    await dirtyDoc();
    const pasted = image('pasted.png');
    vi.mocked(bridge.pasteImage).mockResolvedValue(pasted);
    vi.mocked(bridge.droppedImage).mockResolvedValue(image('dropped.png'));
    vi.mocked(bridge.openImage).mockResolvedValue(image('opened.png'));
    const cases: [string, () => unknown, () => unknown][] = [
      ['open', () => fireEvent.click(screen.getByRole('button', { name: 'Open image' })), () => bridge.openImage],
      ['paste', () => fireEvent.click(screen.getByRole('button', { name: 'Paste image' })), () => bridge.pasteImage],
      ['native drop', () => vi.mocked(listen).mock.calls.find(call => call[0] === 'shell-file-drop')![1]({ event: 'shell-file-drop', id: 0, payload: null }), () => bridge.droppedImage],
    ];
    vi.mocked(bridge.openImage).mockClear();
    for (const [, act, fn] of cases) {
      await act();
      expect(promptButtons()).toEqual(PROMPT);
      expect(vi.mocked(fn() as () => unknown)).not.toHaveBeenCalled();
      await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
      expect(vi.mocked(fn() as () => unknown)).not.toHaveBeenCalled();
    }
    await cases[1][1]();
    await fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
    await waitFor(() => expect(screen.getByLabelText('pasted.png, 2 by 1 pixels')).toBeTruthy());
  });
  it('becomes clean again after undoing back to the saved state', async () => {
    vi.mocked(bridge.saveImage).mockResolvedValue('test.png');
    render(App);
    const viewport = await openImage();
    await draw(viewport);
    await fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    await waitFor(() => expect(bridge.saveImage).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('Saved'));
    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    await clickCapture();
    expect(screen.getByRole('alertdialog')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await fireEvent.keyDown(window, { key: 'z', ctrlKey: true, shiftKey: true });
    await clickCapture();
    await waitFor(() => expect(capture.captureImage).toHaveBeenCalledOnce());
  });
  it('Export flat does not count as saving', async () => {
    vi.mocked(bridge.saveImage).mockResolvedValue('flat.png');
    await dirtyDoc();
    await fireEvent.click(screen.getByText('Share'));
    await fireEvent.click(screen.getByRole('button', { name: 'Export flat…' }));
    await waitFor(() => expect(bridge.saveImage).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('Exported'));
    await clickCapture();
    expect(screen.getByRole('alertdialog')).toBeTruthy();
  });
  it('loads the capture when capture-ready arrives', async () => {
    vi.mocked(capture.captureResult).mockResolvedValue(image('Screenshot 2026-11-25 at 10.00.00.png'));
    render(App);
    await waitFor(() => expect(listen).toHaveBeenCalledWith('capture-ready', expect.any(Function)));
    const handler = vi.mocked(listen).mock.calls.find(call => call[0] === 'capture-ready')![1];
    handler({ event: 'capture-ready', id: 0, payload: null });
    await waitFor(() => expect(screen.getByLabelText('Screenshot 2026-11-25 at 10.00.00.png, 2 by 1 pixels')).toBeTruthy());
    expect(capture.captureResult).toHaveBeenCalledOnce();
  });
  it('shows an immediate capture result (Full screen) as the open image', async () => {
    vi.mocked(capture.captureImage).mockResolvedValue(image('Screenshot.png'));
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Full screen' }));
    await waitFor(() => expect(capture.captureImage).toHaveBeenCalledWith('screen'));
    await waitFor(() => expect(screen.getByLabelText('Screenshot.png, 2 by 1 pixels')).toBeTruthy());
  });
  it('keeps the image and shows a backend or permission error', async () => {
    vi.mocked(capture.captureImage).mockRejectedValue(new Error('Screen capture needs permission. Allow ScribbleCap in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.'));
    render(App);
    await openImage();
    await clickCapture();
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('System Settings → Privacy & Security → Screen & System Audio Recording'));
    expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy();
    vi.mocked(capture.captureImage).mockRejectedValue('A capture is already in progress.');
    await clickCapture();
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('A capture is already in progress.'));
  });
  it('shows a failed capture auto-save while the capture stays open', async () => {
    render(App);
    await waitFor(() => expect(listen).toHaveBeenCalledWith('capture-save-failed', expect.any(Function)));
    const handler = vi.mocked(listen).mock.calls.find(call => call[0] === 'capture-save-failed')![1];
    handler({ event: 'capture-save-failed', id: 0, payload: 'The capture folder is full.' });
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('The capture folder is full.'));
  });
  it('opens the settings panel from the menu', async () => {
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Settings…' }));
    expect(screen.getByRole('dialog', { name: 'Settings' })).toBeTruthy();
    expect((await screen.findByRole('switch', { name: 'Auto-save captures' }) as HTMLInputElement).checked).toBe(false);
  });
});
