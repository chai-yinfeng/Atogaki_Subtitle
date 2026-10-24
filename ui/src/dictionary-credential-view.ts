import { dictionaryCredentialProviders, type DictionaryCredentialProvider } from "./dictionary-credential-backend.ts";
import type { DictionaryCredentialStatus } from "./desktop-settings-types.ts";
import type { DictionaryCredentialAction, createDictionaryCredentialSession } from "./dictionary-credential-session.ts";
export function mountDictionaryCredentialView(root: HTMLElement, session: ReturnType<typeof createDictionaryCredentialSession>, options: { visit(): number; isOpen(): boolean; statuses(): DictionaryCredentialStatus[] }) {
  const events = new AbortController(); let disposed = false;
  const buttons = [...root.querySelectorAll<HTMLButtonElement>("button")];
  const statuses = new Map([...root.querySelectorAll<HTMLElement>("[data-dictionary-status]")].map(node => [node.dataset.dictionaryStatus!, node]));
  const render = () => {
    if (disposed) return;
    for (const button of buttons) if (button.disabled !== session.busy) button.disabled = session.busy;
    for (const status of options.statuses()) {
      const node = statuses.get(status.providerId); if (!node) continue;
      const text = status.configured ? `已保存至 ${status.credentialStore}；内容不会回显` : `尚未在 ${status.credentialStore} 中配置`;
      if (node.textContent !== text) node.textContent = text;
      node.classList.toggle("warning", !status.configured);
    }
  };
  root.addEventListener("click", event => {
    const target = event.target; if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("button"); if (!button || button.disabled || !button.getClientRects().length) return;
    const actions: [DictionaryCredentialAction, string | undefined][] = [["save", button.dataset.saveDictionaryKey], ["delete", button.dataset.clearDictionaryKey], ["check", button.dataset.checkDictionaryKey]];
    const match = actions.find(([, provider]) => dictionaryCredentialProviders.includes(provider as DictionaryCredentialProvider)); if (!match) return;
    const visit = options.visit(), focused = document.activeElement === button;
    void session.run(match[0], match[1] as DictionaryCredentialProvider).finally(() => {
      if (!disposed && focused && visit === options.visit() && options.isOpen() && !button.disabled && button.getClientRects().length && (document.activeElement === document.body || document.activeElement === root.closest("dialog"))) button.focus();
    });
  }, { signal: events.signal });
  render(); return { render, dispose() { disposed = true; events.abort(); statuses.clear(); } };
}
