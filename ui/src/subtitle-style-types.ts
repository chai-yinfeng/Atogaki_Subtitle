export type SubtitleStyle = {
  font_family: string;
  font_size: number;
  primary_color: string;
  outline_color: string;
  outline_width: number;
  shadow: number;
  background_color: string | null;
  bold: boolean;
  italic: boolean;
  alignment: number;
  margin_left: number;
  margin_right: number;
  margin_vertical: number;
  letter_spacing: number;
};

export type SubtitleStyleSet = { schema_version?: number; source: SubtitleStyle; translation: SubtitleStyle };
export type SubtitleFontFamily = { family: string; aliases: string[]; face_count: number; monospaced: boolean };
export type SubtitleFontCoverage = {
  requested_family: string;
  installed: boolean;
  matched_family: string | null;
  post_script_name: string | null;
  missing_characters: string[];
  needs_fallback: boolean;
};
export type SubtitleFontReport = { source: SubtitleFontCoverage; translation: SubtitleFontCoverage };
export type SubtitleStyleState = { styles: SubtitleStyleSet; font_report: SubtitleFontReport };
export type SubtitleStylePreview = {
  output_path: string;
  timestamp_ms: number;
  font_report: SubtitleFontReport;
  font_events: string[];
};

