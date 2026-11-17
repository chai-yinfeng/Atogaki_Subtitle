/** Shared sheet interaction; opening or closing a panel never changes playback. */
export function mountSheet(dialog: HTMLDialogElement, triggers: HTMLElement[], closeButton: HTMLElement) {
  const events = new AbortController();
  let opener: HTMLElement | null = null;
  const close = () => dialog.close();
  const open = (trigger: HTMLElement) => {
    opener = trigger;
    if (!dialog.open) dialog.showModal();
    triggers.forEach(button => button.setAttribute("aria-expanded", "true"));
    dialog.querySelector<HTMLElement>('input[type="search"], [autofocus]')?.focus();
  };
  triggers.forEach(button => {
    button.setAttribute("aria-expanded", "false");
    button.addEventListener("click", () => open(button), { signal: events.signal });
  });
  closeButton.addEventListener("click", close, { signal: events.signal });
  dialog.addEventListener("keydown", event => {
    if (event.key !== "Escape") return;
    event.preventDefault(); event.stopPropagation(); close();
  }, { signal: events.signal });
  dialog.addEventListener("close", () => {
    triggers.forEach(button => button.setAttribute("aria-expanded", "false"));
    if (opener?.isConnected && opener.getClientRects().length && !opener.closest('[inert], [hidden], .hidden') && !document.querySelector("dialog[open]")) opener.focus();
  }, { signal: events.signal });
  dialog.addEventListener("click", event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom) close();
  }, { signal: events.signal });
  return { close, dispose() { events.abort(); } };
}

export function mountNavigationToggle(shell: HTMLElement, sidebar: HTMLElement, button: HTMLButtonElement) {
  const key = "atogaki.navigationCollapsed";
  let collapsed = false;
  try { collapsed = localStorage.getItem(key) === "true"; } catch { /* Optional preference. */ }
  const render = () => {
    shell.classList.toggle("navigation-collapsed", collapsed);
    sidebar.inert = collapsed;
    button.textContent = collapsed ? "显示导航栏" : "收起导航栏";
    button.setAttribute("aria-expanded", String(!collapsed));
  };
  const toggle = () => {
    collapsed = !collapsed; render();
    try { localStorage.setItem(key, String(collapsed)); } catch { /* Session preference still applies. */ }
  };
  button.addEventListener("click", toggle); render();
  return { dispose() { button.removeEventListener("click", toggle); } };
}
