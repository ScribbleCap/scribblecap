import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, it, vi } from 'vitest';
import UnsavedPrompt from './UnsavedPrompt.svelte';

it('offers Save…, Discard and Cancel in that order and reports the choice', async () => {
  const choose = vi.fn();
  render(UnsavedPrompt, { choose });
  const dialog = screen.getByRole('alertdialog');
  expect(Array.from(dialog.querySelectorAll('button')).map(b => b.textContent?.trim())).toEqual(['Save…', 'Discard', 'Cancel']);
  await fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
  expect(choose).toHaveBeenCalledWith('discard');
  await fireEvent.click(screen.getByRole('button', { name: 'Save…' }));
  expect(choose).toHaveBeenLastCalledWith('save');
});
it('treats Escape as Cancel', async () => {
  const choose = vi.fn();
  render(UnsavedPrompt, { choose });
  await fireEvent.keyDown(screen.getByRole('alertdialog'), { key: 'Escape' });
  expect(choose).toHaveBeenCalledWith('cancel');
});
