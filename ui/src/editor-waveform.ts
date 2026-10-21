import type { WaveformWindow } from "./editor-types";
type Context = { jobId: string; navigation: number; durationMs: number; windowMs: number };
type Options = {
  context(): Context | null;
  points(): number;
  request(jobId: string, startMs: number, endMs: number, pointCount: number): Promise<WaveformWindow>;
  started(): void;
  accepted(waveform: WaveformWindow): void;
  failed(error: unknown): void;
};
type Timers = { schedule(callback: () => void): number; cancel(id: number): void };
/** Coalesce viewport requests and reject responses as soon as a new intent exists. */
export function createEditorWaveform(options: Options, timers: Timers = {
  schedule: callback => window.setTimeout(callback, 16), cancel: id => window.clearTimeout(id),
}) {
  let waveform: WaveformWindow | null = null;
  let loading = false, disposed = false, revision = 0, startMs = 0;
  let pending: { start: number; revision: number; context: Context } | null = null;
  let timer: number | null = null;
  const cancelPending = () => { if (timer !== null) timers.cancel(timer); timer = null; pending = null; };
  const matches = (request: number, context: Context) => {
    const current = options.context();
    return !disposed && request === revision && current?.jobId === context.jobId && current.navigation === context.navigation;
  };
  const fetchWindow = async (anchor: number, alignStart: boolean, request: number, context: Context) => {
    if (!matches(request, context)) return;
    const duration = waveform?.duration_ms || (Number.isFinite(context.durationMs) ? context.durationMs : 0);
    const desired = alignStart ? anchor : anchor - context.windowMs / 2;
    startMs = Math.max(0, duration > 0 ? Math.min(Math.round(desired), Math.max(0, duration - context.windowMs)) : Math.round(desired));
    loading = true;
    if (!waveform) options.started();
    try {
      const result = await options.request(context.jobId, startMs, startMs + context.windowMs, Math.max(240, Math.min(2400, Math.round(options.points()))));
      if (!matches(request, context)) return;
      waveform = result; startMs = result.start_ms; options.accepted(result);
    } catch (error) {
      if (!matches(request, context)) return;
      waveform = null; options.failed(error);
    } finally { if (request === revision) loading = false; }
  };
  const reset = () => { revision++; cancelPending(); waveform = null; loading = false; startMs = 0; };
  return {
    get window() { return waveform; }, get loading() { return loading; }, get startMs() { return startMs; },
    async load(anchor: number, alignStart = false) {
      if (disposed) return;
      const context = options.context(); if (!context) return;
      cancelPending(); await fetchWindow(anchor, alignStart, ++revision, context);
    },
    schedule(start: number) {
      if (disposed) return;
      const context = options.context(); if (!context) return;
      loading = false; startMs = start; pending = { start, context, revision: ++revision };
      if (timer !== null) return;
      timer = timers.schedule(() => {
        timer = null; const next = pending; pending = null;
        if (next) void fetchWindow(next.start, true, next.revision, next.context);
      });
    },
    reset, dispose() { disposed = true; reset(); },
  };
}
