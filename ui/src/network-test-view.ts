import type { createNetworkTestSession } from "./network-test-session.ts";

export function mountNetworkTestView(root: HTMLDialogElement, session: ReturnType<typeof createNetworkTestSession>, visit: () => number) {
  const button = root.querySelector<HTMLButtonElement>("#test-network")!;
  const results = root.querySelector<HTMLDivElement>("#network-test-results")!;
  const fields = new Set(["settings-proxy-mode", "settings-proxy-url", "settings-model-mirror"]);
  const events = new AbortController(); let disposed = false, lastPresentation = "";
  const render = () => {
    if (disposed) return;
    if (button.disabled !== session.busy) button.disabled = session.busy;
    const state = session.state, presentation = JSON.stringify(state);
    if (presentation === lastPresentation) return;
    lastPresentation = presentation; results.replaceChildren();
    const row = (label: string, detail: string, ok: boolean) => {
      const container = document.createElement("div"), strong = document.createElement("strong"), span = document.createElement("span");
      container.className = ok ? "success" : "warning"; strong.textContent = label; span.textContent = detail;
      container.append(strong, span); results.append(container);
    };
    if (state.phase === "running" || state.phase === "stale") {
      const span = document.createElement("span"); span.textContent = state.phase === "running" ? "正在测试模型来源…" : "网络参数已变化，请重新测试。"; results.append(span);
    } else if (state.phase === "error") row("配置无效", state.error, false);
    else if (state.phase === "result") for (const check of state.checks) {
      row(check.label, check.ok ? `${check.status ?? "已连接"}${check.resolvedHost ? ` · ${check.resolvedHost}` : ""}` : check.error ?? "连接失败", check.ok);
    }
  };
  button.addEventListener("click", () => {
    const requestVisit = visit(), focused = document.activeElement === button;
    void session.run().finally(() => {
      if (!disposed && focused && requestVisit === visit() && root.open && !button.disabled && button.getClientRects().length && (document.activeElement === document.body || document.activeElement === root)) button.focus();
    });
  }, { signal: events.signal });
  const changed = (event: Event) => { const target = event.target; if (target instanceof HTMLElement && fields.has(target.id)) session.parametersChanged(); };
  root.addEventListener("input", changed, { signal: events.signal });
  root.addEventListener("change", changed, { signal: events.signal });
  render();
  return { render, dispose() { disposed = true; events.abort(); } };
}
