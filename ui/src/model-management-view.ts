import type { ModelCatalogItem, ModelDownloadState } from "./desktop-settings-types.ts";
import type { ModelAction } from "./model-management-session.ts";

type Options = {
  visit(): number;
  state(): { catalog: ModelCatalogItem[]; downloads: ModelDownloadState[]; busy: boolean };
  action(action: ModelAction, id: string): Promise<void>;
  formatBytes(bytes: number): string;
  error(text: string): void;
};
type Card = { node: HTMLElement; name: HTMLElement; description: HTMLElement; progress: HTMLProgressElement; status: HTMLElement; primary: HTMLButtonElement; uninstall: HTMLButtonElement };
const text = (node: HTMLElement, value: string) => { if (node.textContent !== value) node.textContent = value; };
const flag = (node: HTMLElement, name: string, value: boolean) => { if (node.classList.contains(name) !== value) node.classList.toggle(name, value); };
const disabled = (node: HTMLButtonElement, value: boolean) => { if (node.disabled !== value) node.disabled = value; };
const visible = (node: HTMLElement) => node.isConnected && node.getClientRects().length > 0 && !node.closest("[inert]");

/** Stable model-ID cards with one delegated listener; requests stay in the existing session. */
export function mountModelManagementView(root: HTMLElement, options: Options) {
  const cards = new Map<string, Card>(), events = new AbortController();
  let disposed = false;
  const create = (id: string): Card => {
    const node = document.createElement("article"); node.className = "model-card"; node.dataset.modelCard = id;
    node.innerHTML = '<div><strong data-name></strong><span data-description></span></div><progress max="1" class="hidden"></progress><div class="model-card-action"><span data-status tabindex="-1"></span><div><button type="button" data-primary></button><button type="button" class="secondary" data-model-action="uninstall">卸载</button></div></div>';
    const primary = node.querySelector<HTMLButtonElement>("[data-primary]")!, uninstall = node.querySelector<HTMLButtonElement>("[data-model-action=uninstall]")!;
    primary.dataset.modelId = id; uninstall.dataset.modelId = id;
    return { node, primary, uninstall, name: node.querySelector("[data-name]")!, description: node.querySelector("[data-description]")!, progress: node.querySelector("progress")!, status: node.querySelector("[data-status]")! };
  };
  const render = () => {
    if (disposed) return;
    const state = options.state(), byId = new Map(state.downloads.map(item => [item.modelId, item]));
    const downloading = state.downloads.some(item => ["queued", "downloading", "cancelling"].includes(item.status));
    const before = document.activeElement instanceof HTMLElement && root.contains(document.activeElement) ? document.activeElement : null;
    const seen = new Set<string>(), order: HTMLElement[] = [];
    for (const model of state.catalog) {
      if (seen.has(model.id)) continue; seen.add(model.id);
      let card = cards.get(model.id); if (!card) { card = create(model.id); cards.set(model.id, card); }
      const download = byId.get(model.id), installed = download?.status === "done";
      const source = download?.source ? ` · ${download.source}` : "";
      const status = installed ? `${download.active ? "已下载 · 当前使用" : "已下载"}${source}` : download?.status === "failed" ? `失败：${download.error ?? "未知错误"}${source}` : download?.status === "cancelled" ? `已取消；保留临时文件供续传${source}` : download?.status === "cancelling" ? `正在取消…${source}` : download?.status === "downloading" ? `${options.formatBytes(download.downloadedBytes)}${download.totalBytes ? ` / ${options.formatBytes(download.totalBytes)}` : ""}${source}` : download?.status === "queued" ? `等待下载${source}` : "";
      text(card.name, model.name); text(card.description, `${model.sizeLabel} · ${model.recommendedFor}`); text(card.status, status || "未下载");
      const label = `${model.name}：${status || "未下载"}`; if (card.status.getAttribute("aria-label") !== label) card.status.setAttribute("aria-label", label);
      const progressLabel = `${model.name} 下载进度`; if (card.progress.getAttribute("aria-label") !== progressLabel) card.progress.setAttribute("aria-label", progressLabel);
      flag(card.status, "warning", download?.status === "failed");
      const progress = download?.totalBytes ? Math.max(0, Math.min(1, download.downloadedBytes / download.totalBytes)) : null;
      flag(card.progress, "hidden", progress === null); if (progress !== null && card.progress.value !== progress) card.progress.value = progress;
      const cancelling = download && ["queued", "downloading", "cancelling"].includes(download.status);
      const action = installed ? "select" : cancelling ? "cancel" : "download";
      if (card.primary.dataset.modelAction !== action) card.primary.dataset.modelAction = action;
      flag(card.primary, "secondary", !installed);
      text(card.primary, installed ? "设为当前" : cancelling ? download.status === "cancelling" ? "正在取消" : "取消" : download?.status === "failed" || download?.status === "cancelled" ? "继续下载" : "下载");
      flag(card.primary, "hidden", Boolean(installed && download.active)); flag(card.uninstall, "hidden", !installed);
      disabled(card.primary, state.busy || (installed ? false : cancelling ? download.status === "cancelling" : downloading)); disabled(card.uninstall, state.busy);
      order.push(card.node);
    }
    for (const [id, card] of cards) if (!seen.has(id)) { card.node.remove(); cards.delete(id); }
    order.forEach((node, index) => { if (root.children[index] !== node) root.insertBefore(node, root.children[index] ?? null); });
    // Preserve a keyboard user's place when an operation disables or hides its button.
    if (before?.isConnected && (document.activeElement === before || document.activeElement === document.body)) {
      const card = cards.get(before.closest<HTMLElement>("[data-model-card]")?.dataset.modelCard ?? "");
      if (card && before instanceof HTMLButtonElement && (before.disabled || !visible(before))) { if (visible(card.status)) card.status.focus(); }
      else if (visible(before) && document.activeElement === document.body) before.focus();
    }
  };
  root.addEventListener("click", event => {
    const target = event.target; if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("[data-model-action]");
    if (!button || button.disabled || !visible(button)) return;
    const id = button.dataset.modelId!, card = cards.get(id); if (!card) return;
    const action = button.dataset.modelAction;
    if (action !== "download" && action !== "cancel" && action !== "select" && action !== "uninstall") return;
    const visit = options.visit(), focused = document.activeElement === button;
    void (async () => {
      try { await options.action(action === "download" ? "start" : action, id); }
      catch (error) { if (!disposed && visit === options.visit()) options.error(String(error)); }
      finally {
        if (!disposed && focused && visit === options.visit() && card.node.isConnected && (document.activeElement === document.body || document.activeElement === card.status)) {
          const next = !card.primary.disabled && visible(card.primary) ? card.primary : card.status;
          if (visible(next)) next.focus();
        }
      }
    })();
  }, { signal: events.signal });
  render();
  return { render, dispose() { disposed = true; events.abort(); for (const card of cards.values()) card.node.remove(); cards.clear(); } };
}
