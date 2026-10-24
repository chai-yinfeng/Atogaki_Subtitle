import type { DictionaryCatalogItem, DictionaryDownloadState } from "./desktop-settings-types.ts";
type Options = {
  visit(): number;
  state(): { catalog: DictionaryCatalogItem[]; downloads: DictionaryDownloadState[]; busy: boolean };
  start(id: string): Promise<void>;
  formatBytes(bytes: number): string;
  error(text: string): void;
};
type Card = { node: HTMLElement; name: HTMLElement; summary: HTMLElement; description: HTMLElement; license: HTMLElement; source: HTMLElement; progress: HTMLProgressElement; status: HTMLElement; button: HTMLButtonElement };
const text = (node: HTMLElement, value: string) => { if (node.textContent !== value) node.textContent = value; };
const flag = (node: HTMLElement, name: string, value: boolean) => { if (node.classList.contains(name) !== value) node.classList.toggle(name, value); };
const visible = (node: HTMLElement) => node.isConnected && node.getClientRects().length > 0 && !node.closest("[inert]");

/** Stable dictionary-ID nodes; download state and requests remain in the existing session. */
export function mountDictionaryDownloadView(root: HTMLElement, options: Options) {
  const cards = new Map<string, Card>(), events = new AbortController(); let disposed = false;
  const create = (id: string): Card => {
    const node = document.createElement("article"); node.className = "model-card"; node.dataset.dictionaryCard = id;
    node.innerHTML = '<div><strong data-name></strong><span data-summary></span></div><span data-description></span><span data-license></span><span data-source></span><progress max="1" class="hidden"></progress><div class="model-card-action"><span data-status tabindex="-1"></span><button type="button" class="secondary">下载</button></div>';
    const button = node.querySelector<HTMLButtonElement>("button")!; button.dataset.downloadDictionary = id;
    return { node, button, name: node.querySelector("[data-name]")!, summary: node.querySelector("[data-summary]")!, description: node.querySelector("[data-description]")!, license: node.querySelector("[data-license]")!, source: node.querySelector("[data-source]")!, progress: node.querySelector("progress")!, status: node.querySelector("[data-status]")! };
  };
  const render = () => {
    if (disposed) return;
    const state = options.state(), byId = new Map(state.downloads.map(item => [item.dictionaryId, item]));
    const busy = state.busy || state.downloads.some(item => ["queued", "resolving", "downloading"].includes(item.status));
    const before = document.activeElement instanceof HTMLElement && root.contains(document.activeElement) ? document.activeElement : null;
    const seen = new Set<string>(), order: HTMLElement[] = [];
    for (const item of state.catalog) {
      if (seen.has(item.id)) continue; seen.add(item.id);
      let card = cards.get(item.id); if (!card) { card = create(item.id); cards.set(item.id, card); }
      const download = byId.get(item.id);
      const status = download?.status === "done" ? `已安装${download.version ? ` · ${download.version}` : ""}` : download?.status === "failed" ? `失败：${download.error ?? "未知错误"}` : download?.status === "downloading" ? `${options.formatBytes(download.downloadedBytes)}${download.totalBytes ? ` / ${options.formatBytes(download.totalBytes)}` : ""}${download.source ? ` · ${download.source}` : ""}` : download?.status === "resolving" ? "正在获取最新版与校验信息…" : download?.status === "queued" ? "等待下载" : "尚未安装";
      text(card.name, item.name); text(card.summary, `${item.languagePair} · ${item.sizeLabel} · ${item.versionLabel}`);
      text(card.description, item.description); text(card.license, `${item.license} · 署名：${item.attribution}`); text(card.source, `来源：${item.sourceUrl}`); text(card.status, status);
      const statusLabel = `${item.name}：${status}`, progressLabel = `${item.name} 下载进度`;
      if (card.status.getAttribute("aria-label") !== statusLabel) card.status.setAttribute("aria-label", statusLabel);
      if (card.progress.getAttribute("aria-label") !== progressLabel) card.progress.setAttribute("aria-label", progressLabel);
      flag(card.status, "warning", download?.status === "failed");
      const progress = download?.totalBytes ? Math.max(0, Math.min(1, download.downloadedBytes / download.totalBytes)) : null;
      flag(card.progress, "hidden", progress === null); if (progress !== null && card.progress.value !== progress) card.progress.value = progress;
      text(card.button, download?.status === "done" ? "检查更新" : download?.status === "failed" ? "重试" : "下载");
      if (card.button.disabled !== busy) card.button.disabled = busy;
      order.push(card.node);
    }
    for (const [id, card] of cards) if (!seen.has(id)) { card.node.remove(); cards.delete(id); }
    order.forEach((node, index) => { if (root.children[index] !== node) root.insertBefore(node, root.children[index] ?? null); });
    if (before?.isConnected && (document.activeElement === before || document.activeElement === document.body)) {
      const card = cards.get(before.closest<HTMLElement>("[data-dictionary-card]")?.dataset.dictionaryCard ?? "");
      if (card && before instanceof HTMLButtonElement && before.disabled && visible(card.status)) card.status.focus();
      else if (visible(before) && document.activeElement === document.body) before.focus();
    }
  };
  root.addEventListener("click", event => {
    const target = event.target; if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("[data-download-dictionary]");
    const card = button ? cards.get(button.dataset.downloadDictionary ?? "") : undefined;
    if (!button || !card || card.button !== button || button.disabled || !visible(button)) return;
    const visit = options.visit(), focused = document.activeElement === button;
    void options.start(button.dataset.downloadDictionary!).catch(error => { if (!disposed && visit === options.visit()) options.error(String(error)); }).finally(() => {
      if (disposed || !focused || visit !== options.visit() || !card.node.isConnected || !visible(card.node)) return;
      const active = document.activeElement;
      if (active !== document.body && active !== card.status && active !== button) return;
      const next = !button.disabled ? button : card.status; if (visible(next) && active !== next) next.focus();
    });
  }, { signal: events.signal });
  render();
  return { render, dispose() { disposed = true; events.abort(); for (const card of cards.values()) card.node.remove(); cards.clear(); } };
}
