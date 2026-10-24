import type { AsrCredentialAction } from "./asr-credential-session.ts";
import type { createAsrCredentialSession } from "./asr-credential-session.ts";
export function mountAsrCredentialView(root: HTMLDialogElement, session: ReturnType<typeof createAsrCredentialSession>, visit: () => number) {
  const ids: Record<string, AsrCredentialAction> = { "save-gemini-api-key": "save", "clear-gemini-api-key": "delete", "check-gemini-api-key": "check" };
  const buttons = Object.keys(ids).map(id => root.querySelector<HTMLButtonElement>(`#${id}`)!).filter(Boolean);
  const events = new AbortController(); let disposed = false;
  const render = () => { if (!disposed) for (const button of buttons) if (button.disabled !== session.busy) button.disabled = session.busy; };
  root.addEventListener("click", event => {
    const target = event.target; if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("button"); if (!button || !ids[button.id] || button.disabled || !button.getClientRects().length) return;
    const requestVisit = visit(), focused = document.activeElement === button;
    void session.run(ids[button.id]).finally(() => {
      if (!disposed && focused && requestVisit === visit() && root.open && !button.disabled && button.getClientRects().length && (document.activeElement === document.body || document.activeElement === root)) button.focus();
    });
  }, { signal: events.signal });
  render(); return { render, dispose() { disposed = true; events.abort(); } };
}
