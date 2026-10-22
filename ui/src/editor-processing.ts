import type { SubtitleSegment } from "./editor-types";
import type { AsrQualitySignal, CandidateProgress, PreviewRequest, RetranscriptionPreview, ReviewDecision } from "./editor-processing-types";

type Context = { jobId: string; navigation: number };
type Phase = "preview" | "full" | "comparison" | "adopt";
type Ticket = Context & { phase: Phase; signalId: string | null };
type Backend = {
  preview(request: PreviewRequest): Promise<RetranscriptionPreview>;
  full(jobId: string): Promise<RetranscriptionPreview>;
  refresh(jobId: string, runId: string): Promise<RetranscriptionPreview>;
  reopen(jobId: string, runId: string): Promise<RetranscriptionPreview>;
  adopt(jobId: string, previewId: string, comparisonId: string | null): Promise<SubtitleSegment[]>;
  cancel(jobId: string): Promise<void>;
  progress(jobId: string): Promise<CandidateProgress | null>;
  signals(jobId: string): Promise<AsrQualitySignal[]>;
  review(signalId: string, decision: ReviewDecision): Promise<void>;
};
type Options = {
  backend: Backend;
  context(): Context | null;
  visible(): boolean;
  segments(): readonly SubtitleSegment[];
  busyChanged(busy: boolean, phase: Phase | null): void;
  mutationLock(locked: boolean): void;
  previewed(preview: RetranscriptionPreview): void;
  adopted(before: SubtitleSegment[], after: SubtitleSegment[], preview: RetranscriptionPreview, signalId: string | null): void;
  status(message: string, error?: boolean): void;
  comparisonStatus(message: string, error?: boolean): void;
  time(ms: number): string;
};
type Timers = { schedule(callback: () => void): number; cancel(id: number): void };
/** Own task identity, candidate operations and on-demand progress; no DOM or Tauri. */
export function createEditorProcessing(options: Options, timers: Timers = {
  schedule: callback => window.setTimeout(callback, 1000), cancel: id => window.clearTimeout(id),
}) {
  let active: Ticket | null = null, preview: RetranscriptionPreview | null = null;
  let timer: number | null = null, polling: Ticket | null = null;
  let disposed = false, epoch = 0, signalsRequest = 0;
  let previewSignalId: string | null = null;
  const contextMatches = (context: Context) => {
    const current = options.context();
    return !disposed && current?.jobId === context.jobId && current.navigation === context.navigation;
  };
  const current = (ticket: Ticket) => active === ticket && contextMatches(ticket);
  const stopTimer = () => { if (timer !== null) timers.cancel(timer); timer = null; };
  const finish = (ticket: Ticket) => {
    if (active !== ticket) return;
    active = null; stopTimer();
    if (ticket.phase === "adopt") options.mutationLock(false);
    options.busyChanged(false, null);
  };
  const progress = (ticket: Ticket) => {
    if (!current(ticket) || timer !== null || polling === ticket || !options.visible() || ticket.phase !== "preview" && ticket.phase !== "full") return;
    timer = timers.schedule(() => {
      timer = null;
      if (!current(ticket) || !options.visible()) return;
      polling = ticket;
      void options.backend.progress(ticket.jobId).then(value => {
        if (!value || !current(ticket) || !options.visible()) return;
        const phase = ({ extracting_audio: "提取音频", transcribing: "识别", refining_segments: "生成字幕候选", candidate_ready: "候选已就绪", failed: "候选失败" } as Record<string,string>)[value.phase] ?? value.phase;
        options.status(`${phase}${value.duration_ms ? ` · ${options.time(value.completed_ms ?? 0)} / ${options.time(value.duration_ms)}` : ""}`);
      }).catch(() => { /* Keep actionable result/error text. */ }).finally(() => {
        if (polling === ticket) polling = null;
        progress(ticket);
      });
    });
  };
  const reset = () => {
    epoch++; signalsRequest++; preview = null; previewSignalId = null;
    if (active) finish(active); else stopTimer();
  };
  const begin = (phase: Phase, signalId: string | null = null): Ticket | null => {
    const context = options.context(); if (disposed || !context) return null;
    if (active && !contextMatches(active)) reset();
    if (active) return null;
    const ticket = { ...context, phase, signalId }; active = ticket;
    if (phase === "adopt") options.mutationLock(true);
    options.busyChanged(true, phase); progress(ticket); return ticket;
  };
  const completePreview = async (ticket: Ticket, request: Promise<RetranscriptionPreview>) => {
    try {
      const result = await request;
      if (!current(ticket) || result.job_id !== ticket.jobId) return;
      finish(ticket);
      if (!options.visible()) return;
      preview = result; previewSignalId = ticket.signalId; options.previewed(result); options.status("候选已生成；关闭预览不会修改任何字幕。");
    } catch (error) {
      if (!current(ticket)) return;
      if (ticket.phase === "comparison") options.comparisonStatus(`无法读取候选对照：${String(error)}`, true);
      else options.status(`候选识别失败：${String(error)}`, true);
    } finally { finish(ticket); }
  };
  return {
    get busy() { return !!active && contextMatches(active); },
    get preview() { return preview; },
    get adopting() { return active?.phase === "adopt" && contextMatches(active); },
    async previewRange(request: Omit<PreviewRequest,"jobId">) {
      const ticket = begin("preview", request.qualitySignalId); if (!ticket) return;
      options.status(request.providerId === "gemini-transcribe" ? "正在提取所选范围、上传至 Gemini 并请求 word timestamps…" : "正在提取所选音频并用当前任务的模型和词表重新识别…");
      await completePreview(ticket, options.backend.preview({ ...request, jobId: ticket.jobId }));
    },
    async previewFull() {
      const ticket = begin("full"); if (!ticket) return;
      options.status("正在生成全片候选；不会覆盖当前工作区。");
      await completePreview(ticket, options.backend.full(ticket.jobId));
    },
    async reopen(runId: string) {
      const ticket = begin("comparison"); if (ticket) await completePreview(ticket, options.backend.reopen(ticket.jobId, runId));
    },
    async refresh() {
      const context = options.context(), selected = preview;
      if (!context || !selected || context.jobId !== selected.job_id) return;
      const ticket = begin("comparison", previewSignalId); if (ticket) await completePreview(ticket, options.backend.refresh(ticket.jobId, selected.preview_id));
    },
    async adopt() {
      const context = options.context(), selected = preview;
      if (!context || !selected || context.jobId !== selected.job_id) return;
      const ticket = begin("adopt", previewSignalId); if (!ticket) return;
      const before = options.segments().map(cue => ({ ...cue }));
      options.comparisonStatus("正在原子替换所选范围…");
      try {
        const after = await options.backend.adopt(ticket.jobId, selected.preview_id, selected.comparison_id ?? null);
        if (!current(ticket) || preview !== selected) return;
        preview = null; finish(ticket);
        options.adopted(before, after, selected, ticket.signalId);
      } catch (error) { if (current(ticket)) options.comparisonStatus(`替换失败：${String(error)}`, true); }
      finally { finish(ticket); }
    },
    async cancel() {
      const context = options.context(), generation = epoch; if (!context) return;
      try {
        await options.backend.cancel(context.jobId);
        if (contextMatches(context) && generation === epoch) options.status("已请求取消；云端请求会先完成上传文件清理。");
      } catch (error) { if (contextMatches(context) && generation === epoch) options.status(String(error), true); }
    },
    async signals(jobId: string): Promise<AsrQualitySignal[] | null> {
      const context = options.context(), request = ++signalsRequest, generation = epoch;
      if (!context || context.jobId !== jobId) return null;
      try {
        const result = await options.backend.signals(jobId);
        return contextMatches(context) && request === signalsRequest && generation === epoch ? result : null;
      } catch (error) {
        if (contextMatches(context) && request === signalsRequest && generation === epoch) throw error;
        return null;
      }
    },
    async review(signalId: string, decision: ReviewDecision): Promise<boolean> {
      const context = options.context(), generation = epoch; if (!context) return false;
      await options.backend.review(signalId, decision);
      return contextMatches(context) && generation === epoch;
    },
    clearPreview(runId: string) { if (!active && preview?.preview_id === runId) preview = null; },
    opened() { if (active) progress(active); },
    reset, dispose() { reset(); disposed = true; },
  };
}
