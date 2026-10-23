import { needsTranslation } from "./subtitle-translation-state.ts";
import type { SubtitleSegment } from "./editor-types";

export type BatchTranslationResult = {
  segments: SubtitleSegment[]; requested_count: number; translated_count: number;
  empty_segment_ids: string[]; resumed: boolean;
};
export type TranslationContext = {
  jobId: string; navigation: number; segments: readonly SubtitleSegment[];
  contextStaleIds: readonly string[]; provider: string; configured: boolean;
  sourceLabel: string; targetLabel: string;
};
type Confirmation = { title: string; message: string; confirmLabel: string; danger: boolean };
type Options<Detail> = {
  context(): TranslationContext | null;
  available(): boolean; dirty(): boolean;
  flush(cue: SubtitleSegment): Promise<unknown>;
  confirm(options: Confirmation): Promise<boolean>;
  backend: {
    single(jobId: string, segmentId: string): Promise<SubtitleSegment>;
    batch(jobId: string, onlyPending: boolean): Promise<BatchTranslationResult>;
    load(jobId: string): Promise<Detail>;
  };
  acceptSegment(cue: SubtitleSegment): void;
  acceptSegments(cues: SubtitleSegment[]): void;
  acceptDetail(detail: Detail): void;
  status(message: string, error?: boolean): void;
  changed(): void;
  time(ms: number): string;
  elapsed(seconds: number): string;
  clock: { now(): number; repeat(tick: () => void): () => void };
};
/** Background translation may finish after navigation; only its originating
 * view can consume results. Persistence and revision checks remain in Rust. */
export function createSubtitleTranslation<Detail>(options: Options<Detail>) {
  type Ticket = { context: TranslationContext; message: string | null; startedAt: number };
  const activities = new Map<string, Ticket>();
  let disposed = false, stopTimer: (() => void) | null = null;
  const current = (ticket: Ticket) => {
    const ctx = options.context();
    return !disposed && activities.get(ticket.context.jobId) === ticket && ctx?.jobId === ticket.context.jobId && ctx.navigation === ticket.context.navigation;
  };
  const renderActivity = () => {
    const ctx = options.context(), ticket = ctx && activities.get(ctx.jobId);
    if (!disposed && ticket?.message) options.status(`${ticket.message} · 已运行 ${options.elapsed((options.clock.now() - ticket.startedAt) / 1000)}`);
  };
  const reserve = () => {
    const ctx = options.context();
    if (disposed || !ctx || !ctx.configured || !options.available() || activities.has(ctx.jobId)) return null;
    const ticket: Ticket = { context: { ...ctx }, message: null, startedAt: options.clock.now() };
    activities.set(ctx.jobId, ticket); options.changed(); return ticket;
  };
  const start = (ticket: Ticket, message: string) => {
    ticket.message = message; ticket.startedAt = options.clock.now(); renderActivity();
    if (!stopTimer) stopTimer = options.clock.repeat(renderActivity);
  };
  const finish = (ticket: Ticket) => {
    if (activities.get(ticket.context.jobId) !== ticket) return;
    activities.delete(ticket.context.jobId);
    if (![...activities.values()].some(item => item.message) && stopTimer) { stopTimer(); stopTimer = null; }
    if (!disposed) options.changed();
  };
  const reload = async (ticket: Ticket) => {
    if (!current(ticket)) return;
    const detail = await options.backend.load(ticket.context.jobId);
    if (current(ticket)) options.acceptDetail(detail);
  };
  return {
    has(jobId: string) { return activities.has(jobId); },
    renderActivity,
    async single(cue: SubtitleSegment) {
      const ctx = options.context();
      if (cue.job_id !== ctx?.jobId) return;
      const ticket = reserve(); if (!ticket) return;
      const { jobId, provider, targetLabel } = ticket.context;
      try {
        start(ticket, `正在保存并发送本段到 ${provider}`);
        await options.flush(cue);
        if (!current(ticket)) return;
        const translated = await options.backend.single(jobId, cue.id);
        if (!current(ticket)) return;
        options.acceptSegment(translated);
        try {
          await reload(ticket);
          if (current(ticket)) options.status(`本段${targetLabel}译文已由 ${provider} 更新并刷新。`);
        } catch (error) { if (current(ticket)) options.status(`翻译已经写入本地，但重新读取失败：${String(error)}`, true); }
      } catch (error) { if (current(ticket)) options.status(`翻译失败：${String(error)}`, true); }
      finally { finish(ticket); }
    },
    async batch(onlyPending: boolean) {
      const ticket = reserve(); if (!ticket) return;
      const { jobId, provider, sourceLabel, targetLabel, segments, contextStaleIds } = ticket.context;
      try {
        if (options.dirty()) { options.status("请先保存各段尚未保存的修改，再执行全部重译。", true); return; }
        const count = onlyPending ? segments.filter(cue => needsTranslation(cue, contextStaleIds)).length : segments.length;
        if (!count) return;
        const accepted = await options.confirm({
          title: onlyPending ? "继续翻译未完成字幕？" : "全部翻译／重译？",
          message: onlyPending ? `将只把 ${count} 段缺失或过期原文发送到 ${provider}；已经成功的译文保持不变。` : `将把 ${count} 段${sourceLabel}原文发送到 ${provider}，并覆盖现有${targetLabel}译文，包括人工修改。`,
          confirmLabel: onlyPending ? "继续未完成批次" : "发送并覆盖", danger: !onlyPending,
        });
        if (!accepted || !current(ticket)) return;
        if (options.dirty() || !options.available()) { options.status("字幕或任务状态已变化，请保存修改后重新确认翻译。", true); return; }
        start(ticket, `正在通过 ${provider}${onlyPending ? "继续" : ""}翻译 ${count} 段字幕；成功批次会立即保存`);
        const result = await options.backend.batch(jobId, onlyPending);
        if (!current(ticket)) return;
        options.acceptSegments(result.segments);
        try {
          await reload(ticket);
          if (!current(ticket)) return;
          if (result.empty_segment_ids.length) {
            const empty = new Set(result.empty_segment_ids);
            const labels = result.segments.map((cue, index) => ({ cue, index })).filter(({ cue }) => empty.has(cue.id)).map(({ cue, index }) => `第 ${index + 1} 段（${options.time(cue.start_ms)}）`);
            options.status(`已保存 ${result.translated_count}/${result.requested_count} 段；${labels.join("、")}在单独重试后仍为空，已保留为待翻译。`, true);
          } else options.status(`${result.resumed ? "续译" : "翻译"}完成：${result.translated_count} 段已按批次写入本地。`);
        } catch (error) { if (current(ticket)) options.status(`翻译批次已经写入本地，但重新读取失败：${String(error)}`, true); }
      } catch (error) {
        if (!current(ticket)) return;
        try {
          await reload(ticket);
          if (current(ticket)) options.status(`翻译在后续批次停止，之前成功的批次已经保留。${onlyPending ? "点击“补译缺失／过期段”可以继续。" : "未完成重译的段落保留原译文；补译只处理缺失／过期段，再次全体重译会重新处理所有段落。"}错误：${String(error)}`, true);
        } catch (reloadError) { if (current(ticket)) options.status(`翻译停止：${String(error)}；已完成批次可能已保存，但重新读取失败：${String(reloadError)}`, true); }
      } finally { finish(ticket); }
    },
    dispose() { disposed = true; activities.clear(); stopTimer?.(); stopTimer = null; },
  };
}
