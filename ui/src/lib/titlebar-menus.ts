/** Coordinate top-level native details menus without interfering with form fields. */
export function titlebarMenus(header: HTMLElement) {
  const roots = () => [...header.querySelectorAll<HTMLDetailsElement>('details')].filter(menu => !menu.parentElement?.closest('details'));
  const close = (menu: HTMLDetailsElement) => {
    menu.open = false;
    menu.querySelectorAll<HTMLDetailsElement>('details[open]').forEach(nested => { nested.open = false; });
  };
  const outside = (event: PointerEvent) => { for (const menu of roots()) if (!menu.contains(event.target as Node)) close(menu); };
  const toggle = (event: Event) => {
    const menu = event.target;
    if (menu instanceof HTMLDetailsElement && menu.open && roots().includes(menu)) for (const other of roots()) if (other !== menu) close(other);
  };
  const escape = (event: KeyboardEvent) => {
    if (event.key !== 'Escape') return;
    // Native dialogs and our ARIA modals own Escape and their focus restoration.
    if (document.querySelector('dialog[open], [aria-modal="true"]')) return;
    const menu = roots().find(root => root.open);
    if (menu) { close(menu); menu.querySelector('summary')?.focus(); event.preventDefault(); event.stopPropagation(); }
  };
  const action = (event: MouseEvent) => {
    if (event.target instanceof Element && event.target.closest('.share-actions button')) {
      const menu = event.target.closest<HTMLDetailsElement>('.share-actions'); if (menu) close(menu);
    }
  };
  document.addEventListener('pointerdown', outside, true);
  document.addEventListener('keydown', escape, true);
  header.addEventListener('toggle', toggle, true);
  header.addEventListener('click', action);
  return { destroy() {
    document.removeEventListener('pointerdown', outside, true);
    document.removeEventListener('keydown', escape, true);
    header.removeEventListener('toggle', toggle, true);
    header.removeEventListener('click', action);
  } };
}
