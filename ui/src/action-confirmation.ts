import { mountToolDialog } from "./task-tools";

export type ActionConfirmation = { title: string; message: string; confirmLabel: string; danger?: boolean };

/** One shared confirmation view. Nested confirmations remain inside the active
 * native modal; otherwise WKWebView would make this region inert behind it. */
export function mountActionConfirmation(dialog: HTMLDialogElement, compact: () => boolean) {
  const title = dialog.querySelector<HTMLElement>("#confirmation-title")!;
  const message = dialog.querySelector<HTMLElement>("#confirmation-message")!;
  const accept = dialog.querySelector<HTMLButtonElement>("#accept-confirmation")!;
  const cancel = dialog.querySelector<HTMLButtonElement>("#cancel-confirmation")!;
  const form = dialog.querySelector("form")!;
  const events = new AbortController();
  let disposed = false, resolve: ((value: boolean) => void) | null = null;
  const cancelled = () => { const pending = resolve; resolve = null; pending?.(false); };
  const boundary = mountToolDialog(dialog, cancelled);
  const finish = (value: boolean) => {
    const pending = resolve; resolve = null;
    boundary.close(true); pending?.(value);
  };
  dialog.classList.add("action-confirmation");
  form.addEventListener("submit", event => { event.preventDefault(); finish(true); }, { signal: events.signal });
  cancel.addEventListener("click", () => finish(false), { signal: events.signal });
  dialog.addEventListener("cancel", event => { event.preventDefault(); finish(false); }, { signal: events.signal });
  return {
    ask(value: ActionConfirmation): Promise<boolean> {
      if (disposed) return Promise.resolve(false);
      finish(false);
      title.textContent = value.title; message.textContent = value.message; accept.textContent = value.confirmLabel;
      dialog.setAttribute("aria-label", value.title);
      accept.classList.toggle("danger", value.danger ?? false);
      dialog.classList.toggle("editor-confirmation", compact());
      const focusedModal = document.activeElement?.closest("dialog:modal");
      const host = focusedModal instanceof HTMLDialogElement ? focusedModal : [...document.querySelectorAll<HTMLDialogElement>("dialog:modal")].at(-1);
      return new Promise(done => { resolve = done; boundary.open(host); cancel.focus(); });
    },
    dispose() { if (disposed) return; disposed = true; finish(false); events.abort(); boundary.dispose(); dialog.remove(); },
  };
}
