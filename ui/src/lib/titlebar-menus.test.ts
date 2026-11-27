import { afterEach, expect, it } from 'vitest';
import { titlebarMenus } from './titlebar-menus';
let destroy: (() => void) | undefined;
afterEach(() => { destroy?.(); document.body.innerHTML = ''; });
function setup() {
  document.body.innerHTML = '<header><details id="style"><summary>Style</summary><input /></details><details id="more"><summary>More</summary><details id="nested"><summary>Numeric fields</summary><input /></details></details><details class="share-actions"><summary>Share</summary><button>Save</button></details></header><main>Outside</main>';
  const header = document.querySelector('header')!; destroy = titlebarMenus(header).destroy;
  return { style: document.querySelector<HTMLDetailsElement>('#style')!, more: document.querySelector<HTMLDetailsElement>('#more')!, nested: document.querySelector<HTMLDetailsElement>('#nested')!, share: document.querySelector<HTMLDetailsElement>('.share-actions')! };
}
it('closes outside menus but leaves fields and nested menus open on inside interaction', () => {
  const { more,nested } = setup(); more.open = true; nested.open = true;
  nested.querySelector('input')!.dispatchEvent(new Event('pointerdown',{bubbles:true}));
  expect(more.open && nested.open).toBe(true);
  document.querySelector('main')!.dispatchEvent(new Event('pointerdown',{bubbles:true}));
  expect(more.open || nested.open).toBe(false);
});
it('allows one top-level menu and keeps a nested menu from closing its parent', () => {
  const { style,more,nested } = setup(); style.open = true; more.open = true;
  more.dispatchEvent(new Event('toggle')); expect(style.open).toBe(false);
  nested.open = true; nested.dispatchEvent(new Event('toggle')); expect(more.open).toBe(true);
});
it('closes on Escape and returns focus to the trigger', () => {
  const { style } = setup(); style.open = true;
  style.querySelector('input')!.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
  expect(style.open).toBe(false); expect(document.activeElement).toBe(style.querySelector('summary'));
});
it.each([
  '<dialog open><button>Close</button></dialog>',
  '<div role="dialog" aria-modal="true"><button>Close</button></div>',
  '<div role="alertdialog" aria-modal="true"><button>Cancel</button></div>',
])('leaves Escape to the open dialog: %s', markup => {
  const { style } = setup(); style.open = true;
  document.body.insertAdjacentHTML('beforeend', markup);
  const button = document.body.lastElementChild!.querySelector('button')!;
  button.focus();
  let reachedDialog = false;
  button.addEventListener('keydown', () => { reachedDialog = true; });
  const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
  button.dispatchEvent(event);
  expect(reachedDialog).toBe(true);
  expect(event.defaultPrevented).toBe(false);
  expect(style.open).toBe(true);
  expect(document.activeElement).toBe(button);
});
it('still closes menus when a native dialog is closed', () => {
  const { style } = setup(); style.open = true;
  document.body.insertAdjacentHTML('beforeend', '<dialog><button>Close</button></dialog>');
  style.querySelector('input')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  expect(style.open).toBe(false);
});
it('closes Share after choosing an action', () => {
  const { share } = setup(); share.open = true; share.querySelector('button')!.click(); expect(share.open).toBe(false);
});
