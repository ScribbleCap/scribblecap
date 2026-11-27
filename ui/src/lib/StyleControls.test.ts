import { fireEvent, render, screen } from '@testing-library/svelte';
import { expect, it, vi } from 'vitest';
import StyleControls from './StyleControls.svelte';
it('closes after preset and size choices, keeping a custom colour input open until committed', async () => {
  const { container } = render(StyleControls,{ style:{color:'#e5194a',width:6},disabled:false,change:vi.fn() });
  const menu = container.querySelector('details')!; menu.open = true;
  await fireEvent.click(screen.getByRole('button',{name:'Colour #007aff'})); expect(menu.open).toBe(false);
  menu.open = true; await fireEvent.change(screen.getByLabelText('Stroke size'),{target:{value:'10'}}); expect(menu.open).toBe(false);
  menu.open = true; await fireEvent.input(screen.getByLabelText('Custom colour'),{target:{value:'#123456'}}); expect(menu.open).toBe(true);
  await fireEvent.change(screen.getByLabelText('Custom colour')); expect(menu.open).toBe(false);
});
