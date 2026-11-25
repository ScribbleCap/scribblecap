import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, it, vi } from 'vitest';
import CanvasControls from './CanvasControls.svelte';
import { transformImage } from './canvas-transform';
vi.mock('./canvas-transform', () => ({ transformImage: vi.fn() }));
const image = { name: 'test.png', width: 4, height: 3, pixels: new Uint8ClampedArray(48) };
it('applies numeric crop bounds through the existing load transaction', async () => {
  const load = vi.fn(async (action: () => Promise<unknown>) => { await action(); });
  render(CanvasControls, { image, busy: false, load });
  await fireEvent.input(screen.getByLabelText('Width'), { target: { value: '2' } });
  await fireEvent.input(screen.getByLabelText('Crop left'), { target: { value: '1' } });
  await fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
  expect(load).toHaveBeenCalledOnce();
  expect(transformImage).toHaveBeenCalledWith(image, { operation: 'crop', outputWidth: 2, outputHeight: 3, x: 1, y: 0 });
});
it('resets dimensions for a newly loaded image and blocks editable images', async () => {
  const view = render(CanvasControls, { image, busy: false, load: vi.fn() });
  await view.rerender({ image: { ...image, width: 2, height: 1, editable: true }, busy: false, load: vi.fn() });
  expect((screen.getByLabelText('Width') as HTMLInputElement).value).toBe('2');
  expect((screen.getByLabelText('Height') as HTMLInputElement).value).toBe('1');
  expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
});
