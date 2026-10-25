function focusableControls(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), summary, a[href], [tabindex="0"]')].filter(node => {
    if (!node.getClientRects().length || getComputedStyle(node).visibility !== "visible") return false;
    // WebKit can retain rectangles for descendants of a closed disclosure.
    for (let parent = node.parentElement; parent && root.contains(parent); parent = parent.parentElement) {
      if (parent instanceof HTMLDetailsElement && !parent.open && !parent.querySelector(":scope > summary")?.contains(node)) return false;
    }
    return true;
  });
}

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
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
    if (event.key !== "Tab") return;
    const controls = focusableControls(region);
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
export function mountToolDialog(dialog: HTMLDialogElement, hidden: () => void, canClose: () => boolean = () => true) {
  const anchor = document.createComment("processing-tool-mount");
  dialog.before(anchor);
  const parked = document.createDocumentFragment();
  parked.append(dialog);
  const events = new AbortController();
  let background: { node: HTMLElement; inert: boolean }[] = [];
  let mounted = false, previousInert = false;
  let returnFocus: HTMLElement | null = null;
  dialog.setAttribute("role", "region");
  dialog.setAttribute("aria-label", dialog.querySelector("h2")?.textContent ?? "任务工具");
  const unmount = () => {
    if (dialog.open || !mounted) return;
    mounted = false;
    dialog.inert = previousInert;
    for (const item of background) item.node.inert = item.inert;
    background = [];
    parked.append(dialog); hidden();
    if (returnFocus?.isConnected) returnFocus.focus();
    returnFocus = null;
  };
  const close = (force = false) => { if (!force && !canClose()) return; dialog.close(); unmount(); };
  dialog.addEventListener("close", unmount, { signal: events.signal });
  dialog.addEventListener("keydown", event => {
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
    if (event.key !== "Tab") return;
    const controls = focusableControls(dialog);
    const first = controls[0], last = controls.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }, { signal: events.signal });
  return {
    open(host?: HTMLElement) {
      if (dialog.open) return;
      previousInert = dialog.inert; dialog.inert = false; mounted = true;
      returnFocus = document.activeElement as HTMLElement | null;
      if (host) host.append(dialog);
      else if (!dialog.isConnected) anchor.after(dialog);
      let branch: HTMLElement = dialog;
      while (branch.parentElement && branch !== document.body) {
        for (const sibling of branch.parentElement.children) if (sibling !== branch && sibling instanceof HTMLElement) {
          background.push({ node: sibling, inert: sibling.inert }); sibling.inert = true;
        }
        branch = branch.parentElement;
      }
      // WKWebView hides controls in a dynamically mounted modal dialog. A
      // nonmodal named region plus explicit inert/focus management keeps AX live.
      dialog.show();
      dialog.querySelector<HTMLButtonElement>("button")?.focus();
    },
    close,
    dispose() { events.abort(); close(true); anchor.remove(); },
  };
}
