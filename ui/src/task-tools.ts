/** Own the task region's connection and focus boundary. Its single set of input
 * nodes is parked off-document between uses so existing form values survive. */
export function mountTaskTools(region: HTMLElement, editor: HTMLElement, close: () => void) {
  const anchor = document.createComment("task-tools-mount");
  region.before(anchor);
  const parked = document.createDocumentFragment();
  parked.append(region);
  const events = new AbortController();
  let returnFocus: HTMLElement | null = null;
  const unmount = () => {
    const overlay = region.classList.contains("editor-task-tools");
    region.classList.add("hidden");
    region.classList.remove("editor-task-tools");
    region.removeAttribute("role");
    editor.inert = false;
    parked.append(region);
    if (overlay && returnFocus?.isConnected) returnFocus.focus();
    returnFocus = null;
  };
  const mount = () => {
    if (!region.isConnected) anchor.after(region);
    region.classList.remove("hidden");
  };
  region.addEventListener("keydown", event => {
    if (!region.classList.contains("editor-task-tools")) return;
    if (event.key === "Escape") { event.preventDefault(); close(); }
    if (event.key !== "Tab") return;
    const controls = [...region.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex="0"]')].filter(item => item.getClientRects().length);
    const first = controls[0], last = controls.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }, { signal: events.signal });
  return {
    mountPage() { mount(); },
    mountOverlay() {
      returnFocus = document.activeElement as HTMLElement | null;
      mount();
      // Keep a named region: WKWebView modal filtering hides reused controls.
      region.classList.add("editor-task-tools");
      region.setAttribute("role", "region");
      editor.inert = true;
    },
    unmount,
    dispose() { unmount(); events.abort(); anchor.remove(); },
  };
}

/** Keep processing controls out of the document until explicitly opened. */
export function mountToolDialog(dialog: HTMLDialogElement, hidden: () => void) {
  const anchor = document.createComment("processing-tool-mount");
  dialog.before(anchor);
  const parked = document.createDocumentFragment();
  parked.append(dialog);
  const events = new AbortController();
  const unmount = () => { if (!dialog.open) { parked.append(dialog); hidden(); } };
  dialog.addEventListener("close", unmount, { signal: events.signal });
  return {
    open() { if (!dialog.isConnected) anchor.after(dialog); if (!dialog.open) dialog.showModal(); },
    close() { dialog.close(); unmount(); },
    dispose() { events.abort(); dialog.close(); parked.append(dialog); hidden(); anchor.remove(); },
  };
}
