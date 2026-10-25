import type { SubtitleSegment } from "./editor-types";
import type { createEditorBackend } from "./editor-backend";

type Context = { jobId: string; navigation: number; selectedId: string | null; segments: readonly SubtitleSegment[] };
type Entry = { before: SubtitleSegment[]; after: SubtitleSegment[]; label: string };
type Options = {
  context(): Context | null; available(): boolean; dirty(): boolean; flush(): Promise<boolean>;
  backend: Pick<ReturnType<typeof createEditorBackend>, "restoreStructure">;
  busy(value: boolean): void; status(message: string, error?: boolean): void;
  apply(segments: SubtitleSegment[], selectedId?: string): void;
  clearTextHistory(jobId: string): void; changed(): void;
};
const clone = (segments: readonly SubtitleSegment[]) => segments.map(cue => ({ ...cue }));
const fingerprint = (segments: readonly SubtitleSegment[]) => JSON.stringify(segments);

/** The only structure history: per-job, at most ten entries, app-session only.
 * Restore still uses the existing atomic Rust comparison and transaction. */
export function createEditorStructureHistory(options: Options) {
  const histories = new Map<string, Entry[]>();
  let disposed = false, preparing = false, ownsBusy = false;
  const current = (context: Context) => {
    if (disposed) return null;
    const now = options.context();
    return now?.jobId === context.jobId && now.navigation === context.navigation ? now : null;
  };
  const release = () => { if (ownsBusy) { ownsBusy = false; options.busy(false); } };
  return {
    remember(before: readonly SubtitleSegment[], after: readonly SubtitleSegment[], label: string, jobId: string) {
      if (disposed) return;
      const history = histories.get(jobId) ?? [];
      history.push({ before: clone(before), after: clone(after), label });
      if (history.length > 10) history.shift();
      histories.set(jobId, history);
      options.clearTextHistory(jobId);
    },
    presentation() {
      if (disposed) return { available: false, label: "" };
      const context = options.context(), entry = context && histories.get(context.jobId)?.at(-1);
      return { available: !!(context && entry && !preparing && fingerprint(context.segments) === fingerprint(entry.after)), label: entry?.label ?? "" };
    },
    async undo() {
      if (disposed || preparing || !options.available()) return;
      const opened = options.context();
      if (!opened) return;
      const history = histories.get(opened.jobId), entry = history?.at(-1);
      if (!entry) return;
      preparing = true; options.changed();
      try {
        if (!await options.flush()) return;
        const context = current(opened);
        if (!context || context.selectedId !== opened.selectedId || !options.available()) return;
        if (options.dirty()) { options.status("撤销打轴前请先保存或放弃当前文字修改。", true); return; }
        if (history?.at(-1) !== entry || fingerprint(context.segments) !== fingerprint(entry.after)) {
          options.status("字幕或撤销历史已变化，请核对当前内容后重试。", true); return;
        }
        ownsBusy = true; options.busy(true);
        const restored = await options.backend.restoreStructure(context.jobId, clone(entry.before), clone(entry.after));
        if (!current(context)) return;
        // Never pop a newer entry if another completed operation recorded one.
        const index = history.indexOf(entry);
        if (index >= 0) history.splice(index, 1);
        if (history.length === 0) histories.delete(context.jobId);
        options.apply(restored, context.selectedId ?? undefined);
        options.status(`已撤销${entry.label}。撤销历史只在当前 App 会话内保留。`);
      } catch (error) {
        if (current(opened)) options.status(`无法撤销结构修改：${String(error)}`, true);
      } finally {
        release(); preparing = false;
        if (!disposed) options.changed();
      }
    },
    dispose() { if (disposed) return; disposed = true; release(); histories.clear(); },
  };
}
