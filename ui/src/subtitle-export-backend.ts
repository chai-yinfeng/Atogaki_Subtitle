import type { SubtitleExport, SubtitleExportArtifact, SubtitleExportPlan } from "./subtitle-export-types";

export function createSubtitleExportBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    pickDirectory(initialDirectory: string | null) {
      return invoke<string | null>("pick_subtitle_export_directory", { initialDirectory });
    },
    preview(jobId: string, outputDirectory: string, artifacts: SubtitleExportArtifact[]) {
      return invoke<SubtitleExportPlan>("preview_workspace_subtitle_export", { request: { jobId, outputDirectory, artifacts } });
    },
    export(jobId: string, outputDirectory: string, artifacts: SubtitleExportArtifact[], overwriteExisting: boolean) {
      return invoke<SubtitleExport>("export_workspace_subtitles", { request: { jobId, outputDirectory, overwriteExisting, artifacts } });
    },
    reveal(path: string) { return invoke<void>("reveal_exported_subtitle", { path }); },
  };
}
