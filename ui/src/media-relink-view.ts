import type { createMediaRelinkSession } from "./media-relink-session.ts";
const visible = (node: HTMLElement) => node.isConnected && node.getClientRects().length > 0 && !node.closest("[inert]");
export function mountMediaRelinkView(button: HTMLButtonElement, session: ReturnType<typeof createMediaRelinkSession>, visit: () => number) {
  const events = new AbortController(); let disposed = false;
  button.addEventListener("click", () => {
    if (button.disabled || !visible(button)) return;
    const requestVisit = visit(), focused = document.activeElement === button;
    void session.run().finally(() => {
      if (!disposed && focused && requestVisit === visit() && !button.disabled && visible(button) && document.activeElement === document.body) button.focus();
    });
  }, { signal: events.signal });
  return { dispose() { disposed = true; events.abort(); } };
}
