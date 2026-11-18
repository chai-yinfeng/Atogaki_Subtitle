type Caption = {
  start_ms: number;
  end_ms: number;
  source_text: string;
  translated_text: string | null;
};

function cueText(text: string): string {
  // VTTCue text uses WebVTT cue markup, even without a .vtt file.
  return text.trim().replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

function addCaption(track: TextTrack, start: number, end: number, text: string): void {
  if (!text) return;
  const cue = new VTTCue(start, end, text);
  cue.size = 90;
  cue.snapToLines = false;
  cue.line = 87;
  cue.lineAlign = "end";
  track.addCue(cue);
}

export function attachListeningVideoCaptions(
  video: HTMLVideoElement,
  captions: readonly Caption[],
  targetLanguage: string,
  signal: AbortSignal,
): void {
  if (typeof VTTCue !== "function" || !captions.length) return;
  const bilingual = video.addTextTrack("subtitles", "原文＋译文");
  const translation = video.addTextTrack("subtitles", "仅译文", targetLanguage);
  bilingual.mode = translation.mode = "hidden";
  for (const caption of captions) {
    const start = caption.start_ms / 1000;
    const end = caption.end_ms / 1000;
    if (!Number.isFinite(start) || !Number.isFinite(end) || start < 0 || end <= start) continue;
    const source = cueText(caption.source_text);
    const target = cueText(caption.translated_text ?? "");
    const both = [source, target].filter(Boolean).join("\n");
    addCaption(bilingual, start, end, both);
    addCaption(translation, start, end, target);
  }

  let nativeFullscreen = false;
  let fullscreen = false;
  const sync = () => {
    const full = nativeFullscreen || video.ownerDocument.fullscreenElement === video;
    if (full === fullscreen) return;
    fullscreen = full;
    translation.mode = "hidden";
    bilingual.mode = full ? "showing" : "hidden";
  };
  // WebKit's native media fullscreen does not use the document Fullscreen API.
  video.addEventListener("webkitbeginfullscreen", () => { nativeFullscreen = true; sync(); }, { signal });
  video.addEventListener("webkitendfullscreen", () => { nativeFullscreen = false; sync(); }, { signal });
  video.ownerDocument.addEventListener("fullscreenchange", sync, { signal });
}
