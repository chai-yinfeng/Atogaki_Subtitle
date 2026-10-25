import type { createEditorBackend } from "./editor-backend";
import type { SubtitleSegment } from "./editor-types";

type Context = {
  jobId: string; navigation: number; selectedId: string | null;
  segments: readonly SubtitleSegment[]; sourceLanguage: string; targetLanguage: string;
};
type Options = {
  context(): Context | null;
  available(): boolean; dirty(): boolean; flush(): Promise<boolean>;
  lock(): void; positionMs(): number; time(ms: number): string;
  busy(value: boolean): void; status(message: string, error?: boolean): void;
  remember(before: SubtitleSegment[], after: SubtitleSegment[], label: string, jobId: string): void;
  apply(segments: SubtitleSegment[], selectedId: string): void;
  backend: Pick<ReturnType<typeof createEditorBackend>, "split" | "merge">;
};
function joinedText(left: string, right: string, language: string): string {
  const leftText = left.trim(), rightText = right.trim();
  if (!leftText) return rightText;
  if (!rightText) return leftText;
  const noSeparator = /\s$/.test(left) || /^\s/.test(right) || /^[，。！？、,.!?;；:：…]/.test(rightText);
  const separator = noSeparator || (language !== "en" && language !== "ko") ? "" : " ";
  return `${leftText}${separator}${rightText}`;
}

/** Owns Cut/Join preparation and view delivery. Rust retains the atomic write.
 * A committed request is never cancelled or replayed on navigation/dispose. */
export function createEditorStructureActions(options: Options) {
  let disposed = false, preparing = false, ownsBusy = false;
  const current = (context: Context) => {
    if (disposed) return null;
    const now = options.context();
    return now?.jobId === context.jobId && now.navigation === context.navigation ? now : null;
  };
  const release = () => {
    if (ownsBusy) { ownsBusy = false; options.busy(false); }
  };
  const run = async (kind: "cut" | "join") => {
    if (disposed || preparing || !options.available()) return;
    const opened = options.context();
    if (!opened || (kind === "join" && !opened.selectedId)) return;
    const boundary = kind === "cut" ? Math.round(options.positionMs() / 10) * 10 : 0;
    preparing = true;
    try {
      if (kind === "cut") options.lock();
      if (!await options.flush()) return;
      const context = current(opened);
      if (!context || context.selectedId !== opened.selectedId || !options.available()) return;
      if (options.dirty()) { options.status(`${kind === "cut" ? "Cut" : "Join"} 前请先保存或放弃当前文字修改。`, true); return; }
      const index = context.segments.findIndex(cue => cue.id === context.selectedId);
      const left = context.segments[index], right = context.segments[index + 1];
      if (kind === "cut" && (!left || boundary <= left.start_ms || boundary >= left.end_ms)) {
        options.status("播放头必须位于当前编辑段内部才能切开。", true); return;
      }
      if (!left || (kind === "join" && !right)) return;
      if (kind === "join" && left.end_ms !== right.start_ms) {
        options.status("两块之间存在空白；请保留为两个字幕块，或先独立修剪到同一边界。", true); return;
      }
      const before = context.segments.map(cue => ({ ...cue }));
      ownsBusy = true; options.busy(true);
      options.status(kind === "cut"
        ? `正在播放头 ${options.time(boundary)} 切开第 ${left.segment_index + 1} 段…`
        : `正在连接第 ${left.segment_index + 1}、${right.segment_index + 1} 段…`);
      const after = kind === "cut"
        ? await options.backend.split({ jobId: context.jobId, segmentId: left.id, boundaryMs: boundary,
          leftSourceText: left.source_text, rightSourceText: left.source_text,
          leftTranslatedText: left.translated_text, rightTranslatedText: left.translated_text })
        : await options.backend.merge({ jobId: context.jobId, leftSegmentId: left.id, rightSegmentId: right.id,
          sourceText: joinedText(left.source_text, right.source_text, context.sourceLanguage),
          translatedText: left.translated_text && right.translated_text
            ? joinedText(left.translated_text, right.translated_text, context.targetLanguage) : null });
      if (!current(context)) return;
      options.remember(before, after, kind === "cut" ? `第 ${left.segment_index + 1} 段 Cut` : `第 ${left.segment_index + 1}、${right.segment_index + 1} 段 Join`, context.jobId);
      options.apply(after, left.id);
      options.status(kind === "cut"
        ? "已在播放头切开；为保证连续播放观感，文字暂时复制到两块，请在当前编辑框继续整理。 "
        : "已连接相接的字幕块；文字已顺序拼接。 ");
    } catch (error) {
      if (current(opened)) options.status(`${kind === "cut" ? "切开" : "连接"}失败：${String(error)}`, true);
    } finally { release(); preparing = false; }
  };
  return {
    cut: () => run("cut"), join: () => run("join"),
    dispose() { if (disposed) return; disposed = true; release(); },
  };
}
