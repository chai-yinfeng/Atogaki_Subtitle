import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import "./overlay.css";

type LanguageCode = "ja" | "en" | "ko" | "zh-Hans";

type SubtitleOverlayPayload = {
  sourceText: string;
  translatedText: string | null;
  sourceLanguage: LanguageCode;
  targetLanguage: LanguageCode;
};

const root = document.querySelector<HTMLElement>("#overlay-root");
if (!root) throw new Error("missing overlay root");
const overlayRoot = root;

root.innerHTML = `
  <section class="overlay-card" aria-live="polite">
    <header id="overlay-drag-handle" class="overlay-header">
      <span>Atogaki · 悬浮字幕</span>
      <button id="close-overlay" type="button" aria-label="关闭悬浮字幕">×</button>
    </header>
    <div class="subtitle-block">
      <span id="overlay-source-label" class="language-label">原文</span>
      <p id="overlay-source">正在等待播放位置…</p>
    </div>
    <div class="subtitle-block translation">
      <span id="overlay-translation-label" class="language-label">译文</span>
      <p id="overlay-translation">正在等待播放位置…</p>
    </div>
  </section>
`;

const sourceLabel = document.querySelector<HTMLSpanElement>("#overlay-source-label");
const sourceText = document.querySelector<HTMLParagraphElement>("#overlay-source");
const translationLabel = document.querySelector<HTMLSpanElement>("#overlay-translation-label");
const translationText = document.querySelector<HTMLParagraphElement>("#overlay-translation");
const overlayCard = document.querySelector<HTMLElement>(".overlay-card");

function updateOverlayScale(): void {
  if (!overlayCard) return;
  const viewportScale = Math.min(overlayRoot.clientWidth / 680, overlayRoot.clientHeight / 170);
  let scale = Math.min(2.5, Math.max(0.58, viewportScale));
  overlayRoot.style.setProperty("--overlay-scale", scale.toFixed(3));

  requestAnimationFrame(() => {
    const availableHeight = overlayRoot.clientHeight;
    if (overlayCard.scrollHeight > availableHeight) {
      scale = Math.max(0.45, scale * (availableHeight / overlayCard.scrollHeight));
      overlayRoot.style.setProperty("--overlay-scale", scale.toFixed(3));
    }
  });
}

new ResizeObserver(updateOverlayScale).observe(overlayRoot);
updateOverlayScale();

function languageLabel(language: LanguageCode): string {
  if (language === "ja") return "日语";
  if (language === "en") return "英语";
  if (language === "ko") return "韩语";
  return "简体中文";
}

function render(payload: SubtitleOverlayPayload | null): void {
  if (!payload) return;
  if (sourceLabel) sourceLabel.textContent = `${languageLabel(payload.sourceLanguage)}原文`;
  if (translationLabel) translationLabel.textContent = languageLabel(payload.targetLanguage);
  if (sourceText) sourceText.textContent = payload.sourceText || "当前时间没有字幕。";
  if (translationText) translationText.textContent = payload.translatedText || "尚无译文";
  updateOverlayScale();
}

document.querySelector<HTMLButtonElement>("#close-overlay")?.addEventListener("click", () => {
  void invoke("hide_subtitle_overlay");
});

void invoke<SubtitleOverlayPayload | null>("current_subtitle_overlay")
  .then(render)
  .catch(() => undefined);
void listen<SubtitleOverlayPayload>("subtitle-overlay-update", (event) => render(event.payload));
