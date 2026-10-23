import { staleTranslationCount } from "./subtitle-translation-state.ts";
import type { SubtitleSegment } from "./editor-types";
import type { SubtitleExportArtifact } from "./subtitle-export-types";
import type { createSubtitleExportBackend } from "./subtitle-export-backend";

type Context = { jobId: string; navigation: number; inputPath: string | null; segments: readonly SubtitleSegment[]; contextStaleIds: readonly string[] };
type Options = {
  context(): Context | null;
  available(): boolean;
  dirty(): boolean;
  backend: ReturnType<typeof createSubtitleExportBackend>;
  view: { choose(): Promise<SubtitleExportArtifact[] | null>; overwrite(names: string[]): Promise<boolean>; cancel(): void; idle?(): void };
  busyChanged(busy: boolean): void;
  status(message: string, error?: boolean): void;
  resultChanged(path: string | null): void;
  fileManager: string;
};
/** One export intent, bound to the task/navigation that opened it. Back-end
 * validation still owns file conflicts, translation freshness and writes. */
export function createSubtitleExport(options: Options) {
  let disposed = false, generation = 0, lastPath: string | null = null;
  let active: { generation: number; context: Context; locked: boolean } | null = null;
  const current = (ticket: NonNullable<typeof active>) => {
    const context = options.context();
    return !disposed && active === ticket && ticket.generation === generation &&
      context?.jobId === ticket.context.jobId && context.navigation === ticket.context.navigation;
  };
  const finish = (ticket: NonNullable<typeof active>) => {
    if (active !== ticket) return;
    const relevant = current(ticket);
    active = null;
    if (ticket.locked) options.busyChanged(false);
    if (relevant) options.view.idle?.();
  };
  const clearResult = () => { lastPath = null; options.resultChanged(null); };
  const reset = () => {
    generation++;
    const previous = active; active = null;
    options.view.cancel();
    if (previous?.locked) options.busyChanged(false);
    clearResult();
  };
  return {
    get lastPath() { return lastPath; },
    clearResult,
    reset,
    async export() {
      const context = options.context();
      if (disposed || active || !context || !options.available()) return;
      if (options.dirty()) { options.status("请先保存各段尚未保存的修改，再导出字幕。", true); return; }
      const ticket = { generation, context: { ...context }, locked: false }; active = ticket;
      try {
        const selected = await options.view.choose();
        if (!current(ticket) || !selected?.length) return;
        const artifacts = [...selected];
        if (options.dirty()) { options.status("请先保存各段尚未保存的修改，再导出字幕。", true); return; }
        const latest = options.context()!, segments = latest.segments;
        const stale = staleTranslationCount(segments, latest.contextStaleIds);
        const missing = segments.filter(cue => !cue.translated_text?.trim()).length;
        if (artifacts.some(artifact => artifact !== "source_srt") && (stale || missing)) {
          options.status(`含译文的导出需要完整且最新的译文；当前缺失 ${missing} 段、待重译 ${stale} 段。也可以只导出原文 SRT。`, true);
          return;
        }
        ticket.locked = true; options.busyChanged(true);
        options.status("正在选择字幕导出目录…");
        const path = ticket.context.inputPath;
        const separator = path ? Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) : -1;
        const initialDirectory = path && separator > 0 ? path.slice(0, separator) : null;
        const directory = await options.backend.pickDirectory(initialDirectory);
        if (!current(ticket)) return;
        if (!directory) { options.status("已取消字幕导出。"); return; }
        const plan = await options.backend.preview(ticket.context.jobId, directory, artifacts);
        if (!current(ticket)) return;
        let overwrite = false;
        if (plan.existing_files.length) {
          overwrite = await options.view.overwrite(plan.existing_files.map(file => file.split(/[\\/]/).pop() ?? file));
          if (!current(ticket)) return;
          if (!overwrite) { options.status("已取消导出，现有字幕文件没有被修改。"); return; }
        }
        options.status("正在从当前字幕内容生成所选字幕…");
        const exported = await options.backend.export(ticket.context.jobId, directory, artifacts, overwrite);
        if (!current(ticket)) return;
        lastPath = exported[artifacts[0]];
        options.resultChanged(lastPath);
        options.status(`已导出 ${artifacts.length} 个字幕文件到：${directory}`);
      } catch (error) {
        if (current(ticket)) options.status(`导出失败：${String(error)}`, true);
      } finally { finish(ticket); }
    },
    async reveal() {
      const context = options.context(), path = lastPath;
      if (disposed || active || !context || !path || !options.available()) return;
      const ticket = { generation, context: { ...context }, locked: true }; active = ticket;
      options.busyChanged(true);
      try { await options.backend.reveal(path); }
      catch (error) { if (current(ticket)) options.status(`${options.fileManager} 定位失败：${String(error)}`, true); }
      finally { finish(ticket); }
    },
    dispose() { if (!disposed) { reset(); disposed = true; } },
  };
}
