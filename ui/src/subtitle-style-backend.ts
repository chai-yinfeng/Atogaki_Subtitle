import type { SubtitleStyleState, SubtitleStyleSet, SubtitleFontFamily, SubtitleStylePreview } from "./subtitle-style-types";
export function createSubtitleStyleBackend(invoke: <T>(command: string, args?: Record<string, unknown>) => Promise<T>) {
  return {
    load(jobId: string) { return invoke<SubtitleStyleState>("get_subtitle_style_state", { jobId }); },
    fonts() { return invoke<SubtitleFontFamily[]>("list_subtitle_fonts"); },
    save(jobId: string, styles: SubtitleStyleSet) { return invoke<SubtitleStyleState>("save_subtitle_styles", { request: { jobId, styles } }); },
    preview(jobId: string, styles: SubtitleStyleSet, subtitleTrack: string, timestampMs: number) {
      return invoke<SubtitleStylePreview>("preview_subtitle_styles", { request: { jobId, styles, subtitleTrack, timestampMs } });
    },
  };
}
