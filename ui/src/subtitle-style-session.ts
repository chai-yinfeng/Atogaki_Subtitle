import type { SubtitleFontFamily, SubtitleStylePreview, SubtitleStyleState, SubtitleStyleSet } from "./subtitle-style-types";
import type { createSubtitleStyleBackend } from "./subtitle-style-backend";
type Context = { jobId: string; navigation: number; timestampMs: number };
type Options = {
  context(): Context | null; available(): boolean;
  backend: ReturnType<typeof createSubtitleStyleBackend>;
  view: {
    open(): void; close(): void; valid(): boolean;
    read(): { styles: SubtitleStyleSet; track: string };
    loaded(state: SubtitleStyleState, fonts: SubtitleFontFamily[]): void;
    saved(state: SubtitleStyleState): void;
    previewed(preview: SubtitleStylePreview, automatic: boolean, saved: boolean): void;
    message(text: string): void; busy(value: boolean, ready: boolean): void;
  };
  later(callback: () => void, delay: number): () => void;
};
/** One style editing visit, with serial rendering and latest draft acceptance. */
export function createSubtitleStyleSession(options: Options) {
  let disposed = false, generation = 0, edit = 0, savedEdit = 0, open = false, busy = false, ready = false, queued = false;
  let cancelTimer: (() => void) | null = null, fonts: SubtitleFontFamily[] | null = null;
  let visit: Context | null = null;
  const current = (ticket: number) => {
    const context = options.context();
    return !disposed && open && generation === ticket && context?.jobId === visit?.jobId && context?.navigation === visit?.navigation;
  };
  const cancel = () => { cancelTimer?.(); cancelTimer = null; };
  const setBusy = (value: boolean) => { busy = value; options.view.busy(value, ready); };
  const schedule = (delay = 700) => {
    if (!open || !ready || disposed) return;
    cancel();
    if (delay > 0) options.view.message("样式已修改；停止操作后将自动刷新真实预览…");
    const ticket = generation;
    cancelTimer = options.later(() => { cancelTimer = null; if (current(ticket)) void preview(true); }, delay);
  };
  const finish = (ticket: number) => {
    if (!current(ticket)) return;
    setBusy(false);
    if (queued) { queued = false; schedule(150); }
  };
  const preview = async (automatic = false) => {
    cancel();
    const ticket = generation;
    if (!ready || !current(ticket)) return;
    if (busy) { queued = true; return; }
    if (!options.view.valid()) { options.view.message("请先补全或修正标红的样式参数；预览暂未刷新。"); return; }
    const version = edit, draft = options.view.read(), context = options.context()!;
    setBusy(true); options.view.message("正在通过 FFmpeg/libass 生成预览帧…");
    try {
      const result = await options.backend.preview(context.jobId, draft.styles, draft.track, Math.max(0, Math.round(context.timestampMs)));
      if (current(ticket) && version === edit) options.view.previewed(result, automatic, savedEdit === version);
    } catch (error) { if (current(ticket) && version === edit) options.view.message(`预览失败：${String(error)}`); }
    finally { finish(ticket); }
  };
  const close = () => {
    if (disposed) return;
    generation++; open = false; ready = false; queued = false; visit = null; cancel();
    setBusy(false); options.view.close();
  };
  return {
    async open() {
      if (disposed || open || !options.available()) return;
      const context = options.context(); if (!context) return;
      visit = { ...context }; open = true; ready = false; edit = 0; savedEdit = 0;
      const ticket = ++generation;
      options.view.open(); setBusy(true); options.view.message("正在读取任务样式和本机字体…");
      try {
        const [state, loadedFonts] = await Promise.all([options.backend.load(context.jobId), fonts ? Promise.resolve(fonts) : options.backend.fonts()]);
        if (!current(ticket)) return;
        fonts = loadedFonts; options.view.loaded(state, fonts); ready = true;
        options.view.message(`已读取 ${fonts.length} 个字体族，正在生成初始真实预览…`);
      } catch (error) { if (current(ticket)) options.view.message(`无法读取字幕样式：${String(error)}`); }
      finally { finish(ticket); if (current(ticket) && ready) schedule(0); }
    },
    changed(delay = 700) { if (!open || disposed) return; edit++; if (busy && ready) queued = true; schedule(delay); },
    preview,
    async save() {
      const ticket = generation;
      if (!ready || busy || !current(ticket)) return;
      if (!options.view.valid()) { options.view.message("请先补全或修正标红的样式参数，再保存任务样式。"); return; }
      cancel(); const version = edit, draft = options.view.read(), jobId = visit!.jobId;
      queued = true; setBusy(true); options.view.message("正在校验并保存任务样式…");
      try {
        const state = await options.backend.save(jobId, draft.styles);
        if (!current(ticket)) return;
        savedEdit = version;
        if (version === edit) options.view.saved(state);
        options.view.message(version === edit ? "任务样式已保存；之后的 ASS 导出和视频烧录都会使用它。" : "任务样式已保存；保存期间的新修改尚未保存。");
      } catch (error) { if (current(ticket)) options.view.message(`保存失败：${String(error)}`); }
      finally { finish(ticket); }
    },
    close,
    dispose() { if (!disposed) { close(); disposed = true; fonts = null; } },
  };
}
