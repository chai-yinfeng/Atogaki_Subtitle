import type { SubtitleSegment } from "./editor-types";
import type { createStructureEditBackend } from "./editor-structure-backend";
import type { StructureEditResult, StructureUndoDescriptor, StructureUndoResult } from "./editor-structure-types";

type Context = { jobId: string; navigation: number; selectedId: string | null; segments: readonly SubtitleSegment[] };
type Options = {
  context(): Context | null; available(): boolean; dirty(): boolean; flush(): Promise<boolean>;
  backend: Pick<ReturnType<typeof createStructureEditBackend>, "undo" | "state">;
  busy(value: boolean): void; status(message: string, error?: boolean): void;
  apply(result: StructureUndoResult, selectedId?: string): void;
  clearTextHistory(jobId: string): void; changed(): void;
};
export function structureEditNotice(result: Pick<StructureEditResult, "undo" | "warnings">): string {
  const notices = [...result.warnings];
  if (result.undo.oversized) notices.push("本次修改已保存，但超过撤销内存预算，无法撤销；本任务之前的结构历史已清除。");
  else if (result.undo.discarded_entries) notices.push(`已释放 ${result.undo.discarded_entries} 条较早或失效的结构历史；可撤销范围以当前按钮为准。`);
  return notices.length ? ` ${notices.join(" ")}` : "";
}

/** UI caches only the latest opaque descriptor. Backend owns the sole bounded history. */
export function createEditorStructureHistory(options: Options) {
  const latest = new Map<string, StructureUndoDescriptor>();
  let disposed = false, preparing = false, ownsBusy = false, generation = 0;
  const current = (context: Context) => {
    if (disposed) return null;
    const now = options.context();
    return now?.jobId === context.jobId && now.navigation === context.navigation ? now : null;
  };
  const release = () => { if (ownsBusy) { ownsBusy = false; options.busy(false); } };
  return {
    remember(result: StructureEditResult, jobId: string) {
      if (disposed) return;
      generation++;
      if (result.undo.discarded_entries) latest.clear();
      if (result.undo.latest) latest.set(jobId, { ...result.undo.latest }); else latest.delete(jobId);
      options.clearTextHistory(jobId);
    },
    invalidate(jobId: string) { if (disposed) return; generation++; latest.delete(jobId); options.changed(); },
    async refresh() {
      if (disposed) return;
      const opened = options.context(); if (!opened) return;
      const request = ++generation;
      latest.delete(opened.jobId); options.changed();
      try {
        const state = await options.backend.state(opened.jobId), now = current(opened);
        if (!now || request !== generation) return;
        if (JSON.stringify(now.segments) === JSON.stringify(state.segments) && state.latest) latest.set(opened.jobId, { ...state.latest });
      } catch { /* A missing session/history is never an invitation to restore client snapshots. */ }
      finally { if (current(opened) && request === generation) options.changed(); }
    },
    presentation() {
      if (disposed) return { available: false, label: "" };
      const context = options.context(), entry = context && latest.get(context.jobId);
      return { available: !!(entry && !preparing), label: entry?.label ?? "" };
    },
    async undo() {
      if (disposed || preparing || !options.available()) return;
      const opened = options.context(); if (!opened) return;
      const entry = latest.get(opened.jobId); if (!entry) return;
      preparing = true; options.changed();
      try {
        if (!await options.flush()) return;
        const context = current(opened);
        if (!context || context.selectedId !== opened.selectedId || !options.available()) return;
        if (options.dirty()) { options.status("撤销打轴前请先保存或放弃当前文字修改。", true); return; }
        if (latest.get(context.jobId) !== entry) {
          options.status("字幕或撤销历史已变化，请核对当前内容后重试。", true); return;
        }
        ownsBusy = true; options.busy(true);
        const restored = await options.backend.undo(context.jobId, entry.token);
        if (!current(context)) return;
        generation++;
        if (restored.discarded_entries) latest.clear();
        if (restored.latest) latest.set(context.jobId, { ...restored.latest }); else latest.delete(context.jobId);
        options.apply(restored, context.selectedId ?? undefined);
        options.status(`已撤销${entry.label}。撤销历史只在当前 App 会话内保留。${restored.discarded_entries ? ` 已释放 ${restored.discarded_entries} 条失效或超出预算的历史。` : ""}`);
      } catch (error) {
        if (current(opened)) options.status(`无法撤销结构修改：${String(error)}`, true);
      } finally {
        release(); preparing = false;
        if (!disposed) options.changed();
      }
    },
    dispose() { if (disposed) return; disposed = true; generation++; release(); latest.clear(); },
  };
}
