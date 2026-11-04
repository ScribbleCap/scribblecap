import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import App from './App.svelte';
import * as bridge from './lib/bridge';
import type { LoadedImage } from './lib/image-packet';

vi.mock('./lib/bridge', () => ({ desktop: true, openImage: vi.fn(), pasteImage: vi.fn(), browserImage: vi.fn() }));

function image(name = 'test.png'): LoadedImage { return { name, width: 2, height: 1, pixels: new Uint8ClampedArray([255, 0, 0, 255, 0, 0, 255, 255]) }; }
const transform = vi.fn();
// Each redraw sets the image transform, then a screen-space transform for the image edge.
const viewTransform = () => transform.mock.calls.at(-2);
let resize: ResizeObserverCallback | undefined;
beforeEach(() => {
  vi.mocked(bridge.openImage).mockReset(); vi.mocked(bridge.pasteImage).mockReset(); transform.mockReset();
  resize = undefined;
  vi.stubGlobal('ResizeObserver', class { constructor(callback: ResizeObserverCallback) { resize = callback; } observe() {} disconnect() {} });
  vi.stubGlobal('ImageData', class { constructor(public data: Uint8ClampedArray, public width: number, public height: number) {} });
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ putImageData: vi.fn(), setTransform: transform, clearRect: vi.fn(), fillRect: vi.fn(), drawImage: vi.fn(), createPattern: vi.fn(), strokeRect: vi.fn() } as unknown as CanvasRenderingContext2D);
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
    expect(screen.getByRole('status').textContent).toContain('Loading');
    resolve(null);
    await waitFor(() => expect(screen.queryByRole('status')).toBeNull());
  });
  it('pans by dragging, releases pointer capture and handles wheel zoom', async () => {
    vi.mocked(bridge.openImage).mockResolvedValue(image());
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    await waitFor(() => expect(screen.getByLabelText('test.png, 2 by 1 pixels')).toBeTruthy());
    const viewport = screen.getByRole('region', { name: 'Image viewport' });
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
