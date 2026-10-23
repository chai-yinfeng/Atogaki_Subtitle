import type { createVideoRenderBackend } from "./video-render-backend";
import type { MediaCapabilities, VideoRender, VideoRenderEstimate, VideoRenderQuality } from "./video-render-types";
type Draft = { outputPath: string; track: VideoRender["subtitle_track"]; quality: VideoRenderQuality };
type Context = { jobId: string; navigation: number; inputPath: string | null; videoAvailable: boolean };
type Options = {
  context(): Context | null; available(): boolean; dirty(): boolean; staleCount(): number; suggestedName(): string;
  backend: ReturnType<typeof createVideoRenderBackend>;
  confirm(value: { title: string; message: string; confirmLabel: string; danger: boolean }): Promise<boolean>;
  status(message: string, error?: boolean): void; fileManager: string;
  view: { open(): void; close(): void; read(): Draft; output(path: string): void; message(text: string): void; capabilities(error?: string): void; estimates(error?: string): void; history(): void; busy(value: boolean, canSubmit: boolean): void };
  repeat(callback: () => void): () => void;
};
/** A render tool visit owns requests and polling, never the backend worker. */
export function createVideoRenderSession(options: Options) {
  let visit: Context | null = null, generation = 0, disposed = false, submitting = false, picking = false;
  let capabilities: MediaCapabilities | null = null, estimates: VideoRenderEstimate[] = [], renders: VideoRender[] = [];
  let existingPath: string | null = null, stopPoll: (() => void) | null = null;
  let historyRequest: { ticket: number; promise: Promise<void> } | null = null;
  const cancelling = new Set<string>();
  const current = (ticket: number) => {
    const context = options.context();
    return !disposed && !!visit && generation === ticket && context?.jobId === visit.jobId && context.navigation === visit.navigation;
  };
  const canSubmit = () => current(generation) && !submitting && !picking && !!capabilities?.ready_for_hard_subtitles;
  const busy = () => options.view.busy(submitting || picking, canSubmit());
  const guard = () => {
    const context = options.context();
    if (!context || !options.available()) return false;
    if (options.dirty()) { options.status("请先保存各段尚未保存的修改，再烧录视频。", true); return false; }
    const staleCount = options.staleCount();
    if (staleCount) { options.status(`有 ${staleCount} 段译文已过期，请重译或修正后再烧录。`, true); return false; }
    if (!context.videoAvailable) { options.status("当前任务没有可烧录的视频源；如果原文件已移动，请先在字幕编辑中重新定位。", true); return false; }
    return true;
  };
  const refresh = () => {
    const ticket = generation;
    if (!current(ticket)) return Promise.resolve();
    if (historyRequest?.ticket === ticket) return historyRequest.promise;
    const jobId = visit!.jobId;
    const promise = (async () => {
      try { const result = await options.backend.list(jobId); if (current(ticket)) { renders = result; options.view.history(); } }
      catch (error) { if (current(ticket)) options.view.message(`无法读取烧录记录：${String(error)}`); }
      finally { if (historyRequest?.ticket === ticket) historyRequest = null; }
    })();
    historyRequest = { ticket, promise }; return promise;
  };
  const close = () => {
    if (disposed) return;
    generation++; visit = null; submitting = false; picking = false; existingPath = null; cancelling.clear();
    stopPoll?.(); stopPoll = null; historyRequest = null; busy(); options.view.close();
  };
  return {
    get capabilities() { return capabilities; }, get estimates() { return estimates; }, get renders() { return renders; },
    get submitting() { return submitting; }, get canSubmit() { return canSubmit(); },
    refresh,
    async open() {
      if (disposed || visit || !guard()) return;
      visit = { ...options.context()! }; const ticket = ++generation, jobId = visit.jobId;
      capabilities = null; estimates = []; renders = []; existingPath = null;
      options.view.output(""); options.view.message(""); options.view.open();
      options.view.capabilities(); options.view.estimates(); options.view.history(); busy();
      stopPoll = options.repeat(() => { if (current(ticket) && renders.some(r => r.status === "queued" || r.status === "running")) void refresh(); });
      await Promise.all([
        options.backend.capabilities().then(result => { if (current(ticket)) { capabilities = result; options.view.capabilities(); busy(); } }).catch(error => { if (current(ticket)) { capabilities = null; options.view.capabilities(`ffmpeg 检查失败：${String(error)}`); busy(); } }),
        options.backend.estimates(jobId).then(result => { if (current(ticket)) { estimates = result; options.view.estimates(); } }).catch(error => { if (current(ticket)) options.view.estimates(`无法读取视频规格：${String(error)}`); }),
        refresh(),
      ]);
    },
    outputChanged() { existingPath = null; },
    async chooseOutput() {
      const ticket = generation; if (!current(ticket) || submitting || picking) return;
      const path = visit!.inputPath, separator = path ? Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) : -1;
      picking = true; busy(); options.view.message("正在打开视频保存面板…");
      try {
        const selection = await options.backend.pick(path && separator > 0 ? path.slice(0, separator) : null, options.suggestedName());
        if (!current(ticket)) return;
        if (!selection) { options.view.message("已取消选择。"); return; }
        options.view.output(selection.path); existingPath = selection.alreadyExists ? selection.path : null;
        options.view.message(selection.alreadyExists ? "目标文件已经存在，开始前会再次确认覆盖。" : "已选择输出位置。");
      } catch (error) { if (current(ticket)) options.view.message(`无法选择输出位置：${String(error)}`); }
      finally { if (current(ticket)) { picking = false; busy(); } }
    },
    async submit() {
      const ticket = generation; if (!canSubmit() || cancelling.size || !guard()) return;
      const draft = { ...options.view.read() }; draft.outputPath = draft.outputPath.trim();
      if (!draft.outputPath.toLowerCase().endsWith(".mp4")) { options.view.message("输出路径必须以 .mp4 结尾。"); return; }
      submitting = true; busy();
      try {
        let overwriteExisting = false;
        if (existingPath === draft.outputPath) {
          overwriteExisting = await options.confirm({ title: "覆盖已有视频？", message: "目标视频已经存在。只有新视频烧录成功后，Atogaki 才会安全替换它。", confirmLabel: "烧录并替换", danger: true });
          if (!current(ticket)) return;
          if (!overwriteExisting) { options.view.message("已取消，现有视频不会被修改。"); return; }
        }
        if (!current(ticket) || !guard()) return;
        const latest = options.view.read();
        if (latest.outputPath.trim() !== draft.outputPath || latest.track !== draft.track || latest.quality !== draft.quality) { options.view.message("输出或烧录选项已变化，请重新确认后开始。"); return; }
        options.view.message("正在准备字幕并创建烧录任务…");
        await options.backend.submit({ sourceJobId: visit!.jobId, outputPath: draft.outputPath, subtitleTrack: draft.track, quality: draft.quality, overwriteExisting });
        if (!current(ticket)) return;
        existingPath = null; options.view.message("烧录任务已提交，可关闭窗口继续校对。"); await refresh();
      } catch (error) { if (current(ticket)) options.view.message(`提交失败：${String(error)}`); }
      finally { if (current(ticket)) { submitting = false; busy(); } }
    },
    async cancel(renderId: string) {
      const ticket = generation;
      if (!current(ticket) || submitting || cancelling.size || !renders.some(r => r.id === renderId && (r.status === "queued" || r.status === "running"))) return;
      cancelling.add(renderId);
      try {
        const accepted = await options.confirm({ title: "取消视频烧录？", message: "FFmpeg 会停止，临时输出会被清理；已完成的源任务和字幕不会改变。", confirmLabel: "取消烧录", danger: true });
        if (!accepted || !current(ticket) || !renders.some(r => r.id === renderId && (r.status === "queued" || r.status === "running"))) return;
        await options.backend.cancel(renderId);
        if (current(ticket)) { options.view.message("已请求取消，正在停止 FFmpeg 并清理临时文件。"); await refresh(); }
      } catch (error) { if (current(ticket)) options.view.message(`取消失败：${String(error)}`); }
      finally { if (current(ticket)) cancelling.delete(renderId); }
    },
    async reveal(path: string) {
      const ticket = generation;
      if (!current(ticket) || !renders.some(r => r.status === "done" && r.output_path === path)) return;
      try { await options.backend.reveal(path); }
      catch (error) { if (current(ticket)) options.view.message(`${options.fileManager} 定位失败：${String(error)}`); }
    },
    close,
    dispose() { if (!disposed) { close(); disposed = true; capabilities = null; estimates = []; renders = []; } },
  };
}
