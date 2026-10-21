/** Native disclosure menus keep ordinary Tab navigation and have one lifecycle. */
export function mountEditorMenus(root: HTMLElement): { dispose(): void } {
  const events = new AbortController();
  const menus = [...root.querySelectorAll<HTMLDetailsElement>(".editor-menu")];
  const close = (menu: HTMLDetailsElement, restoreFocus = false) => {
    menu.open = false;
    if (restoreFocus) menu.querySelector<HTMLElement>("summary")?.focus();
  };
  document.addEventListener("pointerdown", event => {
    for (const menu of menus) if (!menu.contains(event.target as Node)) close(menu);
  }, { signal: events.signal });
  root.addEventListener("keydown", event => {
    const menu = (event.target as HTMLElement).closest<HTMLDetailsElement>(".editor-menu[open]");
    if (event.key === "Escape" && menu) {
      event.preventDefault(); event.stopPropagation(); close(menu, true);
    }
  }, { capture: true, signal: events.signal });
  root.addEventListener("click", event => {
    const button = (event.target as HTMLElement).closest(".editor-menu button");
    const menu = button?.closest<HTMLDetailsElement>(".editor-menu");
    if (menu) close(menu, true);
  }, { signal: events.signal });
  return { dispose() { events.abort(); for (const menu of menus) close(menu); } };
}
