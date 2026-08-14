import { emitTo, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { playbackActionForKey, type PlaybackAction } from "./playback-shortcuts";
import "./overlay.css";

type LanguageCode = "ja" | "en" | "ko" | "zh-Hans";

type SubtitleOverlayPayload = {
  sourceText: string;
  translatedText: string | null;
  sourceLanguage: LanguageCode;
  targetLanguage: LanguageCode;
  playing: boolean;
  playbackRate: number;
};

const root = document.querySelector<HTMLElement>("#overlay-root");
if (!root) throw new Error("missing overlay root");
const overlayRoot = root;

root.tabIndex = 0;
root.setAttribute("aria-label", "悬浮字幕播放控制");
root.innerHTML = `
  <section class="overlay-card" aria-live="polite">
    <header id="overlay-drag-handle" class="overlay-header">
      <span>Atogaki · 悬浮字幕</span>
      <button id="close-overlay" type="button" aria-label="关闭悬浮字幕">×</button>
    </header>
    <nav class="overlay-controls" aria-label="悬浮字幕播放操作">
      <button type="button" data-playback-action="previous-subtitle" title="上一句（[）">上一句</button>
      <button type="button" data-playback-action="rewind" title="后退 5 秒（← 或 J）">−5s</button>
      <button id="overlay-toggle-playback" type="button" data-playback-action="toggle-playback" title="播放／暂停（空格或 K）">播放</button>
      <button type="button" data-playback-action="forward" title="前进 5 秒（→ 或 L）">+5s</button>
      <button type="button" data-playback-action="next-subtitle" title="下一句（]）">下一句</button>
      <button type="button" data-playback-action="slower" title="减速（,）">慢</button>
      <span id="overlay-playback-rate">1×</span>
      <button type="button" data-playback-action="faster" title="加速（.）">快</button>
    </nav>
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
const togglePlaybackButton = document.querySelector<HTMLButtonElement>("#overlay-toggle-playback");
const playbackRate = document.querySelector<HTMLSpanElement>("#overlay-playback-rate");

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
  if (togglePlaybackButton) togglePlaybackButton.textContent = payload.playing ? "暂停" : "播放";
  if (playbackRate) playbackRate.textContent = `${payload.playbackRate}×`;
  updateOverlayScale();
}

function sendPlaybackAction(action: PlaybackAction): void {
  void emitTo("main", "subtitle-overlay-playback-action", { action });
}

overlayRoot.addEventListener("pointerdown", (event) => {
  if (!(event.target instanceof HTMLButtonElement)) overlayRoot.focus({ preventScroll: true });
});

overlayRoot.addEventListener("keydown", (event) => {
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  const action = playbackActionForKey(event);
  if (!action || (event.repeat && (action === "toggle-playback" || action === "toggle-overlay"))) return;
  sendPlaybackAction(action);
  event.preventDefault();
});

document.querySelectorAll<HTMLButtonElement>("[data-playback-action]").forEach((button) => {
  button.addEventListener("click", () => sendPlaybackAction(button.dataset.playbackAction as PlaybackAction));
});

document.querySelector<HTMLButtonElement>("#close-overlay")?.addEventListener("click", () => {
  void invoke("hide_subtitle_overlay");
});

void invoke<SubtitleOverlayPayload | null>("current_subtitle_overlay")
  .then(render)
  .catch(() => undefined);
void listen<SubtitleOverlayPayload>("subtitle-overlay-update", (event) => render(event.payload));
