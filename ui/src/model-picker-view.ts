import type { createModelPickerSession } from "./model-picker-session.ts";
import type { ModelPickerKind } from "./model-picker-backend.ts";

/** Delegates existing chooser buttons and releases their listeners on dispose. */
export function mountModelPickerView(root: HTMLDialogElement, session: ReturnType<typeof createModelPickerSession>, visit: () => number) {
  const ids: Record<string, ModelPickerKind> = { "settings-choose-whisper": "whisper", "settings-choose-vad": "vad", "settings-choose-hy-mt2": "hy-mt2" };
  const buttons = Object.keys(ids).map(id => root.querySelector<HTMLButtonElement>(`#${id}`)!).filter(Boolean);
  const events = new AbortController(); let disposed = false;
  const render = () => { if (!disposed) for (const button of buttons) if (button.disabled !== session.busy) button.disabled = session.busy; };
  root.addEventListener("click", event => {
    const target = event.target; if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("button"); if (!button || button.disabled || !ids[button.id] || !button.getClientRects().length || button.closest("[inert]")) return;
    const requestVisit = visit(), focused = document.activeElement === button;
    void session.pick(ids[button.id]).finally(() => {
      if (!disposed && focused && requestVisit === visit() && root.open && !button.disabled && button.getClientRects().length && !button.closest("[inert]") && (document.activeElement === document.body || document.activeElement === root)) button.focus();
    });
  }, { signal: events.signal });
  render();
  return { render, dispose() { disposed = true; events.abort(); } };
}
