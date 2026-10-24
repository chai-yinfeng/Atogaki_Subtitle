import type { DictionaryCatalogItem, DictionaryDownloadState } from "./desktop-settings-types.ts";
import type { DictionaryDownloadNetwork, createDictionaryDownloadBackend } from "./dictionary-download-backend.ts";
type Options = {
  backend: ReturnType<typeof createDictionaryDownloadBackend>;
  visit(): number; isOpen(): boolean;
  catalog(): DictionaryCatalogItem[]; downloads(): DictionaryDownloadState[];
  accepted(states: DictionaryDownloadState[]): void; changed(): void;
  captureNetwork(): { request: DictionaryDownloadNetwork; acknowledge(): boolean } | null;
  invalidateLoads(): void;
  reload(): Promise<boolean | void>;
  message(text: string): void; clearMessage(text: string): void;
  schedule?(callback: () => void): number; cancelTimer?(timer: number): void;
};
const active = (state: DictionaryDownloadState) => ["queued", "resolving", "downloading"].includes(state.status);
const terminalMessage = (state: DictionaryDownloadState | undefined) => state?.status === "done" ? "词典包已校验并安装到正式应用数据目录。" : state?.status === "failed" ? `下载失败：${state.error ?? "未知错误"}` : "未取得本次下载状态，请重新打开设置检查。";

/** One visible-settings observer, retaining the existing backend download and state array. */
export function createDictionaryDownloadSession(options: Options) {
  let disposed = false, mutation = false, generation = 0, timer: number | null = null, timerGeneration = 0, lastMessage: string | null = null;
  let reading: { visit: number; request: number; promise: Promise<void> } | null = null;
  const message = (text: string) => { lastMessage = text; options.message(text); };
  const current = (visit: number) => !disposed && options.isOpen() && visit === options.visit();
  const stopTimer = () => { ++timerGeneration; if (timer !== null) { (options.cancelTimer ?? globalThis.clearTimeout)(timer); timer = null; } };
  const arm = () => {
    stopTimer();
    if (!disposed && options.isOpen() && !mutation && (!reading || reading.request !== generation || reading.visit !== options.visit()) && options.downloads().some(active)) {
      const token = timerGeneration, visit = options.visit();
      timer = (options.schedule ?? (fn => globalThis.setTimeout(fn, 500)))(() => { if (token !== timerGeneration || !current(visit)) return; timer = null; void api.refresh(); });
    }
  };
  const reload = async () => { if (await options.reload() === false) throw Error("词典配置读取未完成，可重新打开设置重试"); };
  const api = {
    get busy() { return mutation; },
    observe() { arm(); },
    hidden() { ++generation; stopTimer(); if (!disposed && lastMessage !== null) { options.clearMessage(lastMessage); lastMessage = null; } },
    async refresh(): Promise<void> {
      if (disposed || mutation || !options.isOpen()) return;
      const visit = options.visit();
      if (reading?.visit === visit && reading.request === generation) return reading.promise;
      const request = ++generation, previous = new Set(options.downloads().filter(active).map(item => item.dictionaryId));
      const valid = () => current(visit) && request === generation;
      const ticket = { visit, request, promise: Promise.resolve() };
      const work = async () => {
        try {
          const states = await options.backend.states(); if (!valid()) return;
          options.accepted(states);
          if (previous.size && !states.some(active)) {
            await reload(); if (!valid()) return;
            const finished = states.find(item => previous.has(item.dictionaryId) && item.status === "failed") ?? states.find(item => previous.has(item.dictionaryId));
            message(terminalMessage(finished));
          }
        } catch (error) { if (valid()) message(`无法读取词典下载状态：${String(error)}`); }
        finally { if (reading === ticket) reading = null; arm(); }
      };
      reading = ticket; ticket.promise = work(); return ticket.promise;
    },
    async start(dictionaryId: string): Promise<void> {
      if (disposed || mutation || !options.isOpen() || options.downloads().some(active) || !options.catalog().some(item => item.id === dictionaryId)) return;
      const visit = options.visit();
      mutation = true; ++generation; stopTimer(); options.invalidateLoads(); options.changed();
      let committed = false, networkSaved = false;
      try {
        const snapshot = options.captureNetwork(); if (!snapshot) return;
        message("正在保存当前网络配置并获取词典发布信息…");
        await options.backend.network({ ...snapshot.request }); networkSaved = true;
        if (disposed) return;
        // Closing the dialog hides observation, not an explicitly requested download.
        const newer = current(visit) ? snapshot.acknowledge() : false;
        const state = await options.backend.start(dictionaryId); committed = true;
        if (disposed) return;
        ++generation; options.invalidateLoads();
        if (!current(visit)) return;
        if (state.dictionaryId !== dictionaryId) throw Error("词典下载状态与请求来源不一致");
        options.accepted([...options.downloads().filter(item => item.dictionaryId !== dictionaryId), state]);
        if (!active(state)) { await reload(); if (current(visit)) message(terminalMessage(state)); }
        else message(newer ? "词典下载已排队；新的网络输入尚未保存。" : "词典下载已排队，正在获取发布信息…");
      } catch (error) {
        if (current(visit)) message(committed ? `词典下载操作已完成，但状态刷新失败：${String(error)}` : networkSaved ? `网络配置已保存，但无法开始词典下载：${String(error)}` : `无法开始词典下载：${String(error)}`);
      } finally {
        mutation = false;
        if (!disposed) {
          options.changed();
          if (visit !== options.visit() && options.isOpen()) await reload().catch(() => {});
          arm();
        }
      }
    },
    dispose() { disposed = true; ++generation; stopTimer(); reading = null; lastMessage = null; },
  };
  return api;
}
