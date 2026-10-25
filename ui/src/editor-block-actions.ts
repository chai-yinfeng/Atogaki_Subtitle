import type { createEditorBackend } from "./editor-backend";
import type { SubtitleSegment } from "./editor-types";

type Identity = { jobId: string; navigation: number };
type Context = Identity & { selectedId: string | null; segments: readonly SubtitleSegment[] };
type Options = {
  context(): Context | null; available(): boolean; dirty(): boolean; flush(): Promise<boolean>;
  pause(): void; seek(ms: number): void;
  confirm(options: { title: string; message: string; confirmLabel: string; danger: boolean }): Promise<boolean>;
  backend: Pick<ReturnType<typeof createEditorBackend>, "editBlock">;
  busy(value: boolean): void; status(message: string, error?: boolean): void;
  remember(before: SubtitleSegment[], after: SubtitleSegment[], label: string, jobId: string): void;
  apply(segments: SubtitleSegment[], selectedId?: string): void;
};
const notExecuted = "操作未执行，请先完成当前编辑后重试。";
const changed = "字幕或编辑内容已变化，未删除；请核对后重试。";

/** Uses the existing block transaction and confirmation view. The confirmed
 * cue must still be the selected, unchanged cue when preparation finishes. */
export function createEditorBlockActions(options: Options) {
  let disposed = false, preparing = false, ownsBusy = false;
  const current = (identity: Identity) => {
    if (disposed) return null;
    const now = options.context();
    return now?.jobId === identity.jobId && now.navigation === identity.navigation ? now : null;
  };
  const release = () => { if (ownsBusy) { ownsBusy = false; options.busy(false); } };
  const unchanged = (context: Context, cue: SubtitleSegment) => context.selectedId === cue.id
    && JSON.stringify(context.segments.find(item => item.id === cue.id)) === JSON.stringify(cue) && !options.dirty();
  const commit = async (identity: Identity, insertion: SubtitleSegment | null, confirmed?: SubtitleSegment): Promise<string | null> => {
    if (!await options.flush()) return notExecuted;
    const context = current(identity);
    if (!context || !options.available()) return notExecuted;
    if (confirmed && !unchanged(context, confirmed)) { options.status(changed, true); return changed; }
    if (options.dirty()) return notExecuted;
    const before = context.segments.map(cue => ({ ...cue }));
    ownsBusy = true; options.busy(true);
    const after = await options.backend.editBlock(context.jobId, before, confirmed?.id ?? null, insertion);
    if (current(identity)) {
      options.remember(before, after, confirmed ? "删除字幕块" : "插入字幕块", context.jobId);
      options.apply(after, insertion?.id ?? after[0]?.id);
      if (insertion) options.seek(insertion.start_ms);
      options.status("已保存，可用“撤销上次打轴”撤回；仅实际依赖被删除块的机器译文需要核对。");
    }
    return null;
  };
  const fail = (identity: Identity, error: unknown) => {
    const message = `字幕块操作失败：${String(error)}`;
    if (current(identity)) options.status(message, true);
    return message;
  };
  return {
    async insert(identity: Identity, cue: SubtitleSegment): Promise<string | null> {
      if (disposed || preparing || !options.available() || !current(identity) || cue.job_id !== identity.jobId) return notExecuted;
      preparing = true;
      try { return await commit(identity, { ...cue }); }
      catch (error) { return fail(identity, error); }
      finally { release(); preparing = false; }
    },
    async deleteSelected(): Promise<void> {
      if (disposed || preparing || !options.available()) return;
      const opened = options.context(); if (!opened) return;
      preparing = true;
      try {
        if (!await options.flush()) return;
        const context = current(opened);
        if (!context || context.selectedId !== opened.selectedId || !options.available()) return;
        if (options.dirty()) { options.status("删除前请先保存或放弃文字修改。", true); return; }
        const cue = context.segments.find(item => item.id === context.selectedId);
        if (!cue) { options.status("请先选择要删除的字幕块。", true); return; }
        const confirmed = { ...cue };
        options.pause();
        if (!await options.confirm({ title: "删除当前字幕块？", message: "删除原文、译文及此块时间范围；不会移动其他字幕。学习收藏保留来源快照。当前会话可用“撤销上次打轴”恢复。", confirmLabel: "删除字幕块", danger: true })) return;
        const now = current(opened);
        if (!now) return;
        if (!unchanged(now, confirmed)) { options.status(changed, true); return; }
        await commit(opened, null, confirmed);
      } catch (error) { fail(opened, error); }
      finally { release(); preparing = false; }
    },
    dispose() { if (disposed) return; disposed = true; release(); },
  };
}
