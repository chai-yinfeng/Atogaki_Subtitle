import type { SubtitleSegment } from "./editor-types";
import type { BatchTranslationResult } from "./subtitle-translation";

/** Existing wire contracts; services own translation and revision validation. */
export function createTranslationBackend<Detail>(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    single(jobId: string, segmentId: string) { return invoke<SubtitleSegment>("translate_subtitle", { jobId, segmentId }); },
    batch(jobId: string, onlyPending: boolean) { return invoke<BatchTranslationResult>("translate_all_subtitles", { jobId, onlyPending }); },
    load(jobId: string) { return invoke<Detail>("get_job_detail", { jobId }); },
  };
}
