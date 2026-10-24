import type { ModelCatalogItem, ModelDownloadState } from "./desktop-settings-types.ts";
import type { DownloadNetwork, createModelManagementBackend } from "./model-management-backend.ts";

export type ModelAction = "start" | "cancel" | "select" | "uninstall";
type Options = {
  backend: ReturnType<typeof createModelManagementBackend>;
  visit(): number; isOpen(): boolean;
  catalog(): ModelCatalogItem[]; downloads(): ModelDownloadState[];
  accepted(states: ModelDownloadState[]): void; changed(): void;
  captureNetwork(): { request: DownloadNetwork; acknowledge(): boolean } | null;
  captureSelection(kind: ModelCatalogItem["kind"]): { acknowledge(): boolean };
  invalidateLoads(): void;
  reload(): Promise<boolean | void>;
  message(text: string): void;
  schedule?(callback: () => void): number; cancelTimer?(timer: number): void;
};
const active = (state: ModelDownloadState) => ["queued", "downloading", "cancelling"].includes(state.status);
const terminalMessage = (state: ModelDownloadState | undefined) => state?.status === "done" ? "模型已校验并安装。" : state?.status === "failed" ? `下载失败：${state.error ?? "未知错误"}` : state?.status === "cancelled" ? "下载已取消，临时文件保留供续传。" : "";

/** Owns model operations and one visible-settings observer, not a second download store. */
export function createModelManagementSession(options: Options) {
  let disposed = false, mutation = false, generation = 0, timer: number | null = null, timerGeneration = 0;
  let reading: { visit: number; request: number; promise: Promise<void> } | null = null;
  const current = (visit: number) => !disposed && options.isOpen() && visit === options.visit();
  const stopTimer = () => { ++timerGeneration; if (timer !== null) { (options.cancelTimer ?? globalThis.clearTimeout)(timer); timer = null; } };
  const arm = () => {
    stopTimer();
    if (!disposed && options.isOpen() && !mutation && (!reading || reading.request !== generation || reading.visit !== options.visit()) && options.downloads().some(active)) {
      const token = timerGeneration, visit = options.visit();
      timer = (options.schedule ?? (fn => globalThis.setTimeout(fn, 500)))(() => { if (token !== timerGeneration || !current(visit)) return; timer = null; void api.refresh(); });
    }
  };
  const acceptOne = (state: ModelDownloadState) => options.accepted([...options.downloads().filter(item => item.modelId !== state.modelId), state]);
  const reload = async () => { if (await options.reload() === false) throw Error("配置读取未完成，可重新打开设置重试"); };
  const api = {
    get busy() { return mutation; },
    observe() { arm(); },
    hidden() { ++generation; stopTimer(); },
    async refresh(): Promise<void> {
      if (disposed || !options.isOpen()) return;
      const visit = options.visit();
      if (reading?.visit === visit && reading.request === generation) return reading.promise;
      const request = ++generation, previous = new Set(options.downloads().filter(active).map(item => item.modelId));
      const valid = () => current(visit) && request === generation;
      const ticket = { visit, request, promise: Promise.resolve() };
      const work = async () => {
        try {
          const states = await options.backend.states();
          if (!valid()) return;
          options.accepted(states);
          if (previous.size && !states.some(active)) {
            await reload();
            if (!valid()) return;
            const finished = states.find(item => previous.has(item.modelId) && item.status === "failed") ?? states.find(item => previous.has(item.modelId));
            options.message(terminalMessage(finished));
          }
        } catch (error) { if (valid()) options.message(`无法刷新模型下载状态：${String(error)}`); }
        finally { if (reading === ticket) reading = null; arm(); }
      };
      reading = ticket; ticket.promise = work(); return ticket.promise;
    },
    async run(action: ModelAction, modelId: string): Promise<void> {
      if (disposed || mutation || !options.isOpen()) return;
      const model = options.catalog().find(item => item.id === modelId); if (!model) return;
      const visit = options.visit();
      mutation = true; ++generation; stopTimer(); options.invalidateLoads(); options.changed();
      let committed = false, networkSaved = false;
      try {
        if (action === "start") {
          const snapshot = options.captureNetwork(); if (!snapshot) return;
          options.message("正在保存当前网络配置并连接模型来源…");
          await options.backend.network({ ...snapshot.request }); networkSaved = true;
          if (disposed) return;
          // Closing settings hides observation; the explicit download intent still proceeds.
          const newer = current(visit) ? snapshot.acknowledge() : false;
          const state = await options.backend.start(modelId); committed = true;
          if (disposed) return;
          ++generation; options.invalidateLoads();
          if (!current(visit)) return;
          acceptOne(state);
          if (!active(state)) {
            await reload();
            if (current(visit)) options.message(terminalMessage(state));
          } else options.message(newer ? "下载已排队；新的网络输入尚未保存。" : "下载已排队，正在连接模型来源…");
        } else if (action === "cancel") {
          const state = await options.backend.cancel(modelId); committed = true;
          if (disposed) return;
          ++generation; options.invalidateLoads();
          if (!current(visit)) return;
          acceptOne(state);
          if (!active(state)) { await reload(); if (current(visit)) options.message(terminalMessage(state)); }
        } else {
          const snapshot = action === "select" ? options.captureSelection(model.kind) : null;
          if (action === "select") { options.message("正在校验并切换模型…"); await options.backend.select(modelId); }
          else await options.backend.uninstall(modelId);
          committed = true;
          if (disposed) return;
          ++generation; options.invalidateLoads();
          if (!current(visit)) return;
          const newer = snapshot?.acknowledge() ?? false;
          await reload();
          if (current(visit)) options.message(action === "uninstall" ? "模型及其未完成下载已删除。" : newer ? "模型已设为当前使用；新的路径输入尚未保存。" : "模型已校验并设为当前使用。");
        }
      } catch (error) {
        if (current(visit)) options.message(committed ? `模型操作已完成，但状态刷新失败：${String(error)}` : networkSaved ? `网络配置已保存，但无法开始下载：${String(error)}` : `${action === "start" ? "无法开始下载" : action === "cancel" ? "无法取消下载" : action === "select" ? "无法选择模型" : "无法卸载模型"}：${String(error)}`);
      } finally {
        mutation = false;
        if (!disposed) {
          options.changed();
          if (visit !== options.visit() && options.isOpen()) await reload().catch(() => {});
          arm();
        }
      }
    },
    dispose() { disposed = true; ++generation; stopTimer(); reading = null; },
  };
  return api;
}
