import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import Countdown from './Countdown.svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }));
beforeEach(() => { vi.useFakeTimers(); vi.mocked(invoke).mockReset(); vi.mocked(invoke).mockResolvedValue(undefined); });
afterEach(() => vi.useRealTimers());

it('counts down whole seconds and cancels on click', async () => {
  render(Countdown, { seconds: 5 });
  expect(screen.getByTestId('count').textContent).toBe('5');
  await vi.advanceTimersByTimeAsync(1000);
  expect(screen.getByTestId('count').textContent).toBe('4');
  await vi.advanceTimersByTimeAsync(10_000);
  expect(screen.getByTestId('count').textContent).toBe('0');
  await fireEvent.click(screen.getByRole('button'));
  expect(invoke).toHaveBeenCalledWith('shell_capture_cancel');
});
it('Escape cancels', async () => {
  render(Countdown, { seconds: 3 });
  await fireEvent.keyDown(window, { key: 'Escape' });
  expect(invoke).toHaveBeenCalledWith('shell_capture_cancel');
});
