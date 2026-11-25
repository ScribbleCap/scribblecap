import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, expect, it, vi } from 'vitest';
import CaptureMenu from './CaptureMenu.svelte';
import { captureState } from './capture';

vi.mock('./capture', () => ({ captureState: vi.fn() }));
const WAYLAND = 'Not available on Wayland: the system dialog chooses what to capture.';
const props = () => ({ busy: false, capture: vi.fn(), timer: vi.fn(), settings: vi.fn() });
beforeEach(() => { vi.mocked(captureState).mockReset().mockResolvedValue({ wayland: false, previous: true }); });

it('the main button captures a region', async () => {
  const p = props();
  render(CaptureMenu, p);
  await fireEvent.click(screen.getByRole('button', { name: 'Capture' }));
  expect(p.capture).toHaveBeenCalledWith('region');
});
it('lists the modes, timer and settings in the menu', async () => {
  const p = props();
  render(CaptureMenu, p);
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  await waitFor(() => expect((screen.getByRole('menuitem', { name: 'Previous region' }) as HTMLButtonElement).disabled).toBe(false));
  await fireEvent.click(screen.getByRole('menuitem', { name: 'Window' }));
  expect(p.capture).toHaveBeenCalledWith('window');
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  await fireEvent.click(screen.getByRole('menuitem', { name: 'Full screen' }));
  expect(p.capture).toHaveBeenCalledWith('screen');
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  await fireEvent.click(screen.getByRole('menuitem', { name: 'Previous region' }));
  expect(p.capture).toHaveBeenCalledWith('previous');
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  await fireEvent.click(screen.getByRole('menuitem', { name: 'Settings…' }));
  expect(p.settings).toHaveBeenCalledOnce();
});
it('offers 3, 5 and 10 second timers', async () => {
  const p = props();
  render(CaptureMenu, p);
  for (const seconds of [3, 5, 10]) {
    await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Timer' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: `${seconds} s` }));
    expect(p.timer).toHaveBeenLastCalledWith(seconds);
  }
});
it('disables Previous region until one was captured', async () => {
  vi.mocked(captureState).mockResolvedValue({ wayland: false, previous: false });
  render(CaptureMenu, props());
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  await waitFor(() => expect(captureState).toHaveBeenCalled());
  expect((screen.getByRole('menuitem', { name: 'Previous region' }) as HTMLButtonElement).disabled).toBe(true);
});
it('disables Window and Previous region on Wayland with the reason', async () => {
  vi.mocked(captureState).mockResolvedValue({ wayland: true, previous: true });
  render(CaptureMenu, props());
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  await waitFor(() => expect((screen.getByRole('menuitem', { name: 'Window' }) as HTMLButtonElement).disabled).toBe(true));
  expect(screen.getByRole('menuitem', { name: 'Window' }).getAttribute('title')).toBe(WAYLAND);
  const previous = screen.getByRole('menuitem', { name: 'Previous region' }) as HTMLButtonElement;
  expect(previous.disabled).toBe(true);
  expect(previous.getAttribute('title')).toBe(WAYLAND);
  expect((screen.getByRole('menuitem', { name: 'Full screen' }) as HTMLButtonElement).disabled).toBe(false);
});
it('is disabled while busy and closes on Escape', async () => {
  const p = props();
  const view = render(CaptureMenu, p);
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  expect(screen.getByRole('menu')).toBeTruthy();
  await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
  expect(screen.queryByRole('menu')).toBeNull();
  await view.rerender({ ...p, busy: true });
  expect((screen.getByRole('button', { name: 'Capture' }) as HTMLButtonElement).disabled).toBe(true);
});
it('focuses the first enabled item when opened and explains a disabled Previous region', async () => {
  vi.mocked(captureState).mockResolvedValue({ wayland: false, previous: false });
  render(CaptureMenu, props());
  await fireEvent.click(screen.getByRole('button', { name: 'Capture options' }));
  await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Window' })));
  expect(screen.getByRole('menuitem', { name: 'Previous region' }).getAttribute('title')).toBe('Capture a region first');
});
