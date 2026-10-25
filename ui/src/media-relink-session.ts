import type { RelinkedMediaJob } from "./media-relink-backend.ts";
export type MediaRelinkContext = { jobId: string; navigation: number; area: "karaoke" | "workspace"; section: "review" | "translation" | "export" };
type Options = {
  backend: { relink(jobId: string): Promise<RelinkedMediaJob | null> };
  context(): MediaRelinkContext | null;
  dirty(): boolean;
  acquire(): (() => void) | null;
  reload(context: MediaRelinkContext): { done: Promise<void>; current(): boolean };
  message(text: string, error?: boolean): void;
  changed(): void;
};
/** Frozen task identity; one idempotent busy release, including disposal during native selection. */
export function createMediaRelinkSession(options: Options) {
  let disposed = false, busy = false, releaseLock: (() => void) | null = null;
  const release = () => { const owned = releaseLock; releaseLock = null; owned?.(); };
  return {
    get busy() { return busy; },
    async run(): Promise<void> {
      if (disposed || busy) return;
      const context = options.context(); if (!context) return;
      const snapshot = { ...context };
      if (options.dirty()) { options.message("请先保存或放弃尚未保存的文字和时间修改，再重新定位原媒体。", true); return; }
      releaseLock = options.acquire(); if (!releaseLock) return;
      busy = true; options.changed(); let committed = false;
      let current = () => {
        if (disposed) return false;
        const now = options.context();
        return now?.jobId === snapshot.jobId && now.navigation === snapshot.navigation && now.area === snapshot.area && now.section === snapshot.section;
      };
      options.message("正在选择移动后的原媒体…");
      try {
        const updated = await options.backend.relink(snapshot.jobId);
        if (!current()) return;
        if (!updated) { options.message("已取消重新定位。"); return; }
        committed = true;
        if (updated.job_id !== snapshot.jobId) throw Error("返回的任务身份与原重定位请求不一致");
        if (options.dirty()) { options.message("原媒体路径已保存；保留当前新修改，保存后请重新打开任务以加载媒体。"); return; }
        // Existing navigation requires the workspace lock released; do it exactly once.
        release();
        const reload = options.reload(snapshot);
        current = () => !disposed && reload.current();
        await reload.done;
        if (current()) options.message(`已重新定位原媒体：${updated.input_path ?? ""}`);
      } catch (error) {
        if (current()) options.message(committed ? `原媒体路径已保存，但界面刷新失败：${String(error)}` : `重新定位原媒体失败：${String(error)}`, true);
      } finally { release(); busy = false; if (!disposed) options.changed(); }
    },
    dispose() { if (!disposed) { release(); disposed = true; } },
  };
}
