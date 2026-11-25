import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import Overlay from './Overlay.svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));

function packet(width: number, height: number) {
  const data = new ArrayBuffer(20 + width * height * 4);
  const header = new DataView(data);
  new Uint8Array(data).set([83, 67, 73, 77]);
  [1, width, height, 0].forEach((v, i) => header.setUint32(4 + i * 4, v, true));
  return data;
}
const win = (x: number, y: number, width: number, height: number) => ({ id: 1, title: 't', appName: 'a', bounds: { x, y, width, height } });
function setup(mode: string, opts: { fail?: string } = {}) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === opts.fail) throw new Error('boom');
    if (cmd === 'shell_overlay_info') return { bounds: { x: 1920, y: 0, width: 200, height: 100 }, mode, windows: [win(1930, 10, 60, 40), win(1920, 0, 200, 100)] };
    if (cmd === 'shell_overlay_frame') return packet(400, 200);
    return undefined;
  });
  return render(Overlay, { display: 7 });
}
const surface = () => screen.findByTestId('overlay');
const calls = (name: string) => vi.mocked(invoke).mock.calls.filter(([c]) => c === name);
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null); // jsdom has no canvas
});

it('asks for info and frame on load', async () => {
  setup('region'); await surface();
  expect(invoke).toHaveBeenCalledWith('shell_overlay_info', { display: 7 });
  expect(invoke).toHaveBeenCalledWith('shell_overlay_frame', { display: 7 });
});
it('a click without a drag in region mode keeps the overlay open', async () => {
  setup('region'); const s = await surface();
  await fireEvent.mouseDown(s, { clientX: 20, clientY: 20, button: 0 });
  await fireEvent.mouseUp(s, { clientX: 21, clientY: 21, button: 0 });
  expect(calls('shell_overlay_select')).toHaveLength(0);
  expect(calls('shell_capture_cancel')).toHaveLength(0);
});
it('a region drag selects desktop points on this display', async () => {
  setup('region'); const s = await surface();
  await fireEvent.mouseDown(s, { clientX: 10, clientY: 10, button: 0 });
  await fireEvent.mouseMove(s, { clientX: 60, clientY: 40 });
  expect(await screen.findByText('100 × 60')).toBeTruthy(); // image pixels at 2x
  await fireEvent.mouseUp(s, { clientX: 110, clientY: 60, button: 0 });
  expect(invoke).toHaveBeenCalledWith('shell_overlay_select', { display: 7, rect: { x: 1930, y: 10, width: 100, height: 50 } });
});
it('window mode selects the hovered window bounds', async () => {
  setup('window'); const s = await surface();
  await fireEvent.mouseMove(s, { clientX: 20, clientY: 20 }); // desktop 1940,20 -> window 0
  await fireEvent.mouseDown(s, { clientX: 20, clientY: 20, button: 0 });
  await fireEvent.mouseUp(s, { clientX: 20, clientY: 20, button: 0 });
  expect(invoke).toHaveBeenCalledWith('shell_overlay_select', { display: 7, rect: { x: 1930, y: 10, width: 60, height: 40 } });
});
it('Escape cancels', async () => {
  setup('region'); await surface();
  await fireEvent.keyDown(window, { key: 'Escape' });
  expect(calls('shell_capture_cancel')).toHaveLength(1);
});
it('right click cancels', async () => {
  setup('region'); const s = await surface();
  await fireEvent.contextMenu(s);
  expect(calls('shell_capture_cancel')).toHaveLength(1);
});
it.each(['shell_overlay_info', 'shell_overlay_frame'])('cancels the capture when %s fails', async (fail) => {
  setup('region', { fail });
  await vi.waitFor(() => expect(calls('shell_capture_cancel')).toHaveLength(1));
});
it('cancels the capture when the selection fails', async () => {
  setup('region', { fail: 'shell_overlay_select' }); const s = await surface();
  await fireEvent.mouseDown(s, { clientX: 10, clientY: 10, button: 0 });
  await fireEvent.mouseUp(s, { clientX: 110, clientY: 60, button: 0 });
  await vi.waitFor(() => expect(calls('shell_capture_cancel')).toHaveLength(1));
});
