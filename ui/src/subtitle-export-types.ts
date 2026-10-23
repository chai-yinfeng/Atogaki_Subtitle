export type SubtitleExportArtifact = "source_srt" | "translated_srt" | "bilingual_srt" | "bilingual_ass";
export type SubtitleExport = {
  source_srt: string; translated_srt: string; bilingual_srt: string; bilingual_ass: string;
  missing_translation_count: number;
};
export type SubtitleExportPlan = {
  output_directory: string; base_name: string;
  source_srt: string; translated_srt: string; bilingual_srt: string; bilingual_ass: string;
  existing_files: string[];
};
