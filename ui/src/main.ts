import { mountVideoRenderView } from "./video-render-view";
import { createVideoRenderSession } from "./video-render-session";
import { createVideoRenderBackend } from "./video-render-backend";
import { mountSubtitleStyleView } from "./subtitle-style-view";
import { createSubtitleStyleSession } from "./subtitle-style-session";
import { createSubtitleStyleBackend } from "./subtitle-style-backend";
import { createSubtitleTranslation } from "./subtitle-translation";
import { createTaskTranslationProvider, type TranslationStatus } from "./task-translation-provider";
import { createTaskProviderBackend } from "./task-translation-provider-backend";
import { mountTaskProviderView } from "./task-translation-provider-view";
import { translationUsageLabel } from "./subtitle-translation-usage";
import { needsTranslation, staleTranslationCount } from "./subtitle-translation-state";
import { createTranslationBackend } from "./subtitle-translation-backend";
import { createSubtitleExport } from "./subtitle-export";
import { createSubtitleExportBackend } from "./subtitle-export-backend";
import { mountSubtitleExportDialogs } from "./subtitle-export-view";
import { mountCandidateCleanupConfirmation } from "./candidate-cleanup-confirmation";
import { mountReviewSignals } from "./editor-review-signals";
import { mountCandidateHistory } from "./editor-candidate-history";
import { createEditorProcessing } from "./editor-processing";
import { createProcessingBackend } from "./editor-processing-backend";
import type { RetranscriptionCandidate, RetranscriptionPreview, AsrQualitySignal } from "./editor-processing-types";
import { createEditorWaveform } from "./editor-waveform";
import { createMediaPlayer } from "./media-player";
import type { TimingGesture } from "./timeline-timing";
import { mountTimelineGestures } from "./editor-timeline-gestures";
import { mountEditorTimeline } from "./editor-timeline-view";
import { mountEditorFollow } from "./editor-follow";
import { mountEditorMenus } from "./editor-menus";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { setupImageThemes } from "./appearance";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SubtitleDrafts, cueKey, type Draft } from "./subtitle-drafts";
import { listen } from "@tauri-apps/api/event";
import { playbackActionForKey, type PlaybackAction } from "./playback-shortcuts";
import "./styles.css";
import "./editor.css";
import { editorTemplate } from "./editor-template";
import { EditorSession, mergeDraggedCueTiming, type CueFilter } from "./editor-session";
import { mountEditorLayout } from "./editor-layout";
import { mountEditorCueList } from "./editor-cue-list";
import { mountCurrentCueEditor } from "./current-cue-editor";
import { mountTaskTools, mountToolDialog } from "./task-tools";
import { taskToolsTemplate } from "./task-tools-template";
import { createEditorBackend } from "./editor-backend";
import type { SubtitleSegment } from "./editor-types";
import { SubtitleTimeline } from "./subtitle-timeline";
import { executionSeconds, earlyTranslationLabel, type ProcessingTime } from "./job-progress";
import { applyTheme, readTheme, candidateLabel } from "./themes";
applyTheme(readTheme());
import { matchesQuery, highlightSaved, candidateTextDiff } from "./offline-tools";
let selectedMediaFiles: string[] = [];
let playbackLoop: {startMs:number;endMs:number;repeat:boolean} | null = null;
let loopStartMs: number | null = null;
let taskTitleQuery = "";
let learningQuery = "";
let learningTaskFilter = "";

type LanguageCode = "ja" | "en" | "ko" | "zh-Hans";
type LearningLanguage = "ja" | "en" | "ko";

type RecoverySummary = { phase: string; completed_ms: number; duration_ms: number; completed_ranges: [number, number][]; can_resume: boolean; blocked_reason: string | null; early_translation?: { translated_cues: number; stable_through_ms: number; error: string | null } | null };
type LocalJob = {
  processing_time?: ProcessingTime | null;
  recovery?: RecoverySummary;
  recovery_progress_unverified?: boolean;
  job_id: string;
  display_name: string | null;
  storage_dir: string;
  input_path: string | null;
  status: string;
  message: string;
  error_message: string | null;
  glossary_id: string | null;
  glossary_name: string | null;
  glossary_snapshot_path: string | null;
  source_language: LanguageCode;
  target_language: LanguageCode;
  created_at_unix: number;
  started_at_unix: number | null;
  completed_at_unix: number | null;
  updated_at_unix: number;
  sort_position: number | null;
  translation_status: "not_ready" | "untranslated" | "partial" | "translated" | "stale";
  segment_count: number;
  translated_segment_count: number;
  stale_translation_count: number;
};

type Glossary = {
  id: string;
  name: string;
  source_language: LanguageCode;
  builtin_key: string | null;
  builtin_version: string | null;
  term_count: number;
  prompt_term_count: number;
  correction_count: number;
  core_term_count: number;
  content_term_count: number;
  correction_only_count: number;
  content_group_count: number;
  created_at_unix: number;
  updated_at_unix: number;
};

type GlossaryTerm = {
  id: string;
  glossary_id: string;
  source_text: string;
  target_text: string | null;
  prompt_scope: "core" | "content" | "correction_only";
  content_group: string | null;
};

type GlossaryPromptPreview = {
  glossary_id: string;
  glossary_name: string;
  available_content_groups: string[];
  selected_content_groups: string[];
  core_term_count: number;
  selected_content_term_count: number;
  correction_only_count: number;
  included_prompt_term_count: number;
  prompt_character_count: number;
  prompt: string | null;
};

type GlossaryDetail = {
  glossary: Glossary;
  terms: GlossaryTerm[];
};

type GlossaryPreview = {
  glossary_id: string;
  glossary_name: string;
  changes: Array<{
    segment_id: string;
    segment_index: number;
    before_text: string;
    after_text: string;
    translation_will_be_stale: boolean;
  }>;
};

type GlossaryApplyResult = {
  changed_segments: number;
  stale_translations: number;
  segments: SubtitleSegment[];
};



type SubtitleUndoEntry = {
  before: SubtitleSegment;
  afterFingerprint: string;
};

type SubtitleStructureUndoEntry = {
  before: SubtitleSegment[];
  after: SubtitleSegment[];
  label: string;
};

type JobDetail = {
  translation_context_stale_ids: string[];
  job: LocalJob;
  segments: SubtitleSegment[];
  translation_runs: TranslationRun[];
  playback_path: string | null;
  audio_fallback_path: string | null;
};

type LearningOccurrence = {
  id: string;
  learning_item_id: string;
  job_id: string | null;
  segment_id: string | null;
  job_display_name_snapshot: string;
  segment_source_snapshot: string;
  segment_translation_snapshot: string | null;
  selection_start_utf16: number;
  selection_end_utf16: number;
  start_ms: number;
  end_ms: number;
  created_at_unix: number;
};

type LearningLookupSense = {
  part_of_speech: string | null;
  definitions: string[];
  examples: string[];
};

type LearningLookupResult = {
  id: string;
  learning_item_id: string;
  provider_id: string;
  provider_name: string;
  headword: string;
  reading: string | null;
  pronunciation: string | null;
  audio_url: string | null;
  senses: LearningLookupSense[];
  attribution_text: string;
  source_url: string | null;
  license_label: string | null;
  data_version: string | null;
  fetched_at_unix: number;
  cache_expires_at_unix: number | null;
};

type LearningItemDetail = {
  item: {
    id: string;
    source_language: LanguageCode;
    target_language: LanguageCode;
    item_type: "selection" | "sentence";
    source_text: string;
    meaning_text: string | null;
    meaning_provider_id: string | null;
    meaning_source_label: string | null;
    occurrence_count: number;
    created_at_unix: number;
    updated_at_unix: number;
  };
  occurrences: LearningOccurrence[];
  lookup_results: LearningLookupResult[];
};

type LearningProviderOption = {
  id: string;
  name: string;
  kind: "summary" | "offline" | "api";
};

type PendingLearningSelection = {
  jobId: string;
  segmentId: string;
  selectedText: string;
  selectionStartUtf16: number;
  selectionEndUtf16: number;
};

type TranslationRun = {
  id: string;
  provider_id: string;
  provider_name: string;
  model: string | null;
  endpoint_kind: string;
  segment_count: number;
  input_tokens: number | null;
  output_tokens: number | null;
  plan_json?: string | null;
  completed_at_unix: number;
};

type DesktopSettings = {
  onboardingCompleted: boolean;
  needsOnboarding: boolean;
  whisperModelPath: string | null;
  whisperModelReady: boolean;
  vadModelPath: string | null;
  vadModelReady: boolean;
  hyMt2ModelPath: string | null;
  hyMt2ModelReady: boolean;
  localTranslationRuntimeReady: boolean;
  translationProviderId: "none" | "deepl" | "deepseek" | "openai-compatible" | "hy-mt2-local";
  translationModel: string | null;
  translationBaseUrl: string | null;
  translationStyleInstruction: string;
  translationApiKeyConfigured: boolean;
  translationApiKeySource: "system" | "environment" | "saved" | "deferred" | "local" | null;
  credentialStore: string;
  credentialError: string | null;
  modelsDirectory: string;
  networkProxyMode: "environment" | "direct" | "custom";
  networkProxyUrl: string | null;
  modelMirrorUrl: string | null;
};

type TranslationCredentialCheck = {
  providerId: string;
  providerName: string;
  storedInSystem: boolean;
  availableFromEnvironment: boolean;
  credentialStore: string;
};

type AsrCredentialStatus = {
  providerId: "gemini-transcribe";
  providerName: string;
  configured: boolean;
  credentialStore: string;
};

type ModelCatalogItem = {
  id: string;
  kind: "whisper" | "vad" | "hy-mt2";
  name: string;
  fileName: string;
  sizeLabel: string;
  recommendedFor: string;
  sourceUrl: string;
};

type ModelDownloadState = {
  modelId: string;
  status: "queued" | "downloading" | "cancelling" | "cancelled" | "done" | "failed";
  downloadedBytes: number;
  totalBytes: number | null;
  path: string | null;
  error: string | null;
  source: string | null;
  active: boolean;
};

type DictionaryCatalogItem = {
  id: string;
  name: string;
  languagePair: string;
  versionLabel: string;
  sizeLabel: string;
  description: string;
  license: string;
  attribution: string;
  sourceUrl: string;
};

type DictionaryDownloadState = {
  dictionaryId: string;
  status: "queued" | "resolving" | "downloading" | "done" | "failed";
  downloadedBytes: number;
  totalBytes: number | null;
  path: string | null;
  version: string | null;
  error: string | null;
  source: string | null;
};

type DictionaryCredentialStatus = {
  providerId: "merriam-webster";
  providerName: string;
  configured: boolean;
  credentialStore: string;
};

type NetworkSourceCheck = {
  label: string;
  requestedUrl: string;
  resolvedHost: string | null;
  status: number | null;
  ok: boolean;
  error: string | null;
};

type SubtitleOverlayPayload = {
  sourceText: string;
  translatedText: string | null;
  sourceLanguage: LanguageCode;
  targetLanguage: LanguageCode;
  playing: boolean;
  playbackRate: number;
};

type WorkspaceSection = "review" | "translation" | "export";
type TopLevelArea = "workbench" | "listening" | "learning" | "karaoke" | "workspace";

const desktopPlatform = navigator.userAgent.includes("Windows")
  ? "windows"
  : navigator.userAgent.includes("Macintosh")
    ? "macos"
    : "other";
const fileManagerLabel = desktopPlatform === "macos"
  ? "Finder"
  : desktopPlatform === "windows"
    ? "Explorer"
    : "文件管理器";
const primaryShortcutModifier = desktopPlatform === "macos" ? "⌘" : "Ctrl+";
const credentialCheckHint = desktopPlatform === "macos"
  ? "macOS 可能请求解锁 Keychain"
  : desktopPlatform === "windows"
    ? "将读取 Windows Credential Manager"
    : "将读取系统凭据库";

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing app root");

app.innerHTML = `
  <main class="shell">
    <header>
      <div>
        <p class="eyebrow">LOCAL MEDIA LANGUAGE WORKSPACE</p>
        <h1>Atogaki</h1>
      </div>
      <div class="header-actions">
        <nav class="primary-navigation" aria-label="主要区域">
          <button id="show-workbench" type="button" class="secondary active">工作台</button>
          <button id="show-listening" type="button" class="secondary">收听</button>
          <button id="show-learning" type="button" class="secondary">学习</button>
        </nav>
        <button id="open-settings" type="button" class="secondary">设置</button>
        <button id="refresh" type="button">刷新任务</button>
      </div>
    </header>
    <div id="home-view">
      <section class="intro">
        <h2>离线理解，保留在本机。</h2>
        <p>选择节目语言、本地媒体与 Whisper 模型，保存任务后即可在后台转写。</p>
        <p class="data-path" id="data-path">正在读取本地数据目录…</p>
      </section>
      <section class="create-task" aria-labelledby="create-heading">
        <div class="section-heading"><h2 id="create-heading">新建转写任务</h2><span>本地执行</span></div>
        <form id="task-form">
          <label>节目语言<select id="source-language"><option value="ja">日语</option><option value="en">英语</option><option value="ko">韩语</option></select></label>
          <label>翻译目标<select id="target-language" disabled><option value="zh-Hans">简体中文</option></select></label>
          <label>媒体文件<input id="media-path" required placeholder="选择文件，或直接粘贴完整路径" /></label>
          <button id="choose-media" type="button" class="secondary">选择媒体</button>
          <label>Whisper 模型<input id="model-path" required placeholder="选择文件，或直接粘贴完整路径" /></label>
          <button id="choose-model" type="button" class="secondary">选择模型</button>
          <div class="vad-setting">
            <label><input id="vad-enabled" type="checkbox" checked />启用语音活动检测（推荐）</label>
            <span>先过滤静音、音乐和环境声，再交给 Whisper；使用 Silero VAD 默认参数。</span>
          </div>
          <label>Silero VAD 模型<input id="vad-model-path" required placeholder="选择 ggml-silero-*.bin，或直接粘贴完整路径" /></label>
          <button id="choose-vad-model" type="button" class="secondary">选择 VAD 模型</button>
          <label class="task-translation-setting">翻译方式<select id="task-translation-provider" required><option value="" disabled selected>请选择翻译方式</option><option value="none">仅转录</option><option value="deepseek">DeepSeek · 云端文本翻译</option><option value="gemini">Gemini · 云端文本翻译</option><option value="deepl">DeepL · 云端文本翻译</option><option value="hy-mt2-local">Hy-MT2 · 本地翻译</option><option value="openai-compatible">自定义兼容服务</option></select><span>云端随稳定原文提前翻译，本地在完整转录后翻译；云端只发送字幕文本和上下文。</span></label>
          <label>识别词表<select id="task-glossary"><option value="">不使用词表</option></select></label>
          <button id="manage-glossaries" type="button" class="secondary">管理词表</button>
          <div id="task-glossary-configuration" class="task-glossary-configuration hidden">
            <div class="content-pack-heading"><div><strong>当前内容包</strong><span>核心词条始终提示；只选择本期可能出现的作品。</span></div><span id="task-prompt-summary"></span></div>
            <div id="task-content-packs" class="task-content-packs"></div>
            <details open><summary>最终 Whisper prompt</summary><pre id="task-prompt-preview">正在生成预览…</pre></details>
          </div>
          <div class="form-footer"><span id="task-message" role="status"></span><button id="submit-task" type="submit">开始转写</button></div>
        </form>
      </section>
      <section class="jobs" aria-labelledby="jobs-heading">
        <div class="section-heading">
          <h2 id="jobs-heading">最近任务</h2>
          <span id="job-count"></span>
        </div>
        <p id="job-management-message" class="job-management-message" role="status"></p>
        <div id="job-list" class="job-list" aria-live="polite"></div>
      </section>
    </div>
    <section id="listening-view" class="listening-view hidden" aria-labelledby="listening-title">
      <div class="module-heading listening-heading">
        <div><p class="eyebrow">LISTENING LIBRARY</p><h2 id="listening-title">收听</h2><p>阅读全文并跟随播放；选择一段原文即可收藏词语、短语或语法表达。</p></div>
      </div>
      <div class="listening-layout">
        <aside class="listening-library" aria-label="可收听任务">
          <div class="section-heading"><h3>已翻译节目</h3><span id="listening-job-count"></span></div>
          <div id="listening-job-list" class="listening-job-list"></div>
        </aside>
        <section class="listening-player">
          <div class="panel-heading compact">
            <div><p class="eyebrow">NOW PLAYING</p><h3 id="listening-job-title">选择一个节目开始收听</h3></div>
            <button id="open-subtitle-overlay" type="button" class="secondary">打开悬浮字幕</button>
          </div>
          <div id="listening-media-host" class="media-host"></div>
          <div class="playback-tools" aria-label="收听快捷操作">
            <button id="listening-previous-subtitle" type="button" class="secondary">上一句</button>
            <button id="listening-rewind-media" type="button" class="secondary">−5 秒</button>
            <button id="listening-toggle-playback" type="button">播放</button>
            <button id="listening-forward-media" type="button" class="secondary">+5 秒</button>
            <button id="listening-next-subtitle" type="button" class="secondary">下一句</button>
            <label>速度<select id="listening-playback-rate"><option value="0.5">0.5×</option><option value="0.75">0.75×</option><option value="1" selected>1×</option><option value="1.25">1.25×</option><option value="1.5">1.5×</option><option value="2">2×</option></select></label>
          </div>
          <p class="shortcut-help">快捷键：空格/K 播放暂停 · ←/J 与 →/L 跳 5 秒 · [ ] 切换字幕 · , . 调整速度 · O 开关悬浮字幕</p>
          <p id="listening-media-message" class="media-message">从左侧选择一个已翻译任务。</p>
          <div class="current-caption" aria-live="polite">
            <p id="listening-current-source">原文字幕</p>
            <p id="listening-current-translation">简体中文翻译</p>
          </div>
          <div id="listening-subtitle-list" class="listening-subtitle-list" aria-label="节目全文"></div>
          <div id="listening-selection-menu" class="selection-menu hidden" role="dialog" aria-label="收藏所选原文">
            <div><span>已选择</span><strong id="listening-selection-text"></strong></div>
            <p id="listening-selection-message">可先收藏，之后在学习区补充标准译义。</p>
            <div class="selection-menu-actions">
              <button id="save-learning-selection" type="button">收藏词语／语法</button>
              <button id="save-learning-sentence" type="button" class="secondary">收藏整句</button>
              <button id="close-learning-selection" type="button" class="secondary">取消</button>
            </div>
          </div>
        </section>
      </div>
    </section>
    <section id="learning-view" class="learning-view hidden" aria-labelledby="learning-title">
      <div class="module-heading learning-heading">
        <div><p class="eyebrow">LEARNING LIBRARY</p><h2 id="learning-title">学习</h2><p>保存词语、短语、语法表达与整句，在原节目语境中反复回听。</p></div>
        <button id="refresh-learning" type="button" class="secondary">刷新单词本</button>
      </div>
      <div class="learning-language-bar">
        <nav id="learning-language-tabs" class="learning-language-tabs" aria-label="选择单词本语言">
          <button type="button" class="secondary" data-learning-language="ja">日语单词本 <span data-learning-language-count="ja">0</span></button>
          <button type="button" class="secondary" data-learning-language="en">英语单词本 <span data-learning-language-count="en">0</span></button>
          <button type="button" class="secondary" data-learning-language="ko">韩语单词本 <span data-learning-language-count="ko">0</span></button>
        </nav>
        <span id="learning-language-summary"></span>
      </div>
      <p id="learning-message" class="learning-message" role="status"></p>
      <div id="learning-item-list" class="learning-item-list"></div>
    </section>
${editorTemplate(primaryShortcutModifier)}
${taskToolsTemplate(fileManagerLabel)}
    <dialog id="glossary-dialog" class="glossary-dialog">
      <div class="dialog-heading">
        <div><p class="eyebrow">RECOGNITION GLOSSARIES</p><h2>识别词表</h2></div>
        <button id="close-glossaries" type="button" class="secondary">关闭</button>
      </div>
      <p class="dialog-help"><strong>核心</strong>始终进入 prompt；<strong>内容包</strong>只在新建任务时选中后进入；<strong>仅修正</strong>只在识别后规范化。核心和内容包都可以同时带“读音 → 规范写法”。</p>
      <div class="glossary-manager">
        <aside>
          <button id="new-glossary" type="button">＋ 新建词表</button>
          <div id="glossary-list" class="glossary-list"></div>
        </aside>
        <section class="glossary-editor">
          <label>词表语言<select id="glossary-language"><option value="ja">日语</option><option value="en">英语</option><option value="ko">韩语</option></select></label>
          <label>词表名称<input id="glossary-name" maxlength="80" placeholder="例如：电台常用词" /></label>
          <div class="term-heading"><strong>提示词与识别修正规则</strong><button id="add-glossary-term" type="button" class="secondary">＋ 添加词条</button></div>
          <div id="glossary-terms" class="glossary-terms"></div>
          <div class="glossary-editor-footer">
            <span id="glossary-message" role="status"></span>
            <div><button id="delete-glossary" type="button" class="danger">删除</button><button id="save-glossary" type="button">保存词表</button></div>
          </div>
        </section>
      </div>
    </dialog>
    <dialog id="learning-dictionary-dialog" class="learning-dictionary-dialog">
      <div class="dialog-heading">
        <div><p class="eyebrow">DICTIONARY REFERENCES</p><h2 id="learning-dictionary-title">词典详情</h2></div>
        <button id="close-learning-dictionary" type="button" class="secondary">关闭</button>
      </div>
      <p id="learning-dictionary-subtitle" class="dialog-help"></p>
      <nav id="learning-provider-tabs" class="learning-provider-tabs" aria-label="词典来源"></nav>
      <section id="learning-provider-panel" class="learning-provider-panel" aria-live="polite"></section>
    </dialog>
    <dialog id="rename-job-dialog" class="rename-job-dialog">
      <form id="rename-job-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">TASK NAME</p><h2>重命名任务</h2></div>
          <button id="cancel-rename-job" type="button" class="secondary">取消</button>
        </div>
        <label>任务名称<input id="rename-job-input" maxlength="100" autocomplete="off" /></label>
        <p>名称只用于列表显示；留空会恢复为媒体文件名。</p>
        <div class="rename-job-footer"><span id="rename-job-message" role="status"></span><button type="submit">保存名称</button></div>
      </form>
    </dialog>
    <dialog id="confirmation-dialog" class="rename-job-dialog confirmation-dialog">
      <form id="confirmation-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">CONFIRM ACTION</p><h2 id="confirmation-title">确认操作</h2></div>
        </div>
        <p id="confirmation-message" class="confirmation-message"></p>
        <div class="confirmation-actions">
          <button id="cancel-confirmation" type="button" class="secondary">取消</button>
          <button id="accept-confirmation" type="submit">继续</button>
        </div>
      </form>
    </dialog>
    <dialog id="retranscription-dialog" class="retranscription-dialog">
      <div class="dialog-heading">
        <div><p class="eyebrow">ASR CANDIDATE PREVIEW</p><h2>识别候选对照</h2></div>
        <button id="close-retranscription-preview" type="button" class="secondary">关闭</button>
      </div>
      <p id="retranscription-preview-summary" class="dialog-help"></p>
      <div class="retranscription-comparison">
        <section><h3>当前字幕</h3><div id="retranscription-original-list" class="retranscription-list"></div></section>
        <section><h3>新识别候选</h3><div id="retranscription-candidate-list" class="retranscription-list"></div></section>
      </div>
      <p>确认后只替换所示范围；新字幕没有译文，需要重新翻译。范围外字幕和人工修改保持不变。</p>
      <div class="confirmation-actions"><button id="cancel-retranscription" type="button" class="secondary">保留当前字幕</button><button id="confirm-retranscription" type="button">用候选替换</button></div>
      <p id="retranscription-confirm-message" role="status"></p>
    </dialog>
    <dialog id="glossary-correction-dialog" class="rename-job-dialog glossary-correction-dialog">
      <form id="glossary-correction-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">ADD CORRECTION</p><h2>修正加入词表</h2></div>
          <button id="cancel-glossary-correction" type="button" class="secondary">取消</button>
        </div>
        <label>常见误识别<input id="glossary-correction-source" maxlength="200" autocomplete="off" required /></label>
        <label>规范写法<input id="glossary-correction-target" maxlength="200" autocomplete="off" required /></label>
        <p>规则只用于识别后的文本规范化，不会进入 Whisper prompt；当前字幕修改仍需单独保存。</p>
        <div class="rename-job-footer"><span id="glossary-correction-message" role="status"></span><button type="submit">加入词表</button></div>
      </form>
    </dialog>
    <dialog id="settings-dialog" class="settings-dialog">
      <form id="settings-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">SETTINGS</p><h2>启动配置</h2></div>
          <button id="close-settings" type="button" class="secondary">稍后再说</button>
        </div>
        <p class="dialog-help">媒体和模型默认保留在本机；选择云端翻译 provider 时，只有原文字幕和上下文会发送到对应服务。</p>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>1. 本地模型</strong><span id="models-directory"></span></div><span id="model-readiness"></span></div>
          <div class="settings-path-row">
            <label>Whisper 模型<input id="settings-whisper-model" placeholder="选择已有 ggml-*.bin，或从下方下载" /></label>
            <button id="settings-choose-whisper" type="button" class="secondary">选择文件</button>
          </div>
          <div class="settings-path-row">
            <label>Silero VAD 模型（推荐）<input id="settings-vad-model" placeholder="选择已有 ggml-silero-*.bin，或从下方下载" /></label>
            <button id="settings-choose-vad" type="button" class="secondary">选择文件</button>
          </div>
          <div class="settings-path-row">
            <label>Hy-MT2 模型<input id="settings-hy-mt2-model" placeholder="选择已有 Hy-MT2 *.gguf，或从下方下载" /></label>
            <button id="settings-choose-hy-mt2" type="button" class="secondary">选择文件</button>
          </div>
        </section>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>2. 网络与模型下载</strong><span>代理同时用于模型下载与所选云端翻译；镜像只用于模型下载，失败会回退官方源。</span></div><span>每个模型都会校验 SHA-256</span></div>
          <div class="network-settings-grid">
            <label>代理模式
              <select id="settings-proxy-mode">
                <option value="environment">跟随启动环境</option>
                <option value="direct">直连（忽略系统与环境代理）</option>
                <option value="custom">自定义 HTTP 代理</option>
              </select>
            </label>
            <label id="proxy-url-field">HTTP 代理地址
              <input id="settings-proxy-url" inputmode="url" placeholder="http://127.0.0.1:7897" />
            </label>
            <label>Hugging Face 镜像根地址（可选）
              <input id="settings-model-mirror" inputmode="url" placeholder="https://hf-mirror.com" />
            </label>
          </div>
          <div class="network-test-row"><button id="test-network" type="button" class="secondary">测试当前网络配置</button><span>测试只检查当前输入，不会切换或保存模式；修改后请点击“保存配置”。该代理也用于云端翻译。</span></div>
          <div id="network-test-results" class="network-test-results" role="status"></div>
          <div id="model-catalog" class="model-catalog"></div>
          <p id="model-download-message" class="settings-message" role="status"></p>
        </section>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>3. 云端 ASR（可选）</strong><span>Gemini Key 只存入系统凭据库。配置 Key 不会授权上传；每次运行仍需单独确认音频范围。</span></div><span id="asr-credential-store-label"></span></div>
          <p>与翻译区 Gemini 共用同一把 Key；保存或删除会同时影响两项功能。音频上传仍需逐次授权。</p><label>Gemini API Key<input id="settings-gemini-api-key" type="password" autocomplete="off" placeholder="从 Google AI Studio 创建；保存后立即清空" /></label>
          <div class="credential-check-row"><button id="save-gemini-api-key" type="button">保存 Key</button><button id="check-gemini-api-key" type="button" class="secondary">检查</button><button id="clear-gemini-api-key" type="button" class="secondary">删除</button></div>
          <p id="gemini-api-key-status" class="settings-message">尚未读取配置。</p>
          <p class="settings-message">Atogaki 上传提取后的 WAV，并在转录请求完成后调用 Files API 删除上传文件。免费层价格为 $0，但存在项目级 rate limits，且免费层数据会用于改进 Google 产品。</p>
        </section>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>4. 翻译 provider（可选）</strong><span>Hy-MT2 完全离线运行；云端 provider 只发送原文字幕和上下文。</span></div><span id="credential-store-label"></span></div>
          <label>翻译 provider
            <select id="settings-provider">
              <option value="hy-mt2-local">Hy-MT2（本地翻译）</option>
              <option value="deepl">DeepL（传统翻译 API）</option>
              <option value="deepseek">DeepSeek（LLM API）</option><option value="gemini">Gemini（LLM API）</option>
              <option value="openai-compatible">OpenAI-compatible（高级）</option>
              <option value="none">不翻译／仅保留原文</option>
            </select>
          </label>
          <div id="llm-provider-fields" class="settings-grid hidden">
            <label id="provider-base-url-field">OpenAI-compatible Base URL
              <input id="settings-provider-base-url" type="url" placeholder="https://api.openai.com/v1" />
            </label>
            <label id="provider-model-field">模型
              <input id="settings-provider-model" type="text" placeholder="模型 ID" />
            </label>
            <label class="settings-span">翻译风格
              <input id="settings-translation-style" type="text" placeholder="准确、自然的简体中文口语字幕" />
            </label>
          </div>
          <label id="api-key-field"><span id="api-key-label">DeepL API Key</span>
            <input id="settings-api-key" type="password" autocomplete="off" placeholder="留空则保持当前密钥" />
          </label>
          <label class="clear-secret"><input id="settings-clear-api-key" type="checkbox" /><span id="clear-api-key-label">删除系统凭据库中已保存的 DeepL Key</span></label>
          <div class="credential-check-row"><button id="check-api-key" type="button" class="secondary">检查所选 Key</button><span>仅检查系统凭据条目，不会回显 Key 或调用翻译 API。</span></div>
          <p id="api-key-status" class="settings-message"></p>
        </section>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>5. 学习词典</strong><span>离线包由你明确点击后下载到正式应用数据目录；商业词典的 Key 分来源保存。</span></div><span id="dictionary-directory"></span></div>
          <div>
            <strong class="settings-subheading">离线词典包</strong>
            <p class="settings-message">JMdict 与 Tomoshi 用于日语；ECDICT 基础数据用于英语。ECDICT 属于社区汇总数据，下载会复用上方代理设置并校验固定版本摘要。</p>
          </div>
          <div id="dictionary-catalog" class="model-catalog"></div>
          <p id="dictionary-download-message" class="settings-message" role="status"></p>
          <div>
            <strong class="settings-subheading">在线词典 API</strong>
            <p class="settings-message">每个来源独立配置；Key 仅保存在系统凭据库，不写入日志或任务目录。检查只验证凭据是否存在，不调用词典 API。</p>
          </div>
          <div id="dictionary-credentials" class="dictionary-credentials">
            ${[
              ["merriam-webster", "Merriam-Webster", "保留英英来源标签页；正式展示需满足 Logo 与署名要求。"],
            ].map(([id, name, hint]) => `<article class="dictionary-credential-card" data-dictionary-provider="${id}">
              <div><strong>${name}</strong><span>${hint}</span></div>
              <input type="password" autocomplete="off" data-dictionary-key="${id}" aria-label="${name} API Key" placeholder="粘贴 API Key；保存后立即清空" />
              <div class="model-card-action"><span data-dictionary-status="${id}">尚未读取配置</span><div><button type="button" class="secondary" data-check-dictionary-key="${id}">检查</button><button type="button" data-save-dictionary-key="${id}">保存</button><button type="button" class="secondary" data-clear-dictionary-key="${id}">删除</button></div></div>
            </article>`).join("")}
          </div>
          <p id="dictionary-credential-message" class="settings-message" role="status"></p>
        </section>
        <div class="settings-footer">
          <span id="settings-message" role="status"></span>
          <div><button id="save-settings" type="submit" class="secondary">保存</button><button id="finish-settings" type="button">保存并开始使用</button></div>
        </div>
      </form>
    </dialog>
  </main>
`;

const homeView = document.querySelector<HTMLDivElement>("#home-view");
const listeningView = document.querySelector<HTMLElement>("#listening-view");
const learningView = document.querySelector<HTMLElement>("#learning-view");
const karaokeView = document.querySelector<HTMLElement>("#karaoke-view");
const shell = document.querySelector<HTMLElement>(".shell");
const workspaceView = document.querySelector<HTMLElement>("#workspace-view")!;
const workspaceSectionTabs = Array.from(document.querySelectorAll<HTMLButtonElement>("[data-workspace-section]"));
const workspaceSectionPanels = Array.from(document.querySelectorAll<HTMLElement>("[data-workspace-panel]"));
const openSubtitleEditorButton = workspaceView.querySelector<HTMLButtonElement>("#open-subtitle-editor");
const jobList = document.querySelector<HTMLDivElement>("#job-list");
const jobCount = document.querySelector<HTMLSpanElement>("#job-count");
const jobManagementMessage = document.querySelector<HTMLParagraphElement>("#job-management-message");
const dataPath = document.querySelector<HTMLParagraphElement>("#data-path");
const karaokeJobTitle = document.querySelector<HTMLHeadingElement>("#karaoke-job-title");
const karaokeOpenWorkspaceButton = document.querySelector<HTMLButtonElement>("#karaoke-open-workspace");
const karaokeMediaHost = document.querySelector<HTMLDivElement>("#karaoke-media-host");
const karaokeMediaMessage = document.querySelector<HTMLParagraphElement>("#karaoke-media-message");
const karaokeCurrentTime = document.querySelector<HTMLElement>("#karaoke-current-time");
const karaokeTogglePlaybackButton = document.querySelector<HTMLButtonElement>("#karaoke-toggle-playback");
const karaokePlaybackRateSelect = document.querySelector<HTMLSelectElement>("#karaoke-playback-rate");
const karaokeZoomInput = document.querySelector<HTMLInputElement>("#karaoke-zoom");
const karaokeZoomLabel = document.querySelector<HTMLOutputElement>("#karaoke-zoom-label");
const karaokeWaveformGainInput = document.querySelector<HTMLInputElement>("#karaoke-waveform-gain");
const karaokeWaveformGainLabel = document.querySelector<HTMLOutputElement>("#karaoke-waveform-gain-label");
const karaokeWaveformStatus = document.querySelector<HTMLDivElement>("#karaoke-waveform-status");
const karaokeWaveform = document.querySelector<HTMLCanvasElement>("#karaoke-waveform");
const karaokeSegmentTime = document.querySelector<HTMLSpanElement>("#karaoke-segment-time");
const karaokeCurrentSource = document.querySelector<HTMLTextAreaElement>("#karaoke-current-source");
const karaokeUndoTextButton = document.querySelector<HTMLButtonElement>("#karaoke-undo-text");
const karaokeCutSegmentButton = document.querySelector<HTMLButtonElement>("#karaoke-cut-segment");
const karaokeJoinSegmentButton = document.querySelector<HTMLButtonElement>("#karaoke-join-segment");
const karaokeTimingMessage = document.querySelector<HTMLParagraphElement>("#karaoke-timing-message");
const retranscriptionStartInput = document.querySelector<HTMLInputElement>("#retranscription-start");
const retranscriptionEndInput = document.querySelector<HTMLInputElement>("#retranscription-end");
const retranscriptionProvider = document.querySelector<HTMLSelectElement>("#retranscription-provider");
const authorizeGeminiUpload = document.querySelector<HTMLInputElement>("#authorize-gemini-upload");
const retranscriptionUseWindowButton = document.querySelector<HTMLButtonElement>("#retranscription-use-window");
const retranscriptionExpandSegmentsButton = document.querySelector<HTMLButtonElement>("#retranscription-expand-segments");
const previewRetranscriptionButton = document.querySelector<HTMLButtonElement>("#preview-retranscription");
const retranscriptionMessage = document.querySelector<HTMLParagraphElement>("#retranscription-message");
const mediaPath = document.querySelector<HTMLInputElement>("#media-path");
const modelPath = document.querySelector<HTMLInputElement>("#model-path");
const sourceLanguage = document.querySelector<HTMLSelectElement>("#source-language");
const vadEnabled = document.querySelector<HTMLInputElement>("#vad-enabled");
const vadModelPath = document.querySelector<HTMLInputElement>("#vad-model-path");
const taskGlossary = document.querySelector<HTMLSelectElement>("#task-glossary");
const taskGlossaryConfiguration = document.querySelector<HTMLDivElement>("#task-glossary-configuration");
const taskContentPacks = document.querySelector<HTMLDivElement>("#task-content-packs");
const taskPromptSummary = document.querySelector<HTMLSpanElement>("#task-prompt-summary");
const taskPromptPreview = document.querySelector<HTMLElement>("#task-prompt-preview");
const taskMessage = document.querySelector<HTMLSpanElement>("#task-message");
const submitButton = document.querySelector<HTMLButtonElement>("#submit-task");
const workspaceTitle = workspaceView.querySelector<HTMLHeadingElement>("#workspace-title");
const workspaceMessage = workspaceView.querySelector<HTMLParagraphElement>("#workspace-message");
const mediaHost = workspaceView.querySelector<HTMLDivElement>("#media-host");
const mediaMessage = workspaceView.querySelector<HTMLParagraphElement>("#media-message");
const relinkJobMediaButton = document.querySelector<HTMLButtonElement>("#relink-job-media");
const previousSubtitleButton = workspaceView.querySelector<HTMLButtonElement>("#previous-subtitle");
const rewindMediaButton = workspaceView.querySelector<HTMLButtonElement>("#rewind-media");
const togglePlaybackButton = workspaceView.querySelector<HTMLButtonElement>("#toggle-playback");
const forwardMediaButton = workspaceView.querySelector<HTMLButtonElement>("#forward-media");
const nextSubtitleButton = workspaceView.querySelector<HTMLButtonElement>("#next-subtitle");
const playbackRateSelect = workspaceView.querySelector<HTMLSelectElement>("#playback-rate");
const subtitleList = workspaceView.querySelector<HTMLDivElement>("#subtitle-list");
const segmentCount = workspaceView.querySelector<HTMLSpanElement>("#segment-count");
const currentSource = workspaceView.querySelector<HTMLParagraphElement>("#current-source");
const currentTranslation = workspaceView.querySelector<HTMLParagraphElement>("#current-translation");
const listeningJobList = document.querySelector<HTMLDivElement>("#listening-job-list");
const listeningJobCount = document.querySelector<HTMLSpanElement>("#listening-job-count");
const listeningJobTitle = document.querySelector<HTMLHeadingElement>("#listening-job-title");
const listeningMediaHost = document.querySelector<HTMLDivElement>("#listening-media-host");
const listeningMediaMessage = document.querySelector<HTMLParagraphElement>("#listening-media-message");
const listeningSubtitleList = document.querySelector<HTMLDivElement>("#listening-subtitle-list");
const listeningCurrentSource = document.querySelector<HTMLParagraphElement>("#listening-current-source");
const listeningCurrentTranslation = document.querySelector<HTMLParagraphElement>("#listening-current-translation");
const listeningPreviousSubtitleButton = document.querySelector<HTMLButtonElement>("#listening-previous-subtitle");
const listeningRewindMediaButton = document.querySelector<HTMLButtonElement>("#listening-rewind-media");
const listeningTogglePlaybackButton = document.querySelector<HTMLButtonElement>("#listening-toggle-playback");
const listeningForwardMediaButton = document.querySelector<HTMLButtonElement>("#listening-forward-media");
const listeningNextSubtitleButton = document.querySelector<HTMLButtonElement>("#listening-next-subtitle");
const listeningPlaybackRateSelect = document.querySelector<HTMLSelectElement>("#listening-playback-rate");
const listeningSelectionMenu = document.querySelector<HTMLDivElement>("#listening-selection-menu");
const listeningSelectionText = document.querySelector<HTMLElement>("#listening-selection-text");
const listeningSelectionMessage = document.querySelector<HTMLParagraphElement>("#listening-selection-message");
const saveLearningSelectionButton = document.querySelector<HTMLButtonElement>("#save-learning-selection");
const saveLearningSentenceButton = document.querySelector<HTMLButtonElement>("#save-learning-sentence");
const learningItemList = document.querySelector<HTMLDivElement>("#learning-item-list");
const learningMessage = document.querySelector<HTMLParagraphElement>("#learning-message");
const learningLanguageTabs = document.querySelector<HTMLElement>("#learning-language-tabs");
const learningLanguageSummary = document.querySelector<HTMLElement>("#learning-language-summary");
const learningDictionaryDialog = document.querySelector<HTMLDialogElement>("#learning-dictionary-dialog");
const learningDictionaryTitle = document.querySelector<HTMLHeadingElement>("#learning-dictionary-title");
const learningDictionarySubtitle = document.querySelector<HTMLParagraphElement>("#learning-dictionary-subtitle");
const learningProviderTabs = document.querySelector<HTMLElement>("#learning-provider-tabs");
const learningProviderPanel = document.querySelector<HTMLElement>("#learning-provider-panel");
const translationStatusText = workspaceView.querySelector<HTMLSpanElement>("#translation-status");
const translationRunStatusText = workspaceView.querySelector<HTMLSpanElement>("#translation-run-status");
const translatePendingButton = workspaceView.querySelector<HTMLButtonElement>("#translate-pending");
const translateAllButton = workspaceView.querySelector<HTMLButtonElement>("#translate-all");
const openSubtitleOverlayButton = document.querySelector<HTMLButtonElement>("#open-subtitle-overlay");
const exportButton = workspaceView.querySelector<HTMLButtonElement>("#export-subtitles");
const renderVideoButton = workspaceView.querySelector<HTMLButtonElement>("#render-video");
const editSubtitleStylesButton = workspaceView.querySelector<HTMLButtonElement>("#edit-subtitle-styles");
const revealExportButton = workspaceView.querySelector<HTMLButtonElement>("#reveal-export");
const workspaceActionMessage = workspaceView.querySelector<HTMLParagraphElement>("#workspace-action-message");
const workspaceGlossary = workspaceView.querySelector<HTMLSelectElement>("#workspace-glossary");
const jobGlossaryStatus = workspaceView.querySelector<HTMLSpanElement>("#job-glossary-status");
const workspaceGlossaryMappings = workspaceView.querySelector<HTMLDivElement>("#workspace-glossary-mappings");
const jobGlossarySnapshot = workspaceView.querySelector<HTMLElement>("#job-glossary-snapshot");
const glossaryPreviewHost = workspaceView.querySelector<HTMLDivElement>("#glossary-preview");
const glossaryDialog = document.querySelector<HTMLDialogElement>("#glossary-dialog");
const glossaryListHost = document.querySelector<HTMLDivElement>("#glossary-list");
const glossaryName = document.querySelector<HTMLInputElement>("#glossary-name");
const glossaryLanguage = document.querySelector<HTMLSelectElement>("#glossary-language");
const glossaryTerms = document.querySelector<HTMLDivElement>("#glossary-terms");
const glossaryMessage = document.querySelector<HTMLSpanElement>("#glossary-message");
const deleteGlossaryButton = document.querySelector<HTMLButtonElement>("#delete-glossary");
const saveGlossaryButton = document.querySelector<HTMLButtonElement>("#save-glossary");
const renameJobDialog = document.querySelector<HTMLDialogElement>("#rename-job-dialog");
const renameJobForm = document.querySelector<HTMLFormElement>("#rename-job-form");
const renameJobInput = document.querySelector<HTMLInputElement>("#rename-job-input");
const renameJobMessage = document.querySelector<HTMLSpanElement>("#rename-job-message");
const confirmationDialog = document.querySelector<HTMLDialogElement>("#confirmation-dialog");
const retranscriptionDialog = document.querySelector<HTMLDialogElement>("#retranscription-dialog");
const retranscriptionPreviewSummary = document.querySelector<HTMLParagraphElement>("#retranscription-preview-summary");
const retranscriptionOriginalList = document.querySelector<HTMLDivElement>("#retranscription-original-list");
const retranscriptionCandidateList = document.querySelector<HTMLDivElement>("#retranscription-candidate-list");
const retranscriptionConfirmMessage = document.querySelector<HTMLParagraphElement>("#retranscription-confirm-message");
const confirmRetranscriptionButton = document.querySelector<HTMLButtonElement>("#confirm-retranscription");
const confirmationForm = document.querySelector<HTMLFormElement>("#confirmation-form");
const confirmationTitle = document.querySelector<HTMLHeadingElement>("#confirmation-title");
const confirmationMessage = document.querySelector<HTMLParagraphElement>("#confirmation-message");
const acceptConfirmationButton = document.querySelector<HTMLButtonElement>("#accept-confirmation");
const undoSubtitleStructureButton = document.querySelector<HTMLButtonElement>("#undo-subtitle-structure");
const glossaryCorrectionDialog = document.querySelector<HTMLDialogElement>("#glossary-correction-dialog");
const glossaryCorrectionForm = document.querySelector<HTMLFormElement>("#glossary-correction-form");
const glossaryCorrectionSource = document.querySelector<HTMLInputElement>("#glossary-correction-source");
const glossaryCorrectionTarget = document.querySelector<HTMLInputElement>("#glossary-correction-target");
const glossaryCorrectionMessage = document.querySelector<HTMLSpanElement>("#glossary-correction-message");
const settingsDialog = document.querySelector<HTMLDialogElement>("#settings-dialog");
const settingsForm = document.querySelector<HTMLFormElement>("#settings-form");
const settingsWhisperModel = document.querySelector<HTMLInputElement>("#settings-whisper-model");
const settingsVadModel = document.querySelector<HTMLInputElement>("#settings-vad-model");
const settingsHyMt2Model = document.querySelector<HTMLInputElement>("#settings-hy-mt2-model");
const settingsProxyMode = document.querySelector<HTMLSelectElement>("#settings-proxy-mode");
const settingsProxyUrl = document.querySelector<HTMLInputElement>("#settings-proxy-url");
const settingsModelMirror = document.querySelector<HTMLInputElement>("#settings-model-mirror");
const testNetworkButton = document.querySelector<HTMLButtonElement>("#test-network");
const networkTestResults = document.querySelector<HTMLDivElement>("#network-test-results");
const settingsProvider = document.querySelector<HTMLSelectElement>("#settings-provider");
const settingsProviderBaseUrl = document.querySelector<HTMLInputElement>("#settings-provider-base-url");
const settingsProviderModel = document.querySelector<HTMLInputElement>("#settings-provider-model");
const settingsTranslationStyle = document.querySelector<HTMLInputElement>("#settings-translation-style");
const settingsApiKey = document.querySelector<HTMLInputElement>("#settings-api-key");
const settingsClearApiKey = document.querySelector<HTMLInputElement>("#settings-clear-api-key");
const checkApiKeyButton = document.querySelector<HTMLButtonElement>("#check-api-key");
const settingsMessage = document.querySelector<HTMLSpanElement>("#settings-message");
const apiKeyStatus = document.querySelector<HTMLParagraphElement>("#api-key-status");
const credentialStoreLabel = document.querySelector<HTMLSpanElement>("#credential-store-label");
const asrCredentialStoreLabel = document.querySelector<HTMLSpanElement>("#asr-credential-store-label");
const settingsGeminiApiKey = document.querySelector<HTMLInputElement>("#settings-gemini-api-key");
const saveGeminiApiKeyButton = document.querySelector<HTMLButtonElement>("#save-gemini-api-key");
const checkGeminiApiKeyButton = document.querySelector<HTMLButtonElement>("#check-gemini-api-key");
const clearGeminiApiKeyButton = document.querySelector<HTMLButtonElement>("#clear-gemini-api-key");
const geminiApiKeyStatus = document.querySelector<HTMLParagraphElement>("#gemini-api-key-status");
const modelCatalogHost = document.querySelector<HTMLDivElement>("#model-catalog");
const modelDownloadMessage = document.querySelector<HTMLParagraphElement>("#model-download-message");
const modelsDirectory = document.querySelector<HTMLSpanElement>("#models-directory");
const modelReadiness = document.querySelector<HTMLSpanElement>("#model-readiness");
const dictionaryCatalogHost = document.querySelector<HTMLDivElement>("#dictionary-catalog");
const dictionaryDirectoryLabel = document.querySelector<HTMLSpanElement>("#dictionary-directory");
const dictionaryDownloadMessage = document.querySelector<HTMLParagraphElement>("#dictionary-download-message");
const dictionaryCredentialMessage = document.querySelector<HTMLParagraphElement>("#dictionary-credential-message");

let refreshing = false;
let jobOrderBusy = false;
let latestJobs: LocalJob[] = [];
let currentArea: TopLevelArea = "workbench";
let renderedJobsFingerprint: string | null = null;
let activeDetail: JobDetail | null = null;
const mediaPlayer = createMediaPlayer({
  currentJob: id => activeDetail?.job.job_id === id,
  resolve: convertFileSrc, isAudio: isAudioPath, time: formatTime,
  position: updateActiveSubtitle, progress: schedulePlaybackPositionSave,
  changed: updatePlaybackControls, persist: () => void persistPlaybackPosition(),
  frameEnabled: () => currentArea === "karaoke",
});
let navigationRequestId = 0;
let activeSegmentId: string | null = null;
let subtitleOverlayVisible = false;
let lastSubtitleOverlayKey = "";
const subtitleUndoHistory = new Map<string, SubtitleUndoEntry[]>();
const editorBackend = createEditorBackend(invoke);
const subtitleDrafts = new SubtitleDrafts<SubtitleSegment>(editorBackend.updateText, updateSubtitleDraftUI);

const subtitleStructureUndoHistory = new Map<string, SubtitleStructureUndoEntry[]>();
let workspaceActionBusy = false;
let glossaries: Glossary[] = [];
let editingGlossaryId: string | null = null;
let pendingGlossaryPreview: GlossaryPreview | null = null;
let renamingJob: LocalJob | null = null;
let confirmationResolver: ((confirmed: boolean) => void) | null = null;
let pendingGlossaryCorrection: {
  glossaryId: string;
  state: HTMLElement;
} | null = null;
let taskGlossaryConfigurationId: string | null = null;
let selectedTaskContentGroups = new Set<string>();
let desktopSettings: DesktopSettings | null = null;
let availableModels: ModelCatalogItem[] = [];
let modelDownloads: ModelDownloadState[] = [];
let modelDownloadPoll: number | null = null;
let availableDictionaries: DictionaryCatalogItem[] = [];
let dictionaryDownloads: DictionaryDownloadState[] = [];
let dictionaryCredentials: DictionaryCredentialStatus[] = [];
let dictionaryDownloadPoll: number | null = null;
const settingsDirtyFields = new Set<string>();
let learningItems: LearningItemDetail[] = [];
let activeLearningLanguage: LearningLanguage = readLearningLanguagePreference();
let pendingLearningSelection: PendingLearningSelection | null = null;
let activeLearningItemId: string | null = null;
let activeLearningProviderId = "summary";
let learningActionBusy = false;
let learningLookupBusy = false;
let playbackPositionSaveTimer: number | null = null;
let lastPlaybackPositionSavedAt = 0;
let karaokeFollowPlayhead = true;
let karaokeSelectedJobId: string | null = null;
let karaokeResumeMs = 0;
let karaokeWaveformGain = 1;
let asrQualitySignals: AsrQualitySignal[] = [];
let activeRepairSignalId: string | null = null;
let currentWorkspaceSection: WorkspaceSection = "translation";
type SubtitleFollowState = { userScrollingUntil: number; autoScrollingUntil: number; resumeTimer: number | null };
const subtitleFollowStates = new WeakMap<HTMLElement, SubtitleFollowState>();
let translationStatus: TranslationStatus = {
  provider_id: "none",
  provider: "翻译服务",
  configured: false,
  model: null,
  endpoint_kind: "none",
  configuration_hint: "请在设置中选择并配置翻译服务。",
};
let taskProviderView: ReturnType<typeof mountTaskProviderView> | null = null;
const taskProvider = createTaskTranslationProvider({
  context: () => ({ jobId: activeDetail?.job.job_id ?? null, navigation: navigationRequestId }),
  backend: createTaskProviderBackend(invoke),
  loading: () => {
    translationStatus = { ...translationStatus, configured: false, configuration_hint: "正在读取翻译配置…" };
    updateTranslationControls();
  },
  accepted: status => { translationStatus = status; updateTranslationControls(); },
  changed: () => taskProviderView?.render(),
  message: (text, error) => {
    taskProviderView?.message(text, error);
    if (currentArea === "karaoke" && workspaceView.classList.contains("hidden")) setSubtitleEditAction(text, error);
  },
});
taskProviderView = mountTaskProviderView(workspaceView.querySelector<HTMLElement>("[data-task-provider]")!, taskProvider);

const editorSession = new EditorSession();
const videoRenderView = mountVideoRenderView(shell ?? document.body, {
  context: () => activeDetail ? { name: displayName(activeDetail.job), source: languageLabel(activeSourceLanguage()), target: languageLabel(activeTargetLanguage()) } : null,
  platform: desktopPlatform, fileManager: fileManagerLabel,
});
const videoRenderSession = createVideoRenderSession({
  context: () => activeDetail ? { jobId: activeDetail.job.job_id, navigation: navigationRequestId,
    inputPath: activeDetail.job.input_path, videoAvailable: !!activeDetail.job.input_path && !!activeDetail.playback_path && !isAudioPath(activeDetail.job.input_path) } : null,
  staleCount: () => activeDetail ? staleTranslationCount(activeDetail.segments, activeDetail.translation_context_stale_ids) : 0,
  available: () => !workspaceActionBusy, dirty: hasUnsavedSubtitleEdits, suggestedName: videoRenderView.suggestedName,
  backend: createVideoRenderBackend(invoke), confirm: videoRenderView.confirm, status: setWorkspaceAction, fileManager: fileManagerLabel,
  repeat: tick => { const timer = window.setInterval(tick, 1000); return () => window.clearInterval(timer); },
  view: videoRenderView,
});
videoRenderView.bind(videoRenderSession, renderVideoButton);
const subtitleStyleView = mountSubtitleStyleView(shell ?? document.body, {
  context: () => activeDetail ? { segments: activeDetail.segments, sourceLanguage: activeDetail.job.source_language } : null,
  fileUrl: convertFileSrc, time: formatTime,
});
const subtitleStyles = createSubtitleStyleSession({
  context: () => activeDetail ? { jobId: activeDetail.job.job_id, navigation: navigationRequestId, timestampMs: (mediaPlayer.media?.currentTime ?? 0) * 1000 } : null,
  available: () => !workspaceActionBusy,
  backend: createSubtitleStyleBackend(invoke),
  later: (callback, delay) => { const timer = window.setTimeout(callback, delay); return () => window.clearTimeout(timer); },
  view: subtitleStyleView,
});
subtitleStyleView.bind(subtitleStyles, editSubtitleStylesButton);
const subtitleTranslation = createSubtitleTranslation<JobDetail>({
  context: () => activeDetail && (currentArea === "karaoke" || currentArea === "workspace") ? {
    jobId: activeDetail.job.job_id, navigation: navigationRequestId, segments: activeDetail.segments,
    contextStaleIds: activeDetail.translation_context_stale_ids, provider: translationStatus.provider,
    configured: translationStatus.configured, sourceLabel: languageLabel(activeSourceLanguage()), targetLabel: languageLabel(activeTargetLanguage()),
  } : null,
  available: () => !workspaceActionBusy, dirty: hasUnsavedSubtitleEdits,
  flush: cue => subtitleDrafts.flush(cueKey(cue)), confirm: confirmAction,
  backend: createTranslationBackend<JobDetail>(invoke),
  acceptSegment: cue => {
    replaceActiveSegment(cue);
    if (activeDetail) renderSubtitleListPreservingView(activeDetail.segments, cue.id);
    updateActiveSubtitle((mediaPlayer.media?.currentTime ?? 0) * 1_000);
  },
  acceptSegments: cues => {
    if (!activeDetail) return;
    activeDetail.segments = cues; renderSubtitleListPreservingView(cues);
    updateActiveSubtitle((mediaPlayer.media?.currentTime ?? 0) * 1_000);
  },
  acceptDetail: detail => {
    if (!activeDetail) return;
    activeDetail.job = detail.job; activeDetail.segments = detail.segments;
    activeDetail.translation_runs = detail.translation_runs;
    activeDetail.translation_context_stale_ids = detail.translation_context_stale_ids;
    renderSubtitleListPreservingView(activeDetail.segments); updateTranslationControls();
    updateActiveSubtitle((mediaPlayer.media?.currentTime ?? 0) * 1_000);
  },
  status: setWorkspaceAction, changed: updateTranslationControls, time: formatTime, elapsed: formatElapsed,
  clock: { now: Date.now, repeat: tick => { const timer = window.setInterval(tick, 1_000); return () => window.clearInterval(timer); } },
});
const subtitleExportView = mountSubtitleExportDialogs(shell ?? document.body);
const subtitleExport = createSubtitleExport({
  context: () => activeDetail && (currentArea === "karaoke" || currentArea === "workspace") ? {
    jobId: activeDetail.job.job_id, navigation: navigationRequestId,
    inputPath: activeDetail.job.input_path, segments: activeDetail.segments, contextStaleIds: activeDetail.translation_context_stale_ids,
  } : null,
  available: () => !workspaceActionBusy,
  dirty: hasUnsavedSubtitleEdits,
  backend: createSubtitleExportBackend(invoke), view: subtitleExportView,
  busyChanged: setWorkspaceBusy, status: setWorkspaceAction, fileManager: fileManagerLabel,
  resultChanged: path => {
    revealExportButton?.classList.toggle("hidden", !path);
    updateTranslationControls();
  },
});
subtitleExportView.bindActions(exportButton, revealExportButton, () => void subtitleExport.export(), () => void subtitleExport.reveal());

const editorRoot = document.querySelector<HTMLElement>("#karaoke-view")!;
const editorSearch = document.querySelector<HTMLInputElement>("#editor-search")!;
const editorFilter = document.querySelector<HTMLSelectElement>("#editor-filter")!;
const editorStart = document.querySelector<HTMLInputElement>("#editor-cue-start")!;
const editorEnd = document.querySelector<HTMLInputElement>("#editor-cue-end")!;
let editorTimeFingerprint = "";
const editorRepairDialog = document.querySelector<HTMLDialogElement>("#editor-repair-dialog")!;
const editorLayout = mountEditorLayout(editorRoot);
const editorMenus = mountEditorMenus(editorRoot);
const editorWaveform = createEditorWaveform({
  context: () => activeDetail && currentArea === "karaoke" ? {
    jobId: activeDetail.job.job_id, navigation: navigationRequestId,
    durationMs: Math.round((mediaPlayer.media?.duration ?? 0) * 1_000), windowMs: karaokeWindowDurationMs(),
  } : null,
  points: () => (karaokeWaveform?.getBoundingClientRect().width ?? 1200) * window.devicePixelRatio,
  request: editorBackend.waveform,
  started: () => { if (karaokeWaveformStatus) karaokeWaveformStatus.textContent = "正在读取可见时间范围的波形…"; },
  accepted: waveform => {
    editorSession.timelineViewport = { startMs: waveform.start_ms, endMs: waveform.end_ms };
    karaokeWaveformStatus?.classList.remove("waveform-error");
    syncKaraokeWaveformStatus(); renderKaraokeTimeline();
  },
  failed: error => {
    if (karaokeWaveformStatus) { karaokeWaveformStatus.textContent = `无法生成波形：${String(error)}`; karaokeWaveformStatus.classList.add("waveform-error"); }
    renderKaraokeTimeline();
  },
});
const editorTimeline = mountEditorTimeline(karaokeWaveform!, editorRoot, {
  time: formatPreciseTime,
  resized: () => {
    renderKaraokeTimeline();
    if (activeDetail && currentArea === "karaoke") void loadKaraokeWaveform(editorWaveform.startMs, true);
  },
});
const currentCueEditor = mountCurrentCueEditor<SubtitleSegment>(editorRoot, {
  drafts: subtitleDrafts, current: karaokeDraftSegment, canUndo: canUndoSegment,
  undo: undoKaraokeTextSave, status: setSubtitleEditAction, lock: lockEditorCue,
});
const editorCueList = mountEditorCueList<SubtitleSegment>(document.querySelector<HTMLElement>("#editor-cue-list")!, {
  select: selectEditorCue,
  seek: id => { const cue = activeDetail ? subtitleTimeline().get(id) : undefined; if (cue) seekTo(cue.start_ms); },
  browse: () => editorFollow.browse(),
  time: formatPreciseTime,
  status: cue => {
    const draft = subtitleDrafts.entries.get(cueKey(cue));
    return draft?.error ? "保存失败" : draft?.saving ? "保存中" : draft && subtitleDrafts.pending(draft) ? "草稿"
      : segmentNeedsTranslation(cue) ? (cue.translated_text ? "待重译" : "未翻译") : editFlags(cue) || "已保存";
  },
});
function editorReviewIds(): Set<string> {
  const signals = asrQualitySignals.filter(signal => signal.latest_decision !== "resolved" && signal.latest_decision !== "false_positive");
  return new Set((activeDetail?.segments ?? []).filter(cue => signals.some(signal => signal.start_ms !== null && signal.end_ms !== null && signal.start_ms < cue.end_ms && signal.end_ms > cue.start_ms)).map(cue => cue.id));
}
let editorFilterCache: { segments: SubtitleSegment[]; signals: AsrQualitySignal[]; stale: string[]; query: string; filter: CueFilter; result: SubtitleSegment[] } | null = null;
function visibleEditorCues(): SubtitleSegment[] {
  if (!activeDetail) return [];
  const { segments, translation_context_stale_ids: stale } = activeDetail;
  const { query, filter } = editorSession;
  if (!query.trim() && filter === "all") return segments;
  if (editorFilterCache?.segments === segments && editorFilterCache.signals === asrQualitySignals && editorFilterCache.stale === stale && editorFilterCache.query === query && editorFilterCache.filter === filter) return editorFilterCache.result;
  const result = editorSession.visible(segments, filter === "review" ? editorReviewIds() : new Set(), stale);
  editorFilterCache = { segments, signals: asrQualitySignals, stale, query, filter, result };
  return result;
}

const editorFollow = mountEditorFollow<SubtitleSegment>(editorRoot, {
  session: editorSession, current: karaokeDraftSegment, visible: visibleEditorCues,
  busy: () => workspaceActionBusy, jobId: () => activeDetail?.job.job_id ?? null,
  navigation: () => navigationRequestId, active: () => currentArea === "karaoke",
  flush: flushEditorStructureDrafts, time: formatPreciseTime, status: setSubtitleEditAction,
  resumed: () => {
    karaokeFollowPlayhead = true;
    syncTimelineFollowState();
    editorTimeFingerprint = "";
    updateKaraokePosition((mediaPlayer.media?.currentTime ?? 0) * 1_000);
    currentCueEditor.sync(karaokeDraftSegment(), true);
    editorCueList.reveal(editorSession.playbackCueId);
  },
});
const timelineGestures = mountTimelineGestures<SubtitleSegment>(karaokeWaveform!, karaokeZoomInput!, editorRoot, {
  snapshot: () => activeDetail && editorWaveform.window && currentArea === "karaoke" ? {
    jobId: activeDetail.job.job_id, cues: activeDetail.segments, waveform: editorWaveform.window,
    currentMs: (mediaPlayer.media?.currentTime ?? 0) * 1_000, viewStartMs: editorWaveform.startMs, durationMs: karaokeWindowDurationMs(),
  } : null,
  select: id => {
    if (currentCueEditor.dirty) { setSubtitleEditAction("调整其他字幕块前，请先保存或放弃当前文字修改。", true); return false; }
    return selectEditorCue(id);
  },
  pause: () => mediaPlayer.media?.pause(),
  seek: ms => { seekTo(ms, false); updateActiveSubtitle(ms); },
  stopFollowing: () => { karaokeFollowPlayhead = false; syncTimelineFollowState(); },
  pan: start => editorWaveform.schedule(start),
  zoomValue: () => Number(karaokeZoomInput?.value ?? "66"),
  zoom: value => { karaokeZoomInput!.value = String(value); syncKaraokeZoomLabel(); },
  zoomForDuration: karaokeZoomValueForDuration,
  preview: drag => {
    const cue = drag.draft[drag.segmentIndex];
    const action = drag.mode === "move" ? "移动" : drag.mode === "start" ? "修剪入点" : "修剪出点";
    setSubtitleEditAction(`预览：${action}第 ${cue.segment_index + 1} 段至 ${formatPreciseTime(cue.start_ms)} → ${formatPreciseTime(cue.end_ms)}；松开后保存。`);
    renderKaraokeTimeline();
  },
  commit: commitKaraokeTimingDrag,
  cancelled: () => { renderKaraokeTimeline(); setSubtitleEditAction("已放弃本次边界拖动。"); },
});
function lockEditorCue(): void { editorFollow.lock(); }
function syncEditorFollow(): void { editorFollow.sync(); }
function syncEditorSelectionMessage(): void { editorFollow.syncMessage(); }
function editorTimingDirty(): boolean { return editorFollow.timingDirty(); }
async function resumeEditorFollow(): Promise<void> { await editorFollow.resume(); }
function renderEditorList(): void {
  if (currentArea !== "karaoke" || !activeDetail) return;
  const cues = visibleEditorCues();
  editorCueList.render(cues, editorSession.selectedCueId, editorSession.playbackCueId);
  if (editorSession.listAutoScroll) editorCueList.reveal(editorSession.playbackCueId);
  document.querySelector("#editor-match-count")!.textContent = `${cues.length} / ${activeDetail.segments.length} 段`;
  syncEditorSelectionMessage();
}
function selectEditorCue(id: string): boolean {
  const cue = activeDetail ? subtitleTimeline().get(id) : undefined;
  if (!cue || workspaceActionBusy) return false;
  const previous = karaokeDraftSegment();
  if (previous && previous.id !== id && editorTimingDirty()) { setSubtitleEditAction("请先保存时间，或在时间输入框按 Escape 恢复，再切换字幕。", true); return false; }
  if (previous && previous.id !== id && subtitleDrafts.entries.get(cueKey(previous))?.composing) { setSubtitleEditAction("请先完成输入法选字，再切换字幕。", true); return false; }
  editorFollow.lock(id);
  editorTimeFingerprint = "";
  editorStart.value = formatPreciseTime(cue.start_ms); editorEnd.value = formatPreciseTime(cue.end_ms);
  currentCueEditor.sync(cue, true);
  editorCueList.mark(id, editorSession.playbackCueId);
  updateKaraokePosition((mediaPlayer.media?.currentTime ?? 0) * 1_000);
  renderEditorList();
  return true;
}
function setEditorMutationBusy(busy: boolean): void {
  workspaceActionBusy = busy;
  const panel = editorRoot.querySelector<HTMLElement>(".karaoke-current-segment");
  if (panel) panel.inert = busy;
}
async function flushEditorStructureDrafts(): Promise<boolean> {
  const jobId = activeDetail?.job.job_id;
  try {
    await subtitleDrafts.flushAll();
    if ([...subtitleDrafts.entries.values()].some(draft => draft.saved.job_id === jobId && (draft.composing || subtitleDrafts.pending(draft)))) throw new Error("请完成输入法选字或保存草稿后再执行操作");
  }
  catch (error) { setSubtitleEditAction(`文字保存失败，操作未执行：${String(error)}`, true); return false; }
  return !!jobId && activeDetail?.job.job_id === jobId;
}
async function saveEditorCueTime(): Promise<void> {
  const startInput = editorStart.value, endInput = editorEnd.value;
  const selectedId = editorSession.selectedCueId;
  if (workspaceActionBusy || !await flushEditorStructureDrafts() || !activeDetail) return;
  const cue = karaokeDraftSegment();
  if (!cue || cue.id !== selectedId) return;
  const startMs = parseTimecode(startInput), endMs = parseTimecode(endInput);
  if (startMs === null || endMs === null || startMs < 0 || endMs <= startMs) { setSubtitleEditAction("请输入有效的开始和结束时间。", true); return; }
  const jobId = activeDetail.job.job_id, before = cloneSubtitleSegments(activeDetail.segments);
  const after = before.map(item => item.id === cue.id ? { ...item, start_ms: startMs, end_ms: endMs } : item);
  setEditorMutationBusy(true);
  try {
    const saved = await editorBackend.saveTiming(jobId, before, after);
    rememberStructureEdit(before, saved, "手动调整时间", jobId);
    if (activeDetail?.job.job_id !== jobId) return;
    applySubtitleStructure(saved, cue.id); setSubtitleEditAction("时间已保存，未移动相邻字幕。");
  } catch (error) { setSubtitleEditAction(`时间保存失败：${String(error)}`, true); }
  finally { setEditorMutationBusy(false); }
}
const taskTools = mountTaskTools(workspaceView, editorRoot, closeEditorTools);
const cleanupConfirmation = mountCandidateCleanupConfirmation(shell!);
const repairTool = mountToolDialog(editorRepairDialog, () => { reviewSignals.clear(); candidateHistory.clear(); }, () => !cleanupConfirmation.open);
const processingBackend = createProcessingBackend(invoke);
const editorProcessing = createEditorProcessing({
  backend: processingBackend,
  context: () => activeDetail ? { jobId: activeDetail.job.job_id, navigation: navigationRequestId } : null,
  visible: () => editorRepairDialog.open || !!retranscriptionDialog?.open,
  segments: () => activeDetail?.segments ?? [],
  busyChanged: (busy, phase) => {
    if (previewRetranscriptionButton) previewRetranscriptionButton.disabled = busy;
    if (confirmRetranscriptionButton) confirmRetranscriptionButton.disabled = busy;
    editorRepairDialog.querySelectorAll<HTMLButtonElement>("[data-full-candidate]").forEach(button => button.disabled = busy);
    retranscriptionDialog?.querySelectorAll<HTMLButtonElement>(".refresh-comparison").forEach(button => button.disabled = busy);
    retranscriptionDialog?.querySelectorAll<HTMLButtonElement>("#close-retranscription-preview, #cancel-retranscription").forEach(button => button.disabled = phase === "adopt");
  },
  mutationLock: setEditorMutationBusy,
  previewed: showRetranscriptionPreview,
  adopted: (before, after, preview, signalId) => {
    rememberStructureEdit(before, after, `局部重新识别 ${formatPreciseTime(preview.start_ms)} → ${formatPreciseTime(preview.end_ms)}`, preview.job_id);
    applySubtitleStructure(after, preview.candidate_segments[0]?.id);
    candidateTool.close(); activeRepairSignalId = null;
    candidateHistory.adopted(preview.preview_id);
    if (signalId) void saveAsrReviewDecision(signalId, "resolved");
    setSubtitleEditAction(`已替换所选范围为 ${preview.candidate_segments.length} 段新识别字幕；新段需要重新翻译，可用会话撤销恢复。`);
  },
  status: setRetranscriptionMessage,
  comparisonStatus: message => { if (retranscriptionConfirmMessage) retranscriptionConfirmMessage.textContent = message; },
  time: formatTime,
});
retranscriptionDialog?.addEventListener("cancel", event => { if (editorProcessing.adopting) event.preventDefault(); });
const candidateTool = mountToolDialog(retranscriptionDialog!, () => {}, () => !editorProcessing.adopting);
retranscriptionDialog?.classList.add("editor-candidate-dialog");
const candidateHistory = mountCandidateHistory(editorRepairDialog.querySelector<HTMLElement>(".retranscription-range")!, {
  context: () => activeDetail ? { jobId: activeDetail.job.job_id, navigation: navigationRequestId } : null,
  visible: () => editorRepairDialog.open,
  list: processingBackend.runs,
  release: processingBackend.release,
  confirmRelease: () => cleanupConfirmation.confirm(),
  released: (runId, bytes) => {
    if (editorProcessing.preview?.preview_id === runId) { editorProcessing.clearPreview(runId); candidateTool.close(); }
    setRetranscriptionMessage(`已清理候选产物，本次释放 ${formatBytes(bytes)}。`);
  },
  failed: error => setRetranscriptionMessage(String(error), true),
  label: candidateLabel,
  full: () => { activeRepairSignalId = null; void editorProcessing.previewFull(); },
  cancel: () => void editorProcessing.cancel(),
  reopen: runId => { void editorProcessing.reopen(runId); },
});
const reviewSignals = mountReviewSignals(editorRepairDialog, {
  time: formatPreciseTime, escape: escapeHtml,
  review: (signalId, decision) => { void saveAsrReviewDecision(signalId, decision); },
  locate: signal => {
    if (currentArea !== "karaoke" || signal.start_ms === null) return;
    mediaPlayer.media?.pause(); karaokeFollowPlayhead = false; syncTimelineFollowState();
    seekTo(signal.start_ms, false); updateActiveSubtitle(signal.start_ms);
    void loadKaraokeWaveform(signal.start_ms); repairTool.close();
    if (editorRoot.classList.contains("editor-waveform-hidden")) editorRoot.querySelector<HTMLButtonElement>('[data-editor-toggle="waveform"]')?.click();
  },
  repair: signal => {
    if (signal.start_ms === null || signal.end_ms === null || !retranscriptionStartInput || !retranscriptionEndInput) return;
    activeRepairSignalId = signal.id;
    if (authorizeGeminiUpload) authorizeGeminiUpload.checked = false;
    retranscriptionStartInput.value = formatPreciseTime(signal.start_ms);
    retranscriptionEndInput.value = formatPreciseTime(signal.end_ms);
    setRetranscriptionMessage("已选取待复核范围；选择识别方式后，点击“识别并预览”。当前字幕尚未修改。", false);
    retranscriptionStartInput.focus();
  },
});
function openEditorRepair(): void {
  repairTool.open();
  editorProcessing.opened();
  if (activeDetail) void loadAsrQualitySignals(activeDetail.job.job_id);
}
async function showEditorTools(section: WorkspaceSection): Promise<void> {
  if (!activeDetail || !await flushEditorStructureDrafts()) return;
  taskTools.mountOverlay();
  renderWorkspace(activeDetail);
  showWorkspaceSection(section);
  workspaceView.querySelector<HTMLButtonElement>("#back-to-jobs")!.textContent = "← 返回字幕编辑";
  workspaceView.querySelector<HTMLButtonElement>("#back-to-jobs")?.focus();
  await refreshTranslationStatus();
}
function closeEditorTools(): void {
  taskProvider.closed();
  taskTools.unmount();
  currentCueEditor.sync(karaokeDraftSegment(), true); renderEditorList();
}
editorSearch.addEventListener("input", () => { editorSession.query = editorSearch.value; renderEditorList(); });
editorFilter.addEventListener("change", () => { editorSession.filter = editorFilter.value as CueFilter; renderEditorList(); });
editorRoot.addEventListener("click", event => {
  const button = (event.target as HTMLElement).closest<HTMLElement>("[data-editor-action]");
  if (!button) return;
  const action = button.dataset.editorAction;
  if (action === "back") void showTopLevelArea("workbench").catch(error => setSubtitleEditAction(String(error), true));
  else if (action === "export") void showEditorTools("export");
  else if (action === "repair") openEditorRepair();
  else if (action === "close-repair") repairTool.close();
  else if (action === "save-time") void saveEditorCueTime();
  else if (action === "previous" || action === "next") { const id = editorSession.adjacent(visibleEditorCues(), action === "previous" ? -1 : 1); if (id && selectEditorCue(id)) editorCueList.reveal(id); }
  else if (action === "reveal-playing") { editorSession.listAutoScroll = true; editorCueList.reveal(editorSession.playbackCueId); syncEditorFollow(); }
  else if (action === "follow") void resumeEditorFollow();
  else if (action === "translate") { const cue = karaokeDraftSegment(); if (cue && karaokeTimingMessage) void subtitleTranslation.single(cue); }
  else if (action === "glossary" && !workspaceGlossary?.value) { void showEditorTools("translation"); setWorkspaceAction("请选择要保存修正规则的词表，再返回编辑加入修正。"); }
  else if (action === "glossary") { const cue = karaokeDraftSegment(); if (cue && karaokeCurrentSource && karaokeTimingMessage) captureGlossaryCorrection(cue, karaokeCurrentSource, karaokeTimingMessage); }
});

window.addEventListener("pagehide", () => {
  taskProvider.dispose();
  taskProviderView?.dispose();
  videoRenderSession.dispose();
  videoRenderView.dispose();
  subtitleStyles.dispose();
  subtitleStyleView.dispose();
  subtitleTranslation.dispose();
  subtitleExport.dispose();
  subtitleExportView.dispose();
  editorLayout.dispose();
  editorMenus.dispose();
  editorFollow.dispose();
  editorTimeline.dispose();
  editorWaveform.dispose();
  timelineGestures.dispose();
  mediaPlayer.dispose();
  editorCueList.dispose();
  currentCueEditor.dispose();
  taskTools.dispose();
  reviewSignals.dispose();
  candidateHistory.dispose();
  editorProcessing.dispose();
  candidateTool.dispose();
  cleanupConfirmation.dispose();
  repairTool.dispose();
}, { once: true });

function languageLabel(language: string): string {
  if (language === "ja") return "日语";
  if (language === "en") return "英语";
  if (language === "ko") return "韩语";
  if (language === "zh-Hans") return "简体中文";
  return language;
}

function selectedSourceLanguage(): LanguageCode {
  return (sourceLanguage?.value as LanguageCode | undefined) ?? "ja";
}

function activeSourceLanguage(): LanguageCode {
  return activeDetail?.job.source_language ?? selectedSourceLanguage();
}

function activeTargetLanguage(): LanguageCode {
  return activeDetail?.job.target_language ?? "zh-Hans";
}

function displayName(job: LocalJob): string {
  if (job.display_name?.trim()) return job.display_name;
  const source = job.input_path?.split("/").pop();
  return source || job.job_id;
}

function statusLabel(status: string): string {
  const labels: Record<string, string> = {
    queued: "等待处理",
    created: "已创建",
    extracting_audio: "提取音频",
    transcribing: "正在转写",
    refining_segments: "整理分段",
    translating: "正在翻译",
    exporting_subtitles: "写入字幕",
    rendering_video: "烧录视频",
    done: "已完成",
    failed: "失败",
  };
  return labels[status] ?? status.split("_").join(" ");
}

function formatElapsed(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3_600);
  const minutes = Math.floor((total % 3_600) / 60);
  const remainingSeconds = total % 60;
  if (hours > 0) return `${hours}小时${String(minutes).padStart(2, "0")}分`;
  if (minutes > 0) return `${minutes}分${String(remainingSeconds).padStart(2, "0")}秒`;
  return `${remainingSeconds}秒`;
}

function jobTimingLabel(job: Pick<LocalJob, "status" | "created_at_unix" | "started_at_unix" | "completed_at_unix"> & Partial<Pick<LocalJob,"updated_at_unix" | "processing_time">>): string {
  const now = Math.floor(Date.now() / 1_000);
  if (job.status === "queued") return `排队 ${formatElapsed(now - (job.updated_at_unix ?? job.created_at_unix))}`;
  const elapsed = executionSeconds(job.status, job.processing_time, now);
  if (elapsed !== null) return `累计处理 ${formatElapsed(elapsed)}`;
  if (matchesTerminalStatus(job.status)) {
    if (job.started_at_unix !== null && job.completed_at_unix !== null && job.completed_at_unix >= job.started_at_unix) {
      return `任务历时 ${formatElapsed(job.completed_at_unix - job.started_at_unix)}`;
    }
    return "历史任务未记录处理用时";
  }
  if (job.started_at_unix !== null) return `任务历时 ${formatElapsed(now - job.started_at_unix)}`;
  return `准备中 ${formatElapsed(now - job.created_at_unix)}`;
}

function updateVisibleJobTimings(): void {
  document.querySelectorAll<HTMLElement>("[data-job-timing]").forEach((element) => {
    const createdAt = Number(element.dataset.createdAt);
    const startedAt = element.dataset.startedAt ? Number(element.dataset.startedAt) : null;
    const completedAt = element.dataset.completedAt ? Number(element.dataset.completedAt) : null;
    const status = element.dataset.status ?? "";
    if (!Number.isFinite(createdAt)) return;
    element.textContent = jobTimingLabel({
      created_at_unix: createdAt,
      updated_at_unix: Number(element.dataset.updatedAt) || createdAt,
      started_at_unix: startedAt,
      completed_at_unix: completedAt,
      status,
      processing_time: element.dataset.processingTime ? JSON.parse(element.dataset.processingTime) : null,
    });
  });
}

function runningJob(job: LocalJob): boolean {
  return !matchesTerminalStatus(job.status);
}

function settleConfirmation(confirmed: boolean): void {
  const resolve = confirmationResolver;
  confirmationResolver = null;
  if (confirmationDialog?.open) confirmationDialog.close();
  resolve?.(confirmed);
}

function confirmAction(options: {
  title: string;
  message: string;
  confirmLabel: string;
  danger?: boolean;
}): Promise<boolean> {
  if (!confirmationDialog || !confirmationTitle || !confirmationMessage || !acceptConfirmationButton) {
    return Promise.resolve(false);
  }
  if (confirmationResolver) settleConfirmation(false);
  confirmationTitle.textContent = options.title;
  confirmationMessage.textContent = options.message;
  acceptConfirmationButton.textContent = options.confirmLabel;
  acceptConfirmationButton.classList.toggle("danger", options.danger ?? false);
  confirmationDialog.inert = false;
  confirmationDialog.showModal();
  document.querySelector<HTMLButtonElement>("#cancel-confirmation")?.focus();
  return new Promise<boolean>((resolve) => {
    confirmationResolver = resolve;
  });
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => {
    const entities: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      "'": "&#39;",
      '"': "&quot;",
    };
    return entities[character];
  });
}

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KiB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GiB`;
}

function syncProviderSettings(): void {
  const provider = settingsProvider?.value ?? "none";
  const cloudEnabled = provider !== "none" && provider !== "hy-mt2-local";
  const llmEnabled = provider === "gemini" || provider === "deepseek" || provider === "openai-compatible";
  const customEndpoint = provider === "openai-compatible";
  document.querySelector<HTMLElement>("#api-key-field")?.classList.toggle("hidden", !cloudEnabled);
  settingsClearApiKey?.closest("label")?.classList.toggle("hidden", !cloudEnabled);
  checkApiKeyButton?.closest("div")?.classList.toggle("hidden", !cloudEnabled);
  document.querySelector<HTMLElement>("#llm-provider-fields")?.classList.toggle("hidden", !llmEnabled);
  document.querySelector<HTMLElement>("#provider-base-url-field")?.classList.toggle("hidden", !customEndpoint);
  document.querySelector<HTMLElement>("#provider-model-field")?.classList.toggle("hidden", provider === "hy-mt2-local");
  if (settingsProviderModel) settingsProviderModel.disabled = provider === "hy-mt2-local";
  const providerLabel = provider === "gemini" ? "Gemini（与语音识别共用）" : provider === "deepseek" ? "DeepSeek" : provider === "openai-compatible" ? "OpenAI-compatible" : "DeepL";
  const apiKeyLabel = document.querySelector<HTMLSpanElement>("#api-key-label");
  const clearApiKeyLabel = document.querySelector<HTMLSpanElement>("#clear-api-key-label");
  if (apiKeyLabel) apiKeyLabel.textContent = `${providerLabel} API Key`;
  if (clearApiKeyLabel) clearApiKeyLabel.textContent = `删除系统凭据库中已保存的 ${providerLabel} Key`;
  settingsHyMt2Model?.closest(".settings-path-row")?.classList.remove("hidden");
  renderProviderCards();
}

async function checkSelectedApiKey(): Promise<void> {
  if (!settingsProvider || !checkApiKeyButton || !apiKeyStatus || settingsProvider.value === "none" || settingsProvider.value === "hy-mt2-local") return;
  checkApiKeyButton.disabled = true;
  apiKeyStatus.textContent = `正在检查 ${settingsProvider.value} 的系统凭据；${credentialCheckHint}…`;
  apiKeyStatus.classList.remove("warning");
  try {
    const result = await invoke<TranslationCredentialCheck>("check_translation_api_key", {
      providerId: settingsProvider.value,
    });
    const environment = result.availableFromEnvironment ? "；启动环境中也有兼容 Key" : "";
    apiKeyStatus.textContent = result.storedInSystem
      ? `${result.providerName} Key 存在于 ${result.credentialStore}${environment}；内容不会回显。`
      : `${result.credentialStore} 中没有 ${result.providerName} Key${environment}。`;
    apiKeyStatus.classList.toggle("warning", !result.storedInSystem && !result.availableFromEnvironment);
  } catch (error) {
    apiKeyStatus.textContent = `检查失败：${String(error)}`;
    apiKeyStatus.classList.add("warning");
  } finally {
    checkApiKeyButton.disabled = false;
  }
}

function syncNetworkSettings(): void {
  const customProxy = settingsProxyMode?.value === "custom";
  const proxyField = document.querySelector<HTMLElement>("#proxy-url-field");
  proxyField?.classList.toggle("disabled-field", !customProxy);
  if (settingsProxyUrl) settingsProxyUrl.disabled = !customProxy;
}

function overwriteSettingsField<T extends HTMLInputElement | HTMLSelectElement>(field: T | null, value: string | boolean): void {
  if (!field || settingsDirtyFields.has(field.id)) return;
  if (typeof value === "boolean" && field instanceof HTMLInputElement) field.checked = value;
  else if (typeof value === "string") field.value = value;
}

function renderDesktopSettings(settings: DesktopSettings): void {
  desktopSettings = settings;
  settingsDialog?.classList.toggle("onboarding",settings.needsOnboarding);
  const title=settingsDialog?.querySelector("h2");if(title)title.textContent=settings.needsOnboarding ? "首次配置" : "设置";
  if (!settingsDirtyFields.has("settings-provider")) lastSettingsProvider = settings.translationProviderId;
  overwriteSettingsField(settingsWhisperModel, settings.whisperModelPath ?? "");
  overwriteSettingsField(settingsVadModel, settings.vadModelPath ?? "");
  overwriteSettingsField(settingsHyMt2Model, settings.hyMt2ModelPath ?? "");
  overwriteSettingsField(settingsProxyMode, settings.networkProxyMode);
  overwriteSettingsField(settingsProxyUrl, settings.networkProxyUrl ?? "");
  overwriteSettingsField(settingsModelMirror, settings.modelMirrorUrl ?? "");
  overwriteSettingsField(settingsProvider, settings.translationProviderId);
  const profile = providerDrafts.get(settingsProvider?.value ?? settings.translationProviderId);
  overwriteSettingsField(settingsProviderModel, profile?.model ?? settings.translationModel ?? "");
  overwriteSettingsField(settingsProviderBaseUrl, profile?.endpoint ?? settings.translationBaseUrl ?? "");
  overwriteSettingsField(settingsTranslationStyle, profile?.style ?? settings.translationStyleInstruction);
  overwriteSettingsField(settingsApiKey, "");
  overwriteSettingsField(settingsClearApiKey, false);
  if (modelsDirectory) modelsDirectory.textContent = `下载目录：${settings.modelsDirectory}`;
  if (modelReadiness) {
    const recognition = settings.whisperModelReady
      ? settings.vadModelReady ? "Whisper 与 VAD 已就绪" : "Whisper 已就绪 · VAD 可选"
      : "需要配置 Whisper 模型";
    const localTranslation = settings.hyMt2ModelReady
      ? settings.localTranslationRuntimeReady ? " · Hy-MT2 已就绪" : " · Hy-MT2 runtime 缺失"
      : "";
    modelReadiness.textContent = `${recognition}${localTranslation}`;
    modelReadiness.classList.toggle("warning", !settings.whisperModelReady);
  }
  if (credentialStoreLabel) credentialStoreLabel.textContent = settings.credentialStore;
  if (apiKeyStatus) {
    const source = settings.translationApiKeySource === "system"
      ? `已保存在 ${settings.credentialStore}`
      : settings.translationApiKeySource === "environment"
        ? "当前来自启动环境；保存新 Key 后会改用系统凭据库"
        : settings.translationApiKeySource === "saved"
          ? `已保存至 ${settings.credentialStore}；启动时不读取，首次翻译时验证`
        : settings.translationApiKeySource === "deferred"
          ? `尚未在本 App 中保存 Key；启动时不会读取 ${settings.credentialStore}`
        : settings.translationApiKeySource === "local"
          ? settings.hyMt2ModelReady && settings.localTranslationRuntimeReady
            ? "本地 runtime 与模型已就绪；翻译文本不会离开设备"
            : "本地翻译尚未就绪；请确认当前 App 包含 runtime 并下载 Hy-MT2 模型"
          : "尚未配置；Key 仅保存在系统凭据库";
    apiKeyStatus.textContent = settings.credentialError
      ? `${source}。系统凭据库提示：${settings.credentialError}`
      : source;
    apiKeyStatus.classList.toggle("warning", Boolean(settings.credentialError));
  }
  const taskProvider = document.querySelector<HTMLSelectElement>("#task-translation-provider");
  if (taskProvider && !taskProvider.value && settings.translationProviderId !== "none") taskProvider.value = settings.translationProviderId;
  if (modelPath && settings.whisperModelPath) modelPath.value = settings.whisperModelPath;
  if (vadModelPath && settings.vadModelPath) vadModelPath.value = settings.vadModelPath;
  updateTaskPipelineSummary();
  syncProviderSettings();
  syncNetworkSettings();
  renderModelCatalog();
}

function renderModelCatalog(): void {
  if (!modelCatalogHost) return;
  const activeDownload = modelDownloads.find((download) =>
    download.status === "queued" || download.status === "downloading" || download.status === "cancelling"
  );
  modelCatalogHost.innerHTML = availableModels.map((model) => {
    const download = modelDownloads.find((item) => item.modelId === model.id);
    const progress = download?.totalBytes
      ? Math.min(1, download.downloadedBytes / download.totalBytes)
      : null;
    const source = download?.source ? ` · ${download.source}` : "";
    const state = download?.status === "done"
      ? `${download.active ? "已下载 · 当前使用" : "已下载"}${source}`
      : download?.status === "failed"
        ? `失败：${download.error ?? "未知错误"}${source}`
        : download?.status === "cancelled"
          ? `已取消；保留临时文件供续传${source}`
          : download?.status === "cancelling"
            ? `正在取消…${source}`
            : download?.status === "downloading"
              ? `${formatBytes(download.downloadedBytes)}${download.totalBytes ? ` / ${formatBytes(download.totalBytes)}` : ""}${source}`
              : download?.status === "queued" ? `等待下载${source}` : "";
    const action = download && ["queued", "downloading", "cancelling"].includes(download.status) ? "cancel" : "download";
    const buttonLabel = action === "cancel" ? download?.status === "cancelling" ? "正在取消" : "取消"
      : download?.status === "failed" || download?.status === "cancelled" ? "继续下载" : "下载";
    const disabled = action === "cancel"
      ? download?.status === "cancelling"
      : Boolean(activeDownload);
    const actions = download?.status === "done"
      ? `<div>${download.active ? "" : `<button type="button" data-model-action="select" data-model-id="${escapeHtml(model.id)}">设为当前</button>`}<button type="button" class="secondary" data-model-action="uninstall" data-model-id="${escapeHtml(model.id)}">卸载</button></div>`
      : `<button type="button" class="secondary" data-model-action="${action}" data-model-id="${escapeHtml(model.id)}" ${disabled ? "disabled" : ""}>${buttonLabel}</button>`;
    return `<article class="model-card">
      <div><strong>${escapeHtml(model.name)}</strong><span>${escapeHtml(model.sizeLabel)} · ${escapeHtml(model.recommendedFor)}</span></div>
      ${progress === null ? "" : `<progress max="1" value="${progress}"></progress>`}
      <div class="model-card-action"><span class="${download?.status === "failed" ? "warning" : ""}">${escapeHtml(state)}</span>${actions}</div>
    </article>`;
  }).join("");
  modelCatalogHost.querySelectorAll<HTMLButtonElement>("[data-model-action]").forEach((button) => {
    button.addEventListener("click", () => {
      const modelId = button.dataset.modelId ?? "";
      if (button.dataset.modelAction === "cancel") void cancelModelDownload(modelId);
      else if (button.dataset.modelAction === "uninstall") void uninstallModel(modelId);
      else if (button.dataset.modelAction === "select") void selectModel(modelId);
      else void startModelDownload(modelId);
    });
  });
  renderProviderCards();
}

function renderDictionaryCatalog(): void {
  if (!dictionaryCatalogHost) return;
  const activeDownload = dictionaryDownloads.find((download) =>
    download.status === "queued" || download.status === "resolving" || download.status === "downloading"
  );
  dictionaryCatalogHost.innerHTML = availableDictionaries.map((item) => {
    const download = dictionaryDownloads.find((value) => value.dictionaryId === item.id);
    const progress = download?.totalBytes
      ? Math.min(1, download.downloadedBytes / download.totalBytes)
      : null;
    const state = download?.status === "done"
      ? `已安装${download.version ? ` · ${download.version}` : ""}`
      : download?.status === "failed"
        ? `失败：${download.error ?? "未知错误"}`
        : download?.status === "downloading"
          ? `${formatBytes(download.downloadedBytes)}${download.totalBytes ? ` / ${formatBytes(download.totalBytes)}` : ""}${download.source ? ` · ${download.source}` : ""}`
          : download?.status === "resolving" ? "正在获取最新版与校验信息…"
            : download?.status === "queued" ? "等待下载" : "尚未安装";
    return `<article class="model-card">
      <div><strong>${escapeHtml(item.name)}</strong><span>${escapeHtml(item.languagePair)} · ${escapeHtml(item.sizeLabel)} · ${escapeHtml(item.versionLabel)}</span></div>
      <span>${escapeHtml(item.description)}</span>
      <span>${escapeHtml(item.license)} · 署名：${escapeHtml(item.attribution)}</span>
      <span>来源：${escapeHtml(item.sourceUrl)}</span>
      ${progress === null ? "" : `<progress max="1" value="${progress}"></progress>`}
      <div class="model-card-action"><span class="${download?.status === "failed" ? "warning" : ""}">${escapeHtml(state)}</span><button type="button" class="secondary" data-download-dictionary="${escapeHtml(item.id)}" ${activeDownload ? "disabled" : ""}>${download?.status === "done" ? "检查更新" : download?.status === "failed" ? "重试" : "下载"}</button></div>
    </article>`;
  }).join("");
  dictionaryCatalogHost.querySelectorAll<HTMLButtonElement>("[data-download-dictionary]").forEach((button) => {
    button.addEventListener("click", () => void startDictionaryDownload(button.dataset.downloadDictionary ?? ""));
  });
}

function renderDictionaryCredentials(): void {
  dictionaryCredentials.forEach((status) => {
    const element = document.querySelector<HTMLElement>(`[data-dictionary-status="${status.providerId}"]`);
    if (!element) return;
    element.textContent = status.configured
      ? `已保存至 ${status.credentialStore}；内容不会回显`
      : `尚未在 ${status.credentialStore} 中配置`;
    element.classList.toggle("warning", !status.configured);
  });
}

async function loadDictionarySettings(): Promise<void> {
  try {
    const [catalog, downloads, credentials, directory] = await Promise.all([
      invoke<DictionaryCatalogItem[]>("dictionary_catalog"),
      invoke<DictionaryDownloadState[]>("dictionary_download_states"),
      invoke<DictionaryCredentialStatus[]>("dictionary_credential_statuses"),
      invoke<string>("dictionary_directory"),
    ]);
    availableDictionaries = catalog;
    dictionaryDownloads = downloads;
    dictionaryCredentials = credentials;
    if (dictionaryDirectoryLabel) dictionaryDirectoryLabel.textContent = `目录：${directory}`;
    renderDictionaryCatalog();
    renderDictionaryCredentials();
  } catch (error) {
    if (dictionaryDownloadMessage) dictionaryDownloadMessage.textContent = `无法读取词典配置：${String(error)}`;
  }
}

function renderAsrCredentialStatus(status: AsrCredentialStatus): void {
  if (asrCredentialStoreLabel) asrCredentialStoreLabel.textContent = status.credentialStore;
  if (geminiApiKeyStatus) {
    geminiApiKeyStatus.textContent = status.configured
      ? `Gemini Key 已保存至 ${status.credentialStore}；内容不会回显。`
      : `${status.credentialStore} 中尚未配置 Gemini Key。`;
    geminiApiKeyStatus.classList.toggle("warning", !status.configured);
  }
  if (settingsGeminiApiKey) settingsGeminiApiKey.value = "";
}

async function saveGeminiCredential(clear: boolean): Promise<void> {
  if (!settingsGeminiApiKey || !geminiApiKeyStatus) return;
  const apiKey = settingsGeminiApiKey.value.trim();
  if (!clear && !apiKey) {
    geminiApiKeyStatus.textContent = "请输入 Gemini API Key。";
    geminiApiKeyStatus.classList.add("warning");
    return;
  }
  geminiApiKeyStatus.textContent = clear ? "正在删除 Gemini Key…" : "正在保存 Gemini Key…";
  try {
    const status = await invoke<AsrCredentialStatus>("save_asr_credential", {
      request: { apiKey: clear ? null : apiKey, clear },
    });
    renderAsrCredentialStatus(status);
    await refreshProviderReadiness();
    if (settingsApiKey && settingsProvider?.value === "gemini") settingsApiKey.value = "";
  } catch (error) {
    geminiApiKeyStatus.textContent = `操作失败：${String(error)}`;
    geminiApiKeyStatus.classList.add("warning");
  }
}

async function checkGeminiCredential(): Promise<void> {
  if (!geminiApiKeyStatus) return;
  geminiApiKeyStatus.textContent = `正在检查系统凭据；${credentialCheckHint}…`;
  try {
    renderAsrCredentialStatus(await invoke<AsrCredentialStatus>("check_asr_credential"));
  } catch (error) {
    geminiApiKeyStatus.textContent = `检查失败：${String(error)}`;
    geminiApiKeyStatus.classList.add("warning");
  }
}

async function startDictionaryDownload(dictionaryId: string): Promise<void> {
  if (!dictionaryId || !settingsProxyMode || !settingsProxyUrl || !settingsModelMirror) return;
  if (dictionaryDownloadMessage) dictionaryDownloadMessage.textContent = "正在保存当前网络配置并获取词典发布信息…";
  try {
    await invoke<void>("save_download_network_settings", {
      request: {
        proxyMode: settingsProxyMode.value,
        proxyUrl: settingsProxyUrl.value.trim() || null,
        modelMirrorUrl: settingsModelMirror.value.trim() || null,
      },
    });
    const state = await invoke<DictionaryDownloadState>("start_dictionary_download", { dictionaryId });
    dictionaryDownloads = dictionaryDownloads.filter((download) => download.dictionaryId !== dictionaryId);
    dictionaryDownloads.push(state);
    renderDictionaryCatalog();
    if (dictionaryDownloadPoll === null) {
      dictionaryDownloadPoll = window.setInterval(() => void refreshDictionaryDownloads(), 500);
    }
  } catch (error) {
    if (dictionaryDownloadMessage) dictionaryDownloadMessage.textContent = `无法开始词典下载：${String(error)}`;
  }
}

async function refreshDictionaryDownloads(): Promise<void> {
  try {
    dictionaryDownloads = await invoke<DictionaryDownloadState[]>("dictionary_download_states");
    renderDictionaryCatalog();
    const active = dictionaryDownloads.some((download) =>
      download.status === "queued" || download.status === "resolving" || download.status === "downloading"
    );
    if (!active) {
      if (dictionaryDownloadPoll !== null) window.clearInterval(dictionaryDownloadPoll);
      dictionaryDownloadPoll = null;
      const failed = dictionaryDownloads.find((download) => download.status === "failed");
      if (dictionaryDownloadMessage) {
        dictionaryDownloadMessage.textContent = failed
          ? `下载失败：${failed.error ?? "未知错误"}`
          : "词典包已校验并安装到正式应用数据目录。";
      }
    }
  } catch (error) {
    if (dictionaryDownloadMessage) dictionaryDownloadMessage.textContent = `无法读取词典下载状态：${String(error)}`;
  }
}

async function saveDictionaryCredential(providerId: string, clear = false): Promise<void> {
  const input = document.querySelector<HTMLInputElement>(`[data-dictionary-key="${providerId}"]`);
  if (!input || (!clear && !input.value.trim())) {
    if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = "请先输入要保存的 API Key。";
    return;
  }
  if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = clear ? "正在删除词典凭据…" : "正在保存词典凭据…";
  try {
    const status = await invoke<DictionaryCredentialStatus>("save_dictionary_credential", {
      request: { providerId, apiKey: clear ? null : input.value.trim(), clear },
    });
    input.value = "";
    dictionaryCredentials = dictionaryCredentials.filter((item) => item.providerId !== status.providerId);
    dictionaryCredentials.push(status);
    renderDictionaryCredentials();
    if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = clear
      ? `${status.providerName} Key 已从 ${status.credentialStore} 删除。`
      : `${status.providerName} Key 已保存至 ${status.credentialStore}。`;
  } catch (error) {
    if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = `词典凭据操作失败：${String(error)}`;
  }
}

async function checkDictionaryCredential(providerId: string): Promise<void> {
  if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = `正在检查系统凭据；${credentialCheckHint}…`;
  try {
    const status = await invoke<DictionaryCredentialStatus>("check_dictionary_credential", { providerId });
    dictionaryCredentials = dictionaryCredentials.filter((item) => item.providerId !== status.providerId);
    dictionaryCredentials.push(status);
    renderDictionaryCredentials();
    if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = status.configured
      ? `${status.providerName} Key 存在于 ${status.credentialStore}；内容不会回显。`
      : `${status.credentialStore} 中没有 ${status.providerName} Key。`;
  } catch (error) {
    if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = `检查失败：${String(error)}`;
  }
}

async function loadDesktopSettings(openWhenNeeded = false): Promise<void> {
  try {
    const [settings, catalog, downloads, profiles] = await Promise.all([
      invoke<DesktopSettings>("desktop_settings"),
      availableModels.length ? Promise.resolve(availableModels) : invoke<ModelCatalogItem[]>("model_catalog"),
      invoke<ModelDownloadState[]>("model_download_states"),
      invoke<TranslationProfileSummary[]>("translation_profiles"),
    ]);
    updateProviderProfiles(profiles);
    availableModels = catalog;
    modelDownloads = downloads;
    renderDesktopSettings(settings);
    await loadDictionarySettings();
    if (openWhenNeeded && settings.needsOnboarding && !settingsDialog?.open) settingsDialog?.showModal();
  } catch (error) {
    if (settingsMessage) settingsMessage.textContent = `无法读取启动配置：${String(error)}`;
  }
}

async function refreshTranslationStatus(): Promise<void> {
  await taskProvider.refresh();
}

async function saveSettings(finishOnboarding: boolean): Promise<void> {
  if (!settingsWhisperModel || !settingsVadModel || !settingsHyMt2Model || !settingsProvider || !settingsApiKey || !settingsProxyMode || !settingsProxyUrl || !settingsModelMirror || !settingsProviderModel || !settingsProviderBaseUrl || !settingsTranslationStyle) return;
  if (finishOnboarding && !settingsWhisperModel.value.trim()) {
    if (settingsMessage) settingsMessage.textContent = "请先选择或下载一个 Whisper 模型。";
    settingsWhisperModel.focus();
    return;
  }
  if (settingsMessage) settingsMessage.textContent = "正在保存配置…";
  try {
    const settings = await invoke<DesktopSettings>("save_desktop_settings", {
      request: {
        whisperModelPath: settingsWhisperModel.value.trim() || null,
        vadModelPath: settingsVadModel.value.trim() || null,
        hyMt2ModelPath: settingsHyMt2Model.value.trim() || null,
        translationProviderId: settingsProvider.value,
        translationModel: settingsProviderModel.value.trim() || null,
        translationBaseUrl: settingsProviderBaseUrl.value.trim() || null,
        translationStyleInstruction: settingsTranslationStyle.value.trim() || null,
        apiKey: settingsApiKey.value.trim() || null,
        clearApiKey: settingsClearApiKey?.checked ?? false,
        networkProxyMode: settingsProxyMode.value,
        networkProxyUrl: settingsProxyUrl.value.trim() || null,
        modelMirrorUrl: settingsModelMirror.value.trim() || null,
        onboardingCompleted: finishOnboarding || desktopSettings?.onboardingCompleted || false,
      },
    });
    settingsDirtyFields.clear();
    providerDirtyDrafts.delete(settingsProvider.value);
    updateProviderProfiles(await invoke<TranslationProfileSummary[]>("translation_profiles"));
    renderDesktopSettings(settings);
    await refreshTranslationStatus();
    if (settingsMessage) settingsMessage.textContent = "配置已保存；如提供密钥，只会写入系统凭据库。";
    if (finishOnboarding && !settings.needsOnboarding) settingsDialog?.close();
  } catch (error) {
    if (settingsMessage) settingsMessage.textContent = `保存失败：${String(error)}`;
  }
}

async function testNetworkConnection(): Promise<void> {
  if (!settingsProxyMode || !settingsProxyUrl || !settingsModelMirror || !testNetworkButton || !networkTestResults) return;
  testNetworkButton.disabled = true;
  networkTestResults.innerHTML = "<span>正在测试模型来源…</span>";
  try {
    const checks = await invoke<NetworkSourceCheck[]>("test_network_connection", {
      request: {
        proxyMode: settingsProxyMode.value,
        proxyUrl: settingsProxyUrl.value.trim() || null,
        modelMirrorUrl: settingsModelMirror.value.trim() || null,
      },
    });
    networkTestResults.innerHTML = checks.map((check) => {
      const detail = check.ok
        ? `${check.status ?? "已连接"}${check.resolvedHost ? ` · ${check.resolvedHost}` : ""}`
        : check.error ?? "连接失败";
      return `<div class="${check.ok ? "success" : "warning"}"><strong>${escapeHtml(check.label)}</strong><span>${escapeHtml(String(detail))}</span></div>`;
    }).join("");
  } catch (error) {
    networkTestResults.innerHTML = `<div class="warning"><strong>配置无效</strong><span>${escapeHtml(String(error))}</span></div>`;
  } finally {
    testNetworkButton.disabled = false;
  }
}

async function chooseSettingsModel(kind: "whisper" | "vad" | "hy-mt2"): Promise<void> {
  const command = kind === "whisper"
    ? "pick_model_file"
    : kind === "vad" ? "pick_vad_model_file" : "pick_hy_mt2_model_file";
  try {
    const path = await invoke<string | null>(command);
    const input = kind === "whisper"
      ? settingsWhisperModel
      : kind === "vad" ? settingsVadModel : settingsHyMt2Model;
    if (path && input) {
      input.value = path;
      settingsDirtyFields.add(input.id);
    }
  } catch (error) {
    if (settingsMessage) settingsMessage.textContent = `无法打开模型选择器：${String(error)}`;
  }
}

async function startModelDownload(modelId: string): Promise<void> {
  if (!modelId) return;
  if (!settingsProxyMode || !settingsProxyUrl || !settingsModelMirror) return;
  if (modelDownloadMessage) modelDownloadMessage.textContent = "正在保存当前网络配置并连接模型来源…";
  try {
    await invoke<void>("save_download_network_settings", {
      request: {
        proxyMode: settingsProxyMode.value,
        proxyUrl: settingsProxyUrl.value.trim() || null,
        modelMirrorUrl: settingsModelMirror.value.trim() || null,
      },
    });
    settingsDirtyFields.delete(settingsProxyMode.id);
    settingsDirtyFields.delete(settingsProxyUrl.id);
    settingsDirtyFields.delete(settingsModelMirror.id);
    const state = await invoke<ModelDownloadState>("start_model_download", { modelId });
    modelDownloads = modelDownloads.filter((download) => download.modelId !== modelId);
    modelDownloads.push(state);
    renderModelCatalog();
    if (modelDownloadPoll === null) {
      modelDownloadPoll = window.setInterval(() => void refreshModelDownloads(), 500);
    }
  } catch (error) {
    if (modelDownloadMessage) modelDownloadMessage.textContent = `无法开始下载：${String(error)}`;
  }
}

async function refreshModelDownloads(): Promise<void> {
  try {
    modelDownloads = await invoke<ModelDownloadState[]>("model_download_states");
    renderModelCatalog();
    const active = modelDownloads.some((download) =>
      download.status === "queued" || download.status === "downloading" || download.status === "cancelling"
    );
    if (!active) {
      if (modelDownloadPoll !== null) window.clearInterval(modelDownloadPoll);
      modelDownloadPoll = null;
      await loadDesktopSettings(false);
      const completed = modelDownloads.find((download) => download.status === "done");
      const failed = modelDownloads.find((download) => download.status === "failed");
      if (modelDownloadMessage) {
        modelDownloadMessage.textContent = completed
          ? "模型已校验并安装，路径已自动填入。"
          : failed ? `下载失败：${failed.error ?? "未知错误"}` : "";
      }
    }
  } catch (error) {
    if (modelDownloadMessage) modelDownloadMessage.textContent = `无法读取下载状态：${String(error)}`;
  }
}

async function cancelModelDownload(modelId: string): Promise<void> {
  if (!modelId) return;
  try {
    const state = await invoke<ModelDownloadState>("cancel_model_download", { modelId });
    modelDownloads = modelDownloads.filter((download) => download.modelId !== modelId);
    modelDownloads.push(state);
    renderModelCatalog();
  } catch (error) {
    if (modelDownloadMessage) modelDownloadMessage.textContent = `无法取消下载：${String(error)}`;
  }
}

async function uninstallModel(modelId: string): Promise<void> {
  if (!modelId) return;
  try {
    await invoke<void>("uninstall_model", { modelId });
    modelDownloads = modelDownloads.filter((download) => download.modelId !== modelId);
    await loadDesktopSettings(false);
    if (modelDownloadMessage) modelDownloadMessage.textContent = "模型及其未完成下载已删除。";
  } catch (error) {
    if (modelDownloadMessage) modelDownloadMessage.textContent = `无法卸载模型：${String(error)}`;
  }
}

async function selectModel(modelId: string): Promise<void> {
  if (!modelId) return;
  const model = availableModels.find((candidate) => candidate.id === modelId);
  if (modelDownloadMessage) modelDownloadMessage.textContent = "正在校验并切换模型…";
  try {
    await invoke<ModelDownloadState>("select_model", { modelId });
    if (model?.kind === "whisper" && settingsWhisperModel) {
      settingsDirtyFields.delete(settingsWhisperModel.id);
    } else if (model?.kind === "vad" && settingsVadModel) {
      settingsDirtyFields.delete(settingsVadModel.id);
    } else if (model?.kind === "hy-mt2" && settingsHyMt2Model) {
      settingsDirtyFields.delete(settingsHyMt2Model.id);
    }
    modelDownloads = await invoke<ModelDownloadState[]>("model_download_states");
    await loadDesktopSettings(false);
    if (modelDownloadMessage) modelDownloadMessage.textContent = "模型已校验并设为当前使用。";
  } catch (error) {
    if (modelDownloadMessage) modelDownloadMessage.textContent = `无法选择模型：${String(error)}`;
  }
}

function renderGlossaryOptions(): void {
  const taskSelection = taskGlossary?.value ?? "";
  const workspaceSelection = workspaceGlossary?.value ?? "";
  const taskGlossaries = glossaries.filter((glossary) => glossary.source_language === selectedSourceLanguage());
  const workspaceGlossaries = glossaries.filter((glossary) => glossary.source_language === activeSourceLanguage());
  const taskOptions = taskGlossaries
    .map((glossary) => `<option value="${escapeHtml(glossary.id)}">${escapeHtml(glossary.name)}（核心 ${glossary.core_term_count}／内容包 ${glossary.content_group_count}／仅修正 ${glossary.correction_only_count}）</option>`)
    .join("");
  const workspaceOptions = workspaceGlossaries
    .map((glossary) => `<option value="${escapeHtml(glossary.id)}">${escapeHtml(glossary.name)}（核心 ${glossary.core_term_count}／内容包 ${glossary.content_group_count}／仅修正 ${glossary.correction_only_count}）</option>`)
    .join("");
  if (taskGlossary) {
    taskGlossary.innerHTML = `<option value="">不使用词表</option>${taskOptions}`;
    if (taskGlossaries.some((glossary) => glossary.id === taskSelection)) taskGlossary.value = taskSelection;
  }
  if (workspaceGlossary) {
    workspaceGlossary.innerHTML = `<option value="">选择词表…</option>${workspaceOptions}`;
    const preferred = workspaceGlossaries.some((glossary) => glossary.id === workspaceSelection)
      ? workspaceSelection
      : activeDetail?.job.glossary_id ?? "";
    workspaceGlossary.value = workspaceGlossaries.some((glossary) => glossary.id === preferred) ? preferred : "";
  }
  renderGlossaryList();
}

async function renderWorkspaceGlossaryInspection(): Promise<void> {
  const glossaryId = workspaceGlossary?.value;
  if (!workspaceGlossaryMappings) return;
  if (!glossaryId) {
    workspaceGlossaryMappings.classList.add("muted");
    workspaceGlossaryMappings.textContent = "请选择词表。";
    return;
  }
  workspaceGlossaryMappings.classList.add("muted");
  workspaceGlossaryMappings.textContent = "正在读取修正规则…";
  try {
    const detail = await invoke<GlossaryDetail>("get_glossary", { glossaryId });
    const mappings = detail.terms.filter((term) => term.target_text?.trim());
    workspaceGlossaryMappings.classList.toggle("muted", mappings.length === 0);
    workspaceGlossaryMappings.innerHTML = mappings.length
      ? mappings.map((term) => `<div><span>${escapeHtml(term.source_text)}</span><b aria-hidden="true">→</b><span>${escapeHtml(term.target_text ?? "")}</span></div>`).join("")
      : "这个词表没有识别后修正规则，只会作为新任务的 Whisper 提示词。";
  } catch (error) {
    workspaceGlossaryMappings.classList.add("muted");
    workspaceGlossaryMappings.textContent = `读取失败：${String(error)}`;
  }
}

async function renderJobGlossarySnapshot(jobId: string): Promise<void> {
  if (!jobGlossarySnapshot) return;
  jobGlossarySnapshot.textContent = "正在读取任务快照…";
  try {
    const snapshot = await invoke<string | null>("get_job_glossary_snapshot", { jobId });
    jobGlossarySnapshot.textContent = snapshot?.trim() || "当前任务没有词表快照。";
  } catch (error) {
    jobGlossarySnapshot.textContent = `快照读取失败：${String(error)}`;
  }
}

async function refreshTaskGlossaryConfiguration(): Promise<void> {
  const glossaryId = taskGlossary?.value || null;
  if (!glossaryId) {
    taskGlossaryConfigurationId = null;
    selectedTaskContentGroups.clear();
    taskGlossaryConfiguration?.classList.add("hidden");
    if (taskContentPacks) taskContentPacks.replaceChildren();
    return;
  }
  if (taskGlossaryConfigurationId !== glossaryId) {
    taskGlossaryConfigurationId = glossaryId;
    selectedTaskContentGroups = new Set<string>();
  }
  taskGlossaryConfiguration?.classList.remove("hidden");
  if (taskPromptPreview) taskPromptPreview.textContent = "正在生成预览…";
  try {
    const detail = await invoke<GlossaryDetail>("get_glossary", { glossaryId });
    const groups = Array.from(
      new Set(
        detail.terms
          .filter((term) => term.prompt_scope === "content" && term.content_group)
          .map((term) => term.content_group as string),
      ),
    ).sort((left, right) => left.localeCompare(right, "zh-CN"));
    selectedTaskContentGroups = new Set(
      Array.from(selectedTaskContentGroups).filter((group) => groups.includes(group)),
    );
    if (taskContentPacks) {
      taskContentPacks.innerHTML = groups.length
        ? groups
            .map(
              (group) => `<label><input type="checkbox" value="${escapeHtml(group)}"${selectedTaskContentGroups.has(group) ? " checked" : ""} />${escapeHtml(group)}</label>`,
            )
            .join("")
        : `<span class="muted">这个词表没有内容包，只会使用核心词条和识别后修正。</span>`;
      taskContentPacks.querySelectorAll<HTMLInputElement>("input[type=checkbox]").forEach((checkbox) => {
        checkbox.addEventListener("change", () => {
          if (checkbox.checked) selectedTaskContentGroups.add(checkbox.value);
          else selectedTaskContentGroups.delete(checkbox.value);
          void refreshTaskPromptPreview();
        });
      });
    }
    await refreshTaskPromptPreview();
  } catch (error) {
    if (taskPromptPreview) taskPromptPreview.textContent = `无法生成 prompt：${String(error)}`;
  }
}

async function refreshTaskPromptPreview(): Promise<void> {
  const glossaryId = taskGlossary?.value;
  if (!glossaryId || !taskPromptPreview) return;
  try {
    const preview = await invoke<GlossaryPromptPreview>("preview_glossary_prompt", {
      request: {
        glossaryId,
        selectedContentGroups: Array.from(selectedTaskContentGroups),
      },
    });
    taskPromptPreview.textContent = preview.prompt || "当前选择不会向 Whisper 发送词表提示。";
    if (taskPromptSummary) {
      taskPromptSummary.textContent = `核心 ${preview.core_term_count} · 已选内容 ${preview.selected_content_term_count} · prompt ${preview.included_prompt_term_count} 词／${preview.prompt_character_count} 字 · 仅修正 ${preview.correction_only_count}`;
    }
  } catch (error) {
    taskPromptPreview.textContent = `无法生成 prompt：${String(error)}`;
  }
}

async function refreshGlossaries(): Promise<void> {
  try {
    glossaries = await invoke<Glossary[]>("list_glossaries");
    renderGlossaryOptions();
    await refreshTaskGlossaryConfiguration();
  } catch (error) {
    if (taskMessage) taskMessage.textContent = `无法读取识别词表：${String(error)}`;
  }
}

function renderGlossaryList(): void {
  if (!glossaryListHost) return;
  if (glossaries.length === 0) {
    glossaryListHost.innerHTML = `<p class="muted">还没有词表。</p>`;
    return;
  }
  glossaryListHost.innerHTML = glossaries
    .map(
      (glossary) => `<button type="button" class="glossary-list-item${glossary.id === editingGlossaryId ? " active" : ""}" data-glossary-id="${escapeHtml(glossary.id)}"><strong>${escapeHtml(glossary.name)}${glossary.builtin_key ? ' <span class="builtin-badge">内置</span>' : ""}</strong><span>${languageLabel(glossary.source_language)} · 核心 ${glossary.core_term_count} · 内容 ${glossary.content_group_count} 包 · 仅修正 ${glossary.correction_only_count}</span></button>`,
    )
    .join("");
  glossaryListHost.querySelectorAll<HTMLButtonElement>("[data-glossary-id]").forEach((button) => {
    button.addEventListener("click", () => void editGlossary(button.dataset.glossaryId ?? null));
  });
}

function updateTranslationControls(): void {
  const hasSegments = (activeDetail?.segments.length ?? 0) > 0;
  const taskTranslationBusy = activeDetail
    ? subtitleTranslation.has(activeDetail.job.job_id)
    : false;
  const taskBusy = workspaceActionBusy || taskTranslationBusy;
  const singleTranslate = editorRoot.querySelector<HTMLButtonElement>('[data-editor-action="translate"]');
  if (singleTranslate) { singleTranslate.disabled = taskBusy || !hasSegments || !translationStatus.configured; singleTranslate.title = translationStatus.configured ? `使用 ${translationStatus.provider} 翻译当前句` : "请在翻译与词表中配置翻译方式"; }
  if (translationStatusText) {
    const model = translationStatus.model ? ` · ${translationStatus.model}` : "";
    const source = languageLabel(activeSourceLanguage());
    const target = languageLabel(activeTargetLanguage());
    translationStatusText.textContent = translationStatus.configured
      ? `${translationStatus.provider}${model} 已配置 · ${source}原文会发送到云端翻译为${target}`
      : `未配置 ${translationStatus.provider}；${translationStatus.configuration_hint ?? "请完成翻译服务配置"}`;
    translationStatusText.classList.toggle("warning", !translationStatus.configured);
  }
  if (translationRunStatusText) {
    const latest = activeDetail?.translation_runs[0];
    if (!latest) {
      translationRunStatusText.textContent = "当前任务还没有机器翻译运行记录。";
    } else {
      const model = latest.model ? ` · ${latest.model}` : "";
      const usageLabel = translationUsageLabel(latest);
      const tokens = usageLabel ? ` · ${usageLabel}` : "";
      translationRunStatusText.textContent = `最近批次：${latest.provider_name}${model} · ${latest.segment_count} 段${tokens} · ${new Date(latest.completed_at_unix * 1_000).toLocaleString()}`;
    }
  }
  if (translatePendingButton) {
    translatePendingButton.disabled = taskBusy || !hasSegments || !translationStatus.configured ||
      !activeDetail?.segments.some(segment => segmentNeedsTranslation(segment));
  }
  if (translateAllButton) {
    translateAllButton.disabled = taskBusy || !hasSegments || !translationStatus.configured;
  }
  if (exportButton) exportButton.disabled = taskBusy || !hasSegments;
  if (renderVideoButton) {
    const inputPath = activeDetail?.job.input_path;
    renderVideoButton.disabled =
      taskBusy || !hasSegments || !inputPath || !activeDetail?.playback_path || isAudioPath(inputPath);
    renderVideoButton.title = inputPath && isAudioPath(inputPath)
      ? "音频任务不能烧录视频"
      : inputPath && !activeDetail?.playback_path
        ? "原视频已移动，请先在字幕编辑中重新定位"
        : "";
  }
  if (revealExportButton) {
    revealExportButton.disabled = taskBusy || !subtitleExport.lastPath;
  }
  const previewButton = workspaceView.querySelector<HTMLButtonElement>("#preview-glossary");
  if (previewButton) {
    previewButton.disabled = taskBusy || !hasSegments || !workspaceGlossary?.value;
  }
  subtitleList?.querySelectorAll<HTMLButtonElement>(".translate-segment").forEach((button) => {
    button.disabled = taskBusy || !translationStatus.configured;
  });
  subtitleList?.querySelectorAll<HTMLButtonElement>(".capture-glossary-term").forEach((button) => {
    button.disabled = taskBusy || !workspaceGlossary?.value;
  });
  if (relinkJobMediaButton) relinkJobMediaButton.disabled = taskBusy;
}

function setWorkspaceAction(message: string, isError = false): void {
  if (currentArea === "karaoke" && workspaceView?.classList.contains("hidden")) setSubtitleEditAction(message, isError);
  if (!workspaceActionMessage) return;
  workspaceActionMessage.textContent = message;
  workspaceActionMessage.classList.toggle("warning", isError);
}

function setSubtitleEditAction(message: string, isError = false): void {
  if (currentArea === "karaoke" && karaokeTimingMessage) {
    karaokeTimingMessage.textContent = message;
    karaokeTimingMessage.classList.toggle("warning", isError);
    return;
  }
  setWorkspaceAction(message, isError);
}

function setWorkspaceBusy(busy: boolean): void {
  workspaceActionBusy = busy;
  if (relinkJobMediaButton) relinkJobMediaButton.disabled = busy;
  updateTranslationControls();
}

function hasUnsavedSubtitleEdits(): boolean {
  return [...subtitleDrafts.entries.values()].some(draft => draft.saved.job_id === activeDetail?.job.job_id && subtitleDrafts.pending(draft));
}

function renderJobs(jobs: LocalJob[]): void {
  jobs = jobs.filter(job => matchesQuery(taskTitleQuery,[displayName(job)]));
  if (!jobList || !jobCount) return;
  jobCount.textContent = `${jobs.length} 个任务`;
  if (jobs.length === 0) {
    jobList.innerHTML = `<div class="empty-state"><strong>还没有本地任务。</strong><span>选择媒体与模型，创建第一个离线转写任务。</span></div>`;
    return;
  }

  jobList.innerHTML = jobs
    .map(
      (job) => `
        <article class="job-card" data-job-card="${escapeHtml(job.job_id)}">
          <button class="job-drag-handle" type="button" aria-label="拖动调整${escapeHtml(displayName(job))}的顺序" title="拖动排序；聚焦后也可用上下方向键">⠿</button>
          <button class="job-open" data-job-id="${escapeHtml(job.job_id)}" type="button">
            <div><h3>${escapeHtml(displayName(job))}</h3><p>${escapeHtml(job.message)} · <span data-job-timing data-processing-time="${escapeHtml(JSON.stringify(job.processing_time ?? null))}" data-created-at="${job.created_at_unix}" data-updated-at="${job.updated_at_unix}" data-started-at="${job.started_at_unix ?? ""}" data-completed-at="${job.completed_at_unix ?? ""}" data-status="${escapeHtml(job.status)}">${escapeHtml(jobTimingLabel(job))}</span></p>${runningJob(job) ? '<progress class="job-progress"></progress>' : ""}</div>
            <span class="job-statuses">
              <span class="status status-${escapeHtml(job.status)}">${escapeHtml(job.error_message?.includes("已暂停") ? "已暂停" : statusLabel(job.status))}</span>
              <span class="status translation-status translation-status-${escapeHtml(job.translation_status)}">${escapeHtml(translationStatusLabel(job))}</span>
            </span>
          </button>
          <div class="job-actions">
            ${job.status !== "done" && job.recovery?.early_translation ? `<span title="完整识别后会核对原文与上下文，再写入译文">${job.status === "translating" ? "识别已完成 · 正在完成翻译" : `已提前翻译 ${job.recovery.early_translation.translated_cues} 段${escapeHtml(earlyTranslationLabel(job.status, job.recovery.early_translation.error))}`}</span>` : ""}
            ${job.recovery?.duration_ms ? `<span>${job.status==="failed" ? `${escapeHtml(statusLabel(job.recovery.phase))} · ` : ""}${formatTime(job.recovery.completed_ms)} / ${formatTime(job.recovery.duration_ms)}${job.recovery_progress_unverified ? " · 检查点待校验" : ""}</span><button type="button" class="secondary" data-partial-job="${escapeHtml(job.job_id)}">查看已保存原文</button>` : ""}
            ${job.status === "failed" && job.recovery?.can_resume ? `<button type="button" data-resume-job="${escapeHtml(job.job_id)}">继续</button>` : ""}
            ${runningJob(job) ? `<button type="button" class="secondary" data-pause-job="${escapeHtml(job.job_id)}">暂停</button>` : ""}
            ${job.status === "failed" ? `<button type="button" class="secondary" data-retry-job="${escapeHtml(job.job_id)}">重新运行</button>` : ""}
            <button type="button" class="secondary" data-rename-job="${escapeHtml(job.job_id)}">重命名</button>
            <button type="button" class="danger" data-delete-job="${escapeHtml(job.job_id)}" ${matchesTerminalStatus(job.status) ? "" : "disabled"} title="${matchesTerminalStatus(job.status) ? "删除任务数据，但保留原始媒体" : "任务结束后才能删除"}">删除</button>
          </div>
          ${job.status === "failed" && job.error_message && !job.error_message.includes("已暂停") ? `<details class="job-error"><summary>查看失败原因</summary><pre>${escapeHtml(job.error_message)}</pre><p>失败不会删除已保存的结果。${job.recovery?.can_resume ? "点击“继续”核验检查点并从未完成处重试。" : ""}</p></details>` : ""}
        </article>`,
    )
    .join("");
  if (!taskTitleQuery.trim()) enableJobOrdering();
  for (const [attribute, command] of [["resume", "resume_task"], ["pause", "pause_task"]]) {
    jobList.querySelectorAll<HTMLButtonElement>(`[data-${attribute}-job]`).forEach(button => button.addEventListener("click", async () => {
      button.disabled = true;
      try { await invoke(command, {jobId: button.getAttribute(`data-${attribute}-job`)}); await refresh(); }
      catch (error) { if (jobManagementMessage) jobManagementMessage.textContent = String(error); }
      finally { button.disabled = false; }
    }));
  }
  jobList.querySelectorAll<HTMLButtonElement>("[data-partial-job]").forEach(button => button.addEventListener("click", () => void showPartialTranscript(button.dataset.partialJob ?? "")));
  jobList.querySelectorAll<HTMLButtonElement>("[data-job-id]").forEach((button) => {
    button.addEventListener("click", () => {
      const job = jobs.find(j => j.job_id === button.dataset.jobId);
      if (job && job.status !== "done" && job.recovery?.duration_ms && (job.recovery.completed_ms < job.recovery.duration_ms || job.segment_count === 0)) void showPartialTranscript(job.job_id);
      else void openJob(button.dataset.jobId ?? "");
    });
  });
  jobList.querySelectorAll<HTMLButtonElement>("[data-rename-job]").forEach((button) => {
    button.addEventListener("click", () => {
      const job = jobs.find((item) => item.job_id === button.dataset.renameJob);
      if (job) renameJob(job);
    });
  });
  jobList.querySelectorAll<HTMLButtonElement>("[data-retry-job]").forEach((button) => {
    button.addEventListener("click", () => {
      const job = jobs.find((item) => item.job_id === button.dataset.retryJob);
      if (job) void retryJob(job);
    });
  });
  jobList.querySelectorAll<HTMLButtonElement>("[data-delete-job]").forEach((button) => {
    button.addEventListener("click", () => {
      const job = jobs.find((item) => item.job_id === button.dataset.deleteJob);
      if (job) void deleteJob(job);
    });
  });
}

function jobCardsInDisplayOrder(): HTMLElement[] {
  return Array.from(jobList?.querySelectorAll<HTMLElement>("[data-job-card]") ?? []);
}

async function persistDisplayedJobOrder(): Promise<void> {
  if (jobOrderBusy) return;
  const jobIds = jobCardsInDisplayOrder().map((card) => card.dataset.jobCard ?? "");
  if (jobIds.some((jobId) => !jobId) || jobIds.length !== latestJobs.length) return;
  if (jobIds.every((jobId, index) => latestJobs[index]?.job_id === jobId)) return;
  jobOrderBusy = true;
  if (jobManagementMessage) jobManagementMessage.textContent = "正在保存任务顺序…";
  try {
    await invoke<void>("reorder_jobs", { jobIds });
    const jobsById = new Map(latestJobs.map((job) => [job.job_id, job]));
    latestJobs = jobIds.flatMap((jobId) => {
      const job = jobsById.get(jobId);
      return job ? [job] : [];
    });
    renderedJobsFingerprint = JSON.stringify(latestJobs);
    renderListeningJobs(latestJobs);
    if (jobManagementMessage) jobManagementMessage.textContent = "任务顺序已保存。";
  } catch (error) {
    if (jobManagementMessage) jobManagementMessage.textContent = `保存任务顺序失败：${String(error)}`;
    renderedJobsFingerprint = null;
    jobOrderBusy = false;
    await refresh();
  } finally {
    jobOrderBusy = false;
  }
}

function enableJobOrdering(): void {
  if (!jobList) return;
  jobCardsInDisplayOrder().forEach((card) => {
    const handle = card.querySelector<HTMLElement>(".job-drag-handle");
    if (!handle) return;
    let mouseActive = false;
    let startY = 0;
    let moved = false;
    let suppressOpenClick = false;
    const finishMouseOrder = (): void => {
      if (!mouseActive) return;
      mouseActive = false;
      window.removeEventListener("mousemove", moveMouseOrder);
      window.removeEventListener("mouseup", finishMouseOrder);
      window.removeEventListener("blur", finishMouseOrder);
      card.classList.remove("drag-armed", "dragging");
      jobCardsInDisplayOrder().forEach((jobCard) => jobCard.classList.remove("drag-target"));
      if (moved) {
        suppressOpenClick = true;
        window.setTimeout(() => { suppressOpenClick = false; }, 0);
        void persistDisplayedJobOrder();
      } else if (jobManagementMessage?.textContent?.startsWith("正在调整任务顺序")) {
        jobManagementMessage.textContent = "";
      }
      moved = false;
    };
    const moveMouseOrder = (event: MouseEvent): void => {
      if (!mouseActive) return;
      if ((event.buttons & 1) === 0) {
        finishMouseOrder();
        return;
      }
      if (!moved && Math.abs(event.clientY - startY) < 4) return;
      event.preventDefault();
      moved = true;
      card.classList.remove("drag-armed");
      card.classList.add("dragging");
      const target = jobCardsInDisplayOrder()
        .filter((jobCard) => jobCard !== card)
        .reduce<HTMLElement | null>((nearest, jobCard) => {
          if (!nearest) return jobCard;
          const distance = Math.abs(event.clientY - (jobCard.getBoundingClientRect().top + jobCard.offsetHeight / 2));
          const nearestDistance = Math.abs(event.clientY - (nearest.getBoundingClientRect().top + nearest.offsetHeight / 2));
          return distance < nearestDistance ? jobCard : nearest;
        }, null);
      if (target) {
        const before = event.clientY < target.getBoundingClientRect().top + target.offsetHeight / 2;
        jobList.insertBefore(card, before ? target : target.nextElementSibling);
        jobCardsInDisplayOrder().forEach((jobCard) => jobCard.classList.toggle("drag-target", jobCard === target));
      }
      const edge = 52;
      if (event.clientY < edge) window.scrollBy({ top: -18, behavior: "auto" });
      else if (event.clientY > window.innerHeight - edge) window.scrollBy({ top: 18, behavior: "auto" });
    };
    card.addEventListener("mousedown", (event) => {
      if (event.button !== 0) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest(".job-actions")) return;
      if (target?.closest(".job-drag-handle")) event.preventDefault();
      mouseActive = true;
      startY = event.clientY;
      moved = false;
      card.classList.add("drag-armed");
      if (jobManagementMessage) jobManagementMessage.textContent = "正在调整任务顺序；移动到目标位置后松开鼠标。";
      window.addEventListener("mousemove", moveMouseOrder);
      window.addEventListener("mouseup", finishMouseOrder);
      window.addEventListener("blur", finishMouseOrder);
    });
    card.addEventListener("click", (event) => {
      if (!suppressOpenClick || !(event.target instanceof Element) || !event.target.closest(".job-open")) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      suppressOpenClick = false;
    }, true);
    handle.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
      event.preventDefault();
      const sibling = event.key === "ArrowUp" ? card.previousElementSibling : card.nextElementSibling;
      if (!(sibling instanceof HTMLElement)) return;
      if (event.key === "ArrowUp") jobList.insertBefore(card, sibling);
      else jobList.insertBefore(sibling, card);
      handle.focus();
      void persistDisplayedJobOrder();
    });
  });
}

function renderListeningJobs(jobs: LocalJob[]): void {
  if (!listeningJobList || !listeningJobCount) return;
  const translated = jobs.filter((job) => job.status === "done" && job.translation_status === "translated");
  listeningJobCount.textContent = `${translated.length} 个`;
  listeningJobList.innerHTML = translated.length
    ? translated.map((job) => `<button type="button" class="listening-job${activeDetail?.job.job_id === job.job_id && currentArea === "listening" ? " active" : ""}" data-listen-job="${escapeHtml(job.job_id)}"><strong>${escapeHtml(displayName(job))}</strong><span>${escapeHtml(languageLabel(job.source_language))} → ${escapeHtml(languageLabel(job.target_language))} · ${job.segment_count} 段</span></button>`).join("")
    : `<div class="empty-state"><strong>还没有可收听任务。</strong><span>完整翻译的任务会自动出现在这里。</span></div>`;
  listeningJobList.querySelectorAll<HTMLButtonElement>("[data-listen-job]").forEach((button) => {
    button.addEventListener("click", () => void openListeningJob(button.dataset.listenJob ?? ""));
  });
}

function translationStatusLabel(job: LocalJob): string {
  if (job.translation_status === "translated") return "已翻译";
  if (job.translation_status === "partial") {
    return `部分翻译 ${job.translated_segment_count}/${job.segment_count}`;
  }
  if (job.translation_status === "stale") return `待重译 ${job.stale_translation_count}`;
  if (job.translation_status === "untranslated") return "未翻译";
  return "尚无字幕";
}

async function retryJob(job: LocalJob): Promise<void> {
  if (job.status !== "failed") return;
  if (jobManagementMessage) jobManagementMessage.textContent = "正在从冻结参数创建重试任务…";
  try {
    const newJobId = await invoke<string>("retry_job", { jobId: job.job_id });
    if (jobManagementMessage) {
      jobManagementMessage.textContent = `已创建新的重试任务 ${newJobId}；失败任务保留不变。`;
    }
    await refresh();
  } catch (error) {
    if (jobManagementMessage) jobManagementMessage.textContent = `重试失败：${String(error)}`;
  }
}

function matchesTerminalStatus(status: string): boolean {
  return status === "done" || status === "failed";
}

function renameJob(job: LocalJob): void {
  renamingJob = job;
  if (renameJobInput) renameJobInput.value = job.display_name ?? displayName(job);
  if (renameJobMessage) renameJobMessage.textContent = "";
  renameJobDialog?.showModal();
  renameJobInput?.focus();
  renameJobInput?.select();
}

async function saveRenamedJob(): Promise<void> {
  if (!renamingJob || !renameJobInput) return;
  if (renameJobMessage) renameJobMessage.textContent = "正在保存…";
  try {
    await invoke<LocalJob>("rename_job", {
      jobId: renamingJob.job_id,
      displayName: renameJobInput.value.trim() || null,
    });
    if (jobManagementMessage) jobManagementMessage.textContent = "任务名称已保存。";
    renameJobDialog?.close();
    renamingJob = null;
    await refresh();
  } catch (error) {
    if (renameJobMessage) renameJobMessage.textContent = `保存失败：${String(error)}`;
  }
}

async function deleteJob(job: LocalJob): Promise<void> {
  if (!matchesTerminalStatus(job.status)) return;
  const confirmed = await confirmAction({
    title: "删除任务数据？",
    message: `将删除“${displayName(job)}”在 Atogaki 中的音频、字幕、中间产物和本地记录。原始媒体文件不会被删除。`,
    confirmLabel: "删除任务",
    danger: true,
  });
  if (!confirmed) return;
  if (jobManagementMessage) jobManagementMessage.textContent = "正在删除任务数据…";
  try {
    await subtitleDrafts.flushAll();
    await invoke("delete_job", { jobId: job.job_id });
    subtitleDrafts.forgetJob(job.job_id);
    try { localStorage.setItem("atogaki.subtitleDrafts", subtitleDrafts.snapshot()); } catch { /* Session state was cleared. */ }
    if (jobManagementMessage) jobManagementMessage.textContent = "任务已删除，原始媒体文件未改变。";
    await refresh();
  } catch (error) {
    if (jobManagementMessage) jobManagementMessage.textContent = `删除失败：${String(error)}`;
  }
}

async function refresh(): Promise<void> {
  if (!jobList || refreshing || jobOrderBusy || (currentArea !== "workbench" && currentArea !== "listening")) return;
  refreshing = true;
  if (!activeDetail && renderedJobsFingerprint === null && jobList.childElementCount === 0) {
    jobList.innerHTML = `<div class="empty-state">正在读取任务…</div>`;
  }
  try {
    const jobs = await invoke<LocalJob[]>("list_job_summaries");
    latestJobs = jobs;
    const fingerprint = JSON.stringify(jobs);
    if (currentArea !== "workbench" && currentArea !== "listening") return;
    if (fingerprint !== renderedJobsFingerprint) {
      const scrollTop = window.scrollY;
      renderJobs(jobs);
      renderListeningJobs(jobs);
      renderedJobsFingerprint = fingerprint;
      if (!activeDetail) window.scrollTo({ top: scrollTop, behavior: "auto" });
    }
  } catch (error) {
    jobList.innerHTML = `<div class="empty-state error">无法读取本地任务：${escapeHtml(String(error))}</div>`;
    renderedJobsFingerprint = null;
  } finally {
    refreshing = false;
  }
}

async function refreshActiveJob(): Promise<void> {
  const current = activeDetail;
  if (!current || matchesTerminalStatus(current.job.status) || hasUnsavedSubtitleEdits()) return;
  try {
    const detail = await invoke<JobDetail>("get_job_detail", { jobId: current.job.job_id });
    if (!activeDetail || activeDetail.job.job_id !== detail.job.job_id) return;
    const completedNow = matchesTerminalStatus(detail.job.status);
    activeDetail.job = detail.job;
    if (workspaceMessage) {
      workspaceMessage.textContent = `${statusLabel(detail.job.status)} · ${detail.job.message} · ${jobTimingLabel(detail.job)}`;
    }
    if (completedNow || detail.segments.length !== activeDetail.segments.length) {
      activeDetail = detail;
      renderWorkspace(detail);
    }
  } catch (error) {
    if (workspaceMessage) workspaceMessage.textContent = `自动刷新失败：${String(error)}`;
  }
}

function showWorkspace(show: boolean): void {
  shell?.classList.toggle("workspace-open", show);
  homeView?.classList.toggle("hidden", show);
  if (show) taskTools.mountPage(); else taskTools.unmount();
  listeningView?.classList.add("hidden");
  learningView?.classList.add("hidden");
  karaokeView?.classList.add("hidden");
  if (show) currentArea = "workspace";
  document.querySelector<HTMLButtonElement>("#refresh")?.classList.toggle("hidden", show);
}

async function stopActivePlayback(): Promise<void> {
  if (workspaceActionBusy) throw new Error("请等待当前字幕操作完成后再离开");
  if ([...subtitleDrafts.entries.values()].some(draft => draft.saved.job_id === activeDetail?.job.job_id && draft.composing)) throw new Error("请先完成输入法选字，再离开编辑器");
  await subtitleDrafts.flushAll();
  videoRenderSession.close();
  subtitleStyles.close();
  subtitleExport.reset();
  candidateTool.close();
  cleanupConfirmation.cancel();
  repairTool.close();
  editorProcessing.reset();
  if (workspaceView?.classList.contains("editor-task-tools")) closeEditorTools();
  playbackLoop = null; loopStartMs = null;
  document.querySelectorAll<HTMLElement>(".loop-state").forEach(e=>e.textContent="循环已关闭");

  const detail = activeDetail;
  const area = currentArea;
  editorWaveform.reset();
  editorTimeline.clear();
  timelineGestures.reset();
  currentCueEditor.clear();
  editorCueList.clear(); editorFilterCache = null;
  if (playbackPositionSaveTimer !== null) {
    window.clearTimeout(playbackPositionSaveTimer);
    playbackPositionSaveTimer = null;
  }
  await mediaPlayer.stop(async (media, positionMs) => {
    if (area === "listening" && detail) await savePlaybackPosition(detail.job.job_id, media, positionMs);
    else if (area === "karaoke" && detail) karaokeResumeMs = Math.round(positionMs);
  });
  activeSegmentId = null;
  updatePlaybackControls();
  hideSubtitleOverlay();
}

async function showTopLevelArea(area: Exclude<TopLevelArea, "workspace">, editorDetail?: JobDetail, editorJobId?: string): Promise<void> {
  closeLearningSelection();
  if (workspaceActionBusy) throw new Error("请等待当前字幕操作完成后再离开");
  const requestId = ++navigationRequestId;
  await stopActivePlayback();
  if (requestId !== navigationRequestId) return;
  activeDetail = null;
  subtitleList?.replaceChildren();
  listeningSubtitleList?.replaceChildren();
  currentArea = area;
  shell?.classList.toggle("workspace-open", area === "karaoke");
  taskTools.unmount();
  homeView?.classList.toggle("hidden", area !== "workbench");
  listeningView?.classList.toggle("hidden", area !== "listening");
  learningView?.classList.toggle("hidden", area !== "learning");
  karaokeView?.classList.toggle("hidden", area !== "karaoke");
  document.querySelector<HTMLButtonElement>("#show-workbench")?.classList.toggle("active", area === "workbench");
  document.querySelector<HTMLButtonElement>("#show-listening")?.classList.toggle("active", area === "listening");
  document.querySelector<HTMLButtonElement>("#show-learning")?.classList.toggle("active", area === "learning");
  document.querySelector<HTMLButtonElement>("#refresh")?.classList.toggle("hidden", area === "karaoke");
  updatePlaybackControls();
  renderSubtitleOverlayButton();
  renderListeningJobs(latestJobs);
  if (area === "workbench" || area === "listening") void refresh();
  if (area === "learning") {
    await Promise.all([refreshLearningItems(), loadDictionarySettings()]);
    if (learningDictionaryDialog?.open) renderLearningDictionary();
  }
  const editorTarget = editorJobId ?? karaokeSelectedJobId;
  if (area === "karaoke" && editorTarget) {
    await openKaraokeJob(editorTarget, editorDetail);
    if (currentArea !== "karaoke") return;
  }
  window.scrollTo({ top: 0, behavior: "auto" });
}

async function openListeningJob(
  jobId: string,
  resumeOverrideMs: number | null = null,
  autoplay = false,
): Promise<void> {
  if (!jobId || !listeningMediaMessage) return;
  if (workspaceActionBusy) throw new Error("请等待当前字幕操作完成后再离开");
  const requestId = ++navigationRequestId;
  await stopActivePlayback();
  if (requestId !== navigationRequestId || currentArea !== "listening") return;
  activeDetail = null;
  renderListeningJobs(latestJobs);
  renderSubtitleOverlayButton();
  listeningMediaMessage.textContent = "正在读取节目…";
  try {
    const [detail, savedResumeMs] = await Promise.all([
      invoke<JobDetail>("get_job_detail", { jobId }),
      invoke<number>("get_playback_position", { jobId }),
    ]);
    if (requestId !== navigationRequestId || currentArea !== "listening") return;
    await refreshLearningItems();
    if (requestId !== navigationRequestId) return;
    activeDetail = detail;
    activeSegmentId = null;
    if (listeningJobTitle) listeningJobTitle.textContent = displayName(detail.job);
    renderListeningJobs(latestJobs);
    renderListeningSubtitles(detail.segments);
    const resumeMs = resumeOverrideMs ?? savedResumeMs;
    mountMedia(detail.playback_path, detail.audio_fallback_path, listeningMediaHost, listeningMediaMessage, resumeMs);
    updateActiveSubtitle(resumeMs);
    renderSubtitleOverlayButton();
    if (autoplay && mediaPlayer.media) await mediaPlayer.media.play();
  } catch (error) {
    if (requestId !== navigationRequestId || currentArea !== "listening") return;
    listeningMediaMessage.textContent = `无法打开节目：${String(error)}`;
  }
}

function renderListeningSubtitles(segments: SubtitleSegment[]): void {
  if (!listeningSubtitleList) return;
  closeLearningSelection();
  listeningSubtitleList.innerHTML = segments.map((segment) => `
    <article class="listening-subtitle" data-segment-id="${escapeHtml(segment.id)}">
      <button type="button" class="listening-time" data-listen-time="${segment.start_ms}" aria-label="跳转到 ${escapeHtml(formatTime(segment.start_ms))}">${escapeHtml(formatTime(segment.start_ms))}</button>
      <div class="listening-text">
        <p class="listening-source" data-learning-source title="选择词语、短语或语法表达后收藏">${highlightSaved(segment.source_text,learningItems.filter(d=>d.item.source_language===activeDetail?.job.source_language).map(d=>d.item.source_text), learningItems.flatMap(d=>d.occurrences.filter(o=>o.job_id===activeDetail?.job.job_id && o.segment_id===segment.id && o.segment_source_snapshot===segment.source_text).map(o=>[o.selection_start_utf16,o.selection_end_utf16] as [number,number])), activeDetail?.job.source_language)}</p>
        <p class="listening-translation">${escapeHtml(segment.translated_text ?? "")}</p>
      </div>
      <button type="button" class="collect-learning-sentence secondary" data-collect-sentence="${escapeHtml(segment.id)}">收藏整句</button>
    </article>`).join("");
  listeningSubtitleList.querySelectorAll<HTMLButtonElement>("[data-listen-time]").forEach((button) => {
    button.addEventListener("click", () => seekTo(Number(button.dataset.listenTime ?? "0")));
  });
  listeningSubtitleList.querySelectorAll<HTMLButtonElement>("[data-collect-sentence]").forEach((button) => {
    button.addEventListener("click", () => void saveLearningSentence(button.dataset.collectSentence ?? ""));
  });
}

function selectionOffsetWithin(root: HTMLElement, node: Node, offset: number): number {
  const range = document.createRange();
  range.selectNodeContents(root);
  range.setEnd(node, offset);
  return range.toString().length;
}

function captureListeningSelection(): void {
  if (currentArea !== "listening" || !activeDetail || !listeningSubtitleList) return;
  const selection = window.getSelection();
  if (!selection || selection.rangeCount !== 1 || selection.isCollapsed) return;
  const range = selection.getRangeAt(0);
  const source = (range.startContainer instanceof Element
    ? range.startContainer
    : range.startContainer.parentElement)?.closest<HTMLElement>("[data-learning-source]");
  if (!source || !source.contains(range.startContainer) || !source.contains(range.endContainer)) {
    closeLearningSelection(false);
    return;
  }
  const segmentElement = source.closest<HTMLElement>("[data-segment-id]");
  const segmentId = segmentElement?.dataset.segmentId;
  if (!segmentId) return;
  let start = selectionOffsetWithin(source, range.startContainer, range.startOffset);
  let end = selectionOffsetWithin(source, range.endContainer, range.endOffset);
  if (end < start) [start, end] = [end, start];
  const rawText = source.textContent ?? "";
  const rawSelection = rawText.slice(start, end);
  const leadingWhitespace = rawSelection.length - rawSelection.trimStart().length;
  const trailingWhitespace = rawSelection.length - rawSelection.trimEnd().length;
  start += leadingWhitespace;
  end -= trailingWhitespace;
  const selectedText = rawText.slice(start, end);
  if (!selectedText) {
    closeLearningSelection(false);
    return;
  }
  pendingLearningSelection = {
    jobId: activeDetail.job.job_id,
    segmentId,
    selectedText,
    selectionStartUtf16: start,
    selectionEndUtf16: end,
  };
  if (listeningSelectionText) listeningSelectionText.textContent = selectedText;
  if (listeningSelectionMessage) {
    listeningSelectionMessage.textContent = "可先收藏，之后在学习区补充标准译义。";
    listeningSelectionMessage.classList.remove("warning");
  }
  listeningSelectionMenu?.classList.remove("hidden");
  const followState = subtitleFollowState(listeningSubtitleList);
  followState.userScrollingUntil = Number.MAX_SAFE_INTEGER;
}

function closeLearningSelection(clearBrowserSelection = true): void {
  pendingLearningSelection = null;
  listeningSelectionMenu?.classList.add("hidden");
  if (clearBrowserSelection) window.getSelection()?.removeAllRanges();
  if (listeningSubtitleList) {
    subtitleFollowState(listeningSubtitleList).userScrollingUntil = Date.now() + 3_000;
  }
}

function setLearningBusy(busy: boolean): void {
  learningActionBusy = busy;
  if (saveLearningSelectionButton) saveLearningSelectionButton.disabled = busy;
  if (saveLearningSentenceButton) saveLearningSentenceButton.disabled = busy;
  listeningSubtitleList?.querySelectorAll<HTMLButtonElement>(".collect-learning-sentence").forEach((button) => {
    button.disabled = busy;
  });
}

async function persistLearningSelection(
  selection: PendingLearningSelection,
  itemType: "selection" | "sentence",
): Promise<void> {
  if (learningActionBusy) return;
  setLearningBusy(true);
  if (listeningSelectionMessage) {
    listeningSelectionMessage.textContent = "正在保存到本地单词本…";
    listeningSelectionMessage.classList.remove("warning");
  }
  try {
    const saved = await invoke<LearningItemDetail>("save_learning_selection", {
      request: {
        jobId: selection.jobId,
        segmentId: selection.segmentId,
        itemType,
        selectionStartUtf16: selection.selectionStartUtf16,
        selectionEndUtf16: selection.selectionEndUtf16,
      },
    });
    await refreshLearningItems();
    closeLearningSelection();
    if (listeningMediaMessage) {
      listeningMediaMessage.textContent = `已收藏“${saved.item.source_text}”；可到学习区补充译义和回听例句。`;
    }
  } catch (error) {
    if (listeningSelectionMessage) {
      listeningSelectionMessage.textContent = `收藏失败：${String(error)}`;
      listeningSelectionMessage.classList.add("warning");
    }
  } finally {
    setLearningBusy(false);
  }
}

async function saveLearningSentence(segmentId: string): Promise<void> {
  if (!activeDetail || !segmentId) return;
  const segment = activeDetail.segments.find((candidate) => candidate.id === segmentId);
  if (!segment) return;
  await persistLearningSelection({
    jobId: activeDetail.job.job_id,
    segmentId,
    selectedText: segment.source_text,
    selectionStartUtf16: 0,
    selectionEndUtf16: segment.source_text.length,
  }, "sentence");
}

async function refreshLearningItems(): Promise<void> {
  if (!learningItemList) return;
  if (currentArea === "learning" && learningItems.length === 0) {
    learningItemList.innerHTML = '<div class="empty-state">正在读取单词本…</div>';
  }
  try {
    learningItems = await invoke<LearningItemDetail[]>("list_learning_items");
    renderLearningItems();
    if (learningDictionaryDialog?.open && activeLearningItemId) renderLearningDictionary();
    if (learningMessage) {
      learningMessage.textContent = learningItems.length
        ? "日语、英语和韩语收藏分别保存在对应单词本中。"
        : "从收听区选择原文，建立第一个学习条目。";
      learningMessage.classList.remove("warning");
    }
  } catch (error) {
    learningItemList.innerHTML = `<div class="empty-state error">无法读取单词本：${escapeHtml(String(error))}</div>`;
    if (learningMessage) {
      learningMessage.textContent = "单词本读取失败。";
      learningMessage.classList.add("warning");
    }
  }
}

function renderLearningItems(): void {
  if (!learningItemList) return;
  const allLanguageItems = learningItems.filter(detail => detail.item.source_language === activeLearningLanguage);
  const languageItems = allLanguageItems.filter(detail =>
    matchesQuery(learningQuery,[detail.item.source_text,detail.item.meaning_text,...detail.occurrences.map(o=>o.segment_source_snapshot)])
    && (!learningTaskFilter || detail.occurrences.some(o=>o.job_id===learningTaskFilter)));
  const taskFilter = document.querySelector<HTMLSelectElement>("#learning-task-filter");
  if (taskFilter) {
    const sources = new Map(learningItems.flatMap(d=>d.occurrences.filter(o=>o.job_id).map(o=>[o.job_id!,o.job_display_name_snapshot] as const)));
    taskFilter.innerHTML = '<option value="">全部来源任务</option>' + Array.from(sources).map(([id,title])=>`<option value="${escapeHtml(id)}">${escapeHtml(title)}</option>`).join("");
    taskFilter.value = learningTaskFilter;
  }
  const languageName = languageLabel(activeLearningLanguage);
  learningLanguageTabs?.querySelectorAll<HTMLButtonElement>("[data-learning-language]").forEach((button) => {
    const language = button.dataset.learningLanguage as LearningLanguage;
    const active = language === activeLearningLanguage;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
    const count = learningItems.filter((detail) => detail.item.source_language === language).length;
    const countHost = button.querySelector<HTMLElement>("[data-learning-language-count]");
    if (countHost) countHost.textContent = String(count);
  });
  if (learningLanguageSummary) {
    learningLanguageSummary.textContent = `${languageName} · ${languageItems.length} 个条目${learningQuery.trim() || learningTaskFilter ? `（共 ${allLanguageItems.length} 个）` : ""}`;
  }
  if (learningItems.length === 0) {
    learningItemList.innerHTML = '<div class="empty-state"><strong>单词本还是空的。</strong><span>到收听区选择词语、短语或语法表达；也可以直接收藏整句。</span></div>';
    return;
  }
  if (languageItems.length === 0) {
    learningItemList.innerHTML = allLanguageItems.length
      ? '<div class="empty-state"><strong>没有匹配的学习条目。</strong><span>请调整搜索文字或来源任务筛选。</span></div>'
      : `<div class="empty-state"><strong>${escapeHtml(languageName)}单词本还是空的。</strong><span>打开一项${escapeHtml(languageName)}任务，在收听区选择词语、短语或语法表达。</span></div>`;
    return;
  }
  learningItemList.innerHTML = languageItems.map((detail) => {
    const occurrence = detail.occurrences[0];
    const sourceLabel = detail.item.item_type === "sentence" ? "整句" : "词语／语法";
    const sourceMeta = occurrence
      ? `${escapeHtml(occurrence.job_display_name_snapshot)} · ${escapeHtml(formatTime(occurrence.start_ms))}`
      : "来源已不可用";
    const context = occurrence
      ? `<blockquote><strong>${escapeHtml(occurrence.segment_source_snapshot)}</strong>${occurrence.segment_translation_snapshot ? `<span>${escapeHtml(occurrence.segment_translation_snapshot)}</span>` : ""}</blockquote>`
      : "";
    return `<article class="learning-card" data-learning-item="${escapeHtml(detail.item.id)}">
      <div class="learning-card-heading">
        <div><span>${sourceLabel} · ${escapeHtml(languageLabel(detail.item.source_language))}</span><h3>${escapeHtml(detail.item.source_text)}</h3></div>
        <span>${detail.item.occurrence_count} 个例句</span>
      </div>
      <label>学习译义${detail.item.meaning_source_label ? `<span class="learning-meaning-source">当前来自 ${escapeHtml(detail.item.meaning_source_label)}</span>` : ""}<input class="learning-meaning" value="${escapeHtml(detail.item.meaning_text ?? "")}" placeholder="可手工填写，或从词典详情选择一条释义" /></label>
      <div class="learning-source-meta">${sourceMeta}</div>
      ${context}
      <details><summary>全部 ${detail.occurrences.length} 个来源例句</summary>${detail.occurrences.map(o=>`<blockquote><p>${escapeHtml(o.segment_source_snapshot)}</p><p>${escapeHtml(o.segment_translation_snapshot ?? "")}</p><span>${escapeHtml(o.job_display_name_snapshot)} · ${formatTime(o.start_ms)}</span>${o.job_id ? `<button type="button" class="secondary" data-play-occurrence="${escapeHtml(o.id)}">回到原句</button>` : '<span>原任务已删除；保留收藏快照</span>'}</blockquote>`).join("")}</details>
      <div class="learning-card-actions">
        <button type="button" data-save-learning-meaning="${escapeHtml(detail.item.id)}">保存译义</button>
        <button type="button" class="secondary" data-open-learning-dictionary="${escapeHtml(detail.item.id)}">查看词典</button>
        ${occurrence?.job_id ? `<button type="button" class="secondary" data-play-learning-source="${escapeHtml(detail.item.id)}">播放例句</button>` : ""}
        <button type="button" class="danger" data-delete-learning-item="${escapeHtml(detail.item.id)}">删除</button>
      </div>
    </article>`;
  }).join("");
  learningItemList.querySelectorAll<HTMLButtonElement>("[data-play-occurrence]").forEach(button => button.onclick = async () => {
    const occurrence = learningItems.flatMap(d=>d.occurrences).find(o=>o.id===button.dataset.playOccurrence);
    if (occurrence?.job_id) { await showTopLevelArea("listening"); await openListeningJob(occurrence.job_id,occurrence.start_ms,true); }
  });
  learningItemList.querySelectorAll<HTMLButtonElement>("[data-save-learning-meaning]").forEach((button) => {
    button.addEventListener("click", () => void saveLearningMeaning(button.dataset.saveLearningMeaning ?? ""));
  });
  learningItemList.querySelectorAll<HTMLButtonElement>("[data-play-learning-source]").forEach((button) => {
    button.addEventListener("click", () => void playLearningSource(button.dataset.playLearningSource ?? ""));
  });
  learningItemList.querySelectorAll<HTMLButtonElement>("[data-open-learning-dictionary]").forEach((button) => {
    button.addEventListener("click", () => openLearningDictionary(button.dataset.openLearningDictionary ?? ""));
  });
  learningItemList.querySelectorAll<HTMLButtonElement>("[data-delete-learning-item]").forEach((button) => {
    button.addEventListener("click", () => void removeLearningItem(button.dataset.deleteLearningItem ?? ""));
  });
}

function readLearningLanguagePreference(): LearningLanguage {
  try {
    const stored = window.localStorage.getItem("atogaki.learningLanguage");
    if (stored === "ja" || stored === "en" || stored === "ko") return stored;
  } catch {
    // A blocked WebView storage preference should not block the local learning library.
  }
  return "ja";
}

function selectLearningLanguage(language: LearningLanguage): void {
  activeLearningLanguage = language;
  try {
    window.localStorage.setItem("atogaki.learningLanguage", language);
  } catch {
    // Keep the selection for this run even when preference storage is unavailable.
  }
  renderLearningItems();
}

function learningProviderOptions(detail: LearningItemDetail): LearningProviderOption[] {
  const providers: LearningProviderOption[] = [{ id: "summary", name: "简明", kind: "summary" }];
  if (detail.item.item_type === "sentence") return providers;
  if (detail.item.source_language === "en") {
    providers.push(
      { id: "ecdict", name: "ECDICT 英中", kind: "offline" },
      { id: "merriam-webster", name: "Merriam-Webster", kind: "api" },
    );
  } else if (detail.item.source_language === "ja") {
    providers.push(
      { id: "jmdict", name: "JMdict", kind: "offline" },
      { id: "tomoshi", name: "Tomoshi", kind: "offline" },
    );
  }
  return providers;
}

function openLearningDictionary(itemId: string): void {
  const detail = learningItems.find((candidate) => candidate.item.id === itemId);
  if (!detail || !learningDictionaryDialog) return;
  activeLearningItemId = itemId;
  activeLearningProviderId = "summary";
  renderLearningDictionary();
  learningDictionaryDialog.showModal();
}

function closeLearningDictionary(): void {
  learningDictionaryDialog?.close();
  activeLearningItemId = null;
  activeLearningProviderId = "summary";
}

function renderLearningDictionary(): void {
  const detail = learningItems.find((candidate) => candidate.item.id === activeLearningItemId);
  if (!detail || !learningProviderTabs || !learningProviderPanel) {
    closeLearningDictionary();
    return;
  }
  const providers = learningProviderOptions(detail);
  if (!providers.some((provider) => provider.id === activeLearningProviderId)) {
    activeLearningProviderId = "summary";
  }
  const occurrence = detail.occurrences[0];
  if (learningDictionaryTitle) learningDictionaryTitle.textContent = detail.item.source_text;
  if (learningDictionarySubtitle) {
    learningDictionarySubtitle.textContent = `${languageLabel(detail.item.source_language)} · ${detail.item.item_type === "sentence" ? "整句收藏只显示简明译义" : "各词典来源独立显示，不自动合并"}`;
  }
  learningProviderTabs.innerHTML = providers.map((provider) => {
    const hasResult = provider.id === "summary"
      ? Boolean(detail.item.meaning_text)
      : detail.lookup_results.some((result) => result.provider_id === provider.id);
    const status = hasResult ? "已有内容" : learningProviderStatus(provider);
    return `<button type="button" class="secondary${provider.id === activeLearningProviderId ? " active" : ""}" data-learning-provider="${escapeHtml(provider.id)}" aria-pressed="${provider.id === activeLearningProviderId}" ${learningLookupBusy ? "disabled" : ""}>
      ${escapeHtml(provider.name)}<span>${escapeHtml(status)}</span>
    </button>`;
  }).join("");
  learningProviderTabs.querySelectorAll<HTMLButtonElement>("[data-learning-provider]").forEach((button) => {
    button.addEventListener("click", () => {
      activeLearningProviderId = button.dataset.learningProvider ?? "summary";
      renderLearningDictionary();
    });
  });

  if (activeLearningProviderId === "summary") {
    const meaning = detail.item.meaning_text
      ? `<p class="learning-summary-meaning">${escapeHtml(detail.item.meaning_text)}</p><p class="learning-source-meta">${detail.item.meaning_source_label ? `选自 ${escapeHtml(detail.item.meaning_source_label)}` : "手工维护"}</p>`
      : '<p class="learning-provider-empty-copy">还没有简明译义。可在其他词典标签中选择一条释义，也可关闭详情后手工填写。</p>';
    const context = occurrence
      ? `<blockquote><strong>${escapeHtml(occurrence.segment_source_snapshot)}</strong>${occurrence.segment_translation_snapshot ? `<span>${escapeHtml(occurrence.segment_translation_snapshot)}</span>` : ""}</blockquote>
        <p class="learning-source-meta">${escapeHtml(occurrence.job_display_name_snapshot)} · ${escapeHtml(formatTime(occurrence.start_ms))}</p>`
      : '<p class="learning-provider-empty-copy">原任务已删除；收藏时的来源快照仍保存在本机。</p>';
    learningProviderPanel.innerHTML = `<article class="learning-provider-result">
      <div class="learning-provider-result-heading"><div><span>我的学习译义</span><h3>${escapeHtml(detail.item.source_text)}</h3></div><span>本地</span></div>
      ${meaning}${context}
    </article>`;
    return;
  }

  const provider = providers.find((candidate) => candidate.id === activeLearningProviderId);
  const result = detail.lookup_results.find((candidate) => candidate.provider_id === activeLearningProviderId);
  if (!provider || !result) {
    const message = dictionaryProviderEmptyMessage(provider);
    learningProviderPanel.innerHTML = `<div class="learning-provider-empty">
      <span>${provider?.kind === "offline" ? "LOCAL DICTIONARY PACK" : "DICTIONARY API"}</span>
      <h3>${escapeHtml(provider?.name ?? activeLearningProviderId)}</h3>
      <p>${escapeHtml(message)}</p>
      ${provider ? `<button type="button" data-lookup-learning-provider="${escapeHtml(provider.id)}" ${learningLookupBusy ? "disabled" : ""}>${learningLookupBusy ? "正在查询…" : "查询这个来源"}</button>` : ""}
    </div>`;
    learningProviderPanel.querySelector<HTMLButtonElement>("[data-lookup-learning-provider]")?.addEventListener("click", () => {
      void lookupLearningDictionary(provider?.id ?? "");
    });
    return;
  }

  const senses = result.senses.map((sense, senseIndex) => `<section class="learning-dictionary-sense">
    ${sense.part_of_speech ? `<strong>${escapeHtml(sense.part_of_speech)}</strong>` : ""}
    <ol>${sense.definitions.map((definition, definitionIndex) => `<li><span>${escapeHtml(definition)}</span><button type="button" class="secondary" data-use-learning-definition="${senseIndex}:${definitionIndex}">设为简明</button></li>`).join("")}</ol>
    ${sense.examples.length ? `<div class="learning-dictionary-examples">${sense.examples.map((example) => `<p>${escapeHtml(example)}</p>`).join("")}</div>` : ""}
  </section>`).join("");
  learningProviderPanel.innerHTML = `<article class="learning-provider-result">
    <div class="learning-provider-result-heading">
      <div><span>${escapeHtml(result.provider_name)}</span><h3>${escapeHtml(result.headword)}</h3></div>
      <div class="learning-dictionary-reading">${result.reading ? escapeHtml(result.reading) : ""}${result.pronunciation ? `<span>${escapeHtml(result.pronunciation)}</span>` : ""}</div>
    </div>
    ${result.provider_id === "merriam-webster" ? '<img class="mw-logo" src="/merriam-webster-logo.png" width="50" height="50" alt="Merriam-Webster®" />' : ""}
    ${result.audio_url ? `<audio class="learning-dictionary-audio" controls preload="none" src="${escapeHtml(result.audio_url)}">当前环境无法播放词典发音。</audio>` : ""}
    ${senses}
    <footer><span>${escapeHtml(result.attribution_text)}</span>${result.license_label ? `<span>${escapeHtml(result.license_label)}</span>` : ""}${result.data_version ? `<span>数据 ${escapeHtml(result.data_version)}</span>` : ""}${result.source_url ? `<a href="${escapeHtml(result.source_url)}" target="_blank" rel="noreferrer">来源页面</a>` : ""}<button type="button" class="secondary" data-refresh-learning-provider="${escapeHtml(result.provider_id)}" ${learningLookupBusy ? "disabled" : ""}>${learningLookupBusy ? "正在刷新…" : "刷新来源"}</button></footer>
  </article>`;
  learningProviderPanel.querySelector<HTMLButtonElement>("[data-refresh-learning-provider]")?.addEventListener("click", () => {
    void lookupLearningDictionary(result.provider_id);
  });
  learningProviderPanel.querySelectorAll<HTMLButtonElement>("[data-use-learning-definition]").forEach((button) => {
    button.addEventListener("click", () => {
      const [senseIndex, definitionIndex] = (button.dataset.useLearningDefinition ?? "").split(":").map(Number);
      const definition = result.senses[senseIndex]?.definitions[definitionIndex];
      if (definition) void useLearningDictionaryDefinition(result.provider_id, definition);
    });
  });
}

function learningProviderStatus(provider: LearningProviderOption): string {
  if (provider.kind === "summary") return "可编辑";
  if (provider.kind === "offline") {
    const packageId = provider.id === "jmdict"
      ? "jmdict-en"
      : provider.id === "tomoshi"
        ? "tomoshi-open"
        : "ecdict-eng-zho";
    return dictionaryDownloads.some((item) => item.dictionaryId === packageId && item.status === "done") ? "可查询" : "未下载";
  }
  return dictionaryCredentials.some((item) => item.providerId === provider.id && item.configured) ? "可查询" : "未配置";
}

function dictionaryProviderEmptyMessage(provider: LearningProviderOption | undefined): string {
  if (!provider) return "词典来源不可用。";
  const status = learningProviderStatus(provider);
  if (status === "未下载") return `${provider.name} 的离线包尚未安装。请先到设置 → 学习词典下载；包会保存在正式应用数据目录。`;
  if (status === "未配置") return `${provider.name} API Key 尚未配置。请先到设置 → 学习词典保存并检查该来源的 Key。`;
  return provider.kind === "offline"
    ? "已安装本地数据。第一次查询可能需要解压或建立轻量索引，之后会直接复用。"
    : "查询只会把当前词语发送给这个词典 API；结果按来源独立保存。";
}

async function lookupLearningDictionary(providerId: string): Promise<void> {
  if (!activeLearningItemId || !providerId || learningLookupBusy) return;
  learningLookupBusy = true;
  renderLearningDictionary();
  let failure: string | null = null;
  try {
    const updated = await invoke<LearningItemDetail>("lookup_learning_dictionary", {
      itemId: activeLearningItemId,
      providerId,
    });
    learningItems = learningItems.map((item) => item.item.id === updated.item.id ? updated : item);
    renderLearningItems();
    renderLearningDictionary();
  } catch (error) {
    failure = String(error);
  } finally {
    learningLookupBusy = false;
    renderLearningDictionary();
    if (failure && learningProviderPanel) {
      learningProviderPanel.innerHTML = `<div class="learning-provider-empty"><span>LOOKUP FAILED</span><h3>查询失败</h3><p>${escapeHtml(failure)}</p><p>可以切换到其他词典来源，或稍后重试当前来源。</p><button type="button" data-retry-learning-provider>重试</button></div>`;
      learningProviderPanel.querySelector<HTMLButtonElement>("[data-retry-learning-provider]")?.addEventListener("click", () => void lookupLearningDictionary(providerId));
    }
  }
}

async function useLearningDictionaryDefinition(providerId: string, definition: string): Promise<void> {
  if (!activeLearningItemId || learningLookupBusy) return;
  learningLookupBusy = true;
  renderLearningDictionary();
  try {
    const updated = await invoke<LearningItemDetail>("use_learning_dictionary_definition", {
      request: { itemId: activeLearningItemId, providerId, definition },
    });
    learningItems = learningItems.map((item) => item.item.id === updated.item.id ? updated : item);
    renderLearningItems();
    activeLearningProviderId = "summary";
  } catch (error) {
    if (learningMessage) {
      learningMessage.textContent = `设置简明译义失败：${String(error)}`;
      learningMessage.classList.add("warning");
    }
  } finally {
    learningLookupBusy = false;
    renderLearningDictionary();
  }
}

async function saveLearningMeaning(itemId: string): Promise<void> {
  const card = learningItemList?.querySelector<HTMLElement>(`[data-learning-item="${CSS.escape(itemId)}"]`);
  const input = card?.querySelector<HTMLInputElement>(".learning-meaning");
  if (!input || learningActionBusy) return;
  learningActionBusy = true;
  if (learningMessage) learningMessage.textContent = "正在保存学习译义…";
  try {
    await invoke("update_learning_item_meaning", {
      request: { itemId, meaningText: input.value.trim() || null },
    });
    await refreshLearningItems();
    if (learningMessage) learningMessage.textContent = "学习译义已保存到本机。";
  } catch (error) {
    if (learningMessage) {
      learningMessage.textContent = `保存失败：${String(error)}`;
      learningMessage.classList.add("warning");
    }
  } finally {
    learningActionBusy = false;
  }
}

async function removeLearningItem(itemId: string): Promise<void> {
  const detail = learningItems.find((candidate) => candidate.item.id === itemId);
  if (!detail || learningActionBusy) return;
  const confirmed = await confirmAction({
    title: "删除学习条目？",
    message: `将从本机单词本删除“${detail.item.source_text}”及其 ${detail.item.occurrence_count} 个来源例句。原任务和字幕不会改变。`,
    confirmLabel: "删除条目",
    danger: true,
  });
  if (!confirmed) return;
  learningActionBusy = true;
  try {
    await invoke("delete_learning_item", { itemId });
    await refreshLearningItems();
  } catch (error) {
    if (learningMessage) {
      learningMessage.textContent = `删除失败：${String(error)}`;
      learningMessage.classList.add("warning");
    }
  } finally {
    learningActionBusy = false;
  }
}

async function playLearningSource(itemId: string): Promise<void> {
  const occurrence = learningItems.find((candidate) => candidate.item.id === itemId)?.occurrences[0];
  if (!occurrence?.job_id) return;
  await showTopLevelArea("listening");
  if (currentArea !== "listening") return;
  await openListeningJob(occurrence.job_id, occurrence.start_ms, true);
}

async function openKaraokeJob(jobId: string, loadedDetail?: JobDetail): Promise<void> {
  if (!jobId || !karaokeMediaMessage) return;
  const reopeningSelectedJob = karaokeSelectedJobId === jobId;
  if (workspaceActionBusy) throw new Error("请等待当前字幕操作完成后再离开");
  const requestId = ++navigationRequestId;
  await stopActivePlayback();
  if (requestId !== navigationRequestId || currentArea !== "karaoke") return;
  karaokeSelectedJobId = jobId;
  if (!reopeningSelectedJob) {
    karaokeResumeMs = 0;

  }
  activeDetail = null;
  editorWaveform.reset();
  currentCueEditor.clear();
  karaokeFollowPlayhead = true;
  activeRepairSignalId = null;
  if (retranscriptionStartInput) retranscriptionStartInput.value = "";
  if (retranscriptionEndInput) retranscriptionEndInput.value = "";
  syncTimelineFollowState();
  renderKaraokeTimeline();
  karaokeMediaMessage.textContent = "正在读取字幕编辑任务…";
  if (karaokeJobTitle) karaokeJobTitle.textContent = "正在打开任务…";
  if (karaokeWaveformStatus) karaokeWaveformStatus.textContent = "正在准备本地波形…";
  try {
    const detail = loadedDetail ?? await invoke<JobDetail>("get_job_detail", { jobId });
    if (requestId !== navigationRequestId || currentArea !== "karaoke") return;
    if (!detail.segments.length || !matchesTerminalStatus(detail.job.status)) { await openJob(jobId, "review"); return; }
    activeDetail = detail;
    editorSession.open(jobId, detail.segments);
    editorSearch.value = editorSession.query; editorFilter.value = editorSession.filter;
    asrQualitySignals = [];
    currentCueEditor.sync(karaokeDraftSegment(), true); renderEditorList(); syncEditorFollow();
    activeSegmentId = null;
    if (karaokeJobTitle) karaokeJobTitle.textContent = displayName(detail.job);
    relinkJobMediaButton?.classList.toggle("hidden", !detail.job.input_path || Boolean(detail.playback_path));
    if (karaokeOpenWorkspaceButton) karaokeOpenWorkspaceButton.disabled = false;
    mountMedia(detail.playback_path, detail.audio_fallback_path, karaokeMediaHost, karaokeMediaMessage, karaokeResumeMs);
    updateActiveSubtitle(karaokeResumeMs);
    updateStructureUndoButton();
    setSubtitleEditAction("拖动字幕块可整体移动，拖动左右边缘可单独修剪；按 10 ms 吸附，空白合法，同轨不能重叠。");
    await loadKaraokeWaveform(karaokeResumeMs, true);
    usePlayheadSubtitleForRetranscription(false);
    await loadAsrQualitySignals(jobId);
    renderEditorList();
    await refreshTranslationStatus();
  } catch (error) {
    if (requestId !== navigationRequestId || currentArea !== "karaoke") return;
    karaokeMediaMessage.textContent = `无法打开字幕编辑任务：${String(error)}`;
    if (karaokeWaveformStatus) karaokeWaveformStatus.textContent = "波形不可用。";
  }
}

async function saveAsrReviewDecision(
  signalId: string,
  decision: "confirmed_problem" | "false_positive" | "resolved",
): Promise<void> {
  if (!activeDetail) return;
  const jobId = activeDetail.job.job_id, navigation = navigationRequestId;
  try {
    if (!await editorProcessing.review(signalId, decision)) return;
    if (activeDetail?.job.job_id !== jobId || navigation !== navigationRequestId) return;
    await loadAsrQualitySignals(jobId);
  } catch (error) {
    if (activeDetail?.job.job_id !== jobId || navigation !== navigationRequestId) return;
    setRetranscriptionMessage(`保存审查结论失败：${String(error)}`, true);
  }
}

async function loadAsrQualitySignals(jobId: string): Promise<void> {
  const navigation = navigationRequestId;
  reviewSignals.prepare(jobId);
  try {
    const signals = await editorProcessing.signals(jobId);
    if (!signals || activeDetail?.job.job_id !== jobId || navigation !== navigationRequestId) return;
    asrQualitySignals = signals; renderEditorList(); reviewSignals.show(jobId, signals);
  } catch (error) {
    if (activeDetail?.job.job_id === jobId && navigation === navigationRequestId) reviewSignals.failed(error);
  }
}

function setRetranscriptionMessage(message: string, isError = false): void {
  if (!retranscriptionMessage) return;
  retranscriptionMessage.textContent = message;
  retranscriptionMessage.classList.toggle("warning", isError);
}

function usePlayheadSubtitleForRetranscription(reportGap = true): void {
  const position = (mediaPlayer.media?.currentTime ?? 0) * 1_000;
  const segment = activeDetail?.segments.find(cue => cue.start_ms <= position && position < cue.end_ms);
  if (!segment) {
    if (reportGap) setRetranscriptionMessage("播放头位于字幕空白处。请移动到字幕段，或手动选择要补识别的范围。", true);
    return;
  }
  if (!retranscriptionStartInput || !retranscriptionEndInput) return;
  if (authorizeGeminiUpload) authorizeGeminiUpload.checked = false;
  activeRepairSignalId = null;
  retranscriptionStartInput.value = formatPreciseTime(segment.start_ms);
  retranscriptionEndInput.value = formatPreciseTime(segment.end_ms);
  setRetranscriptionMessage(`已选第 ${segment.segment_index + 1} 段完整字幕。选择识别方式后预览候选；Gemini 仍需本次上传授权。`);
}

function useVisibleWindowForRetranscription(): void {
  if(authorizeGeminiUpload)authorizeGeminiUpload.checked=false;
  activeRepairSignalId=null;
  if (!editorWaveform.window || !retranscriptionStartInput || !retranscriptionEndInput) return;
  retranscriptionStartInput.value = formatPreciseTime(editorWaveform.window.start_ms);
  retranscriptionEndInput.value = formatPreciseTime(editorWaveform.window.end_ms);
  setRetranscriptionMessage("已填写当前可视范围；若边界穿过字幕，请显式对齐到完整字幕块。");
}

function retranscriptionRange(): { startMs: number; endMs: number } | null {
  const startMs = parseTimecode(retranscriptionStartInput?.value ?? "");
  const endMs = parseTimecode(retranscriptionEndInput?.value ?? "");
  if (startMs === null || endMs === null || endMs <= startMs) {
    setRetranscriptionMessage("请输入有效范围，且结束时间必须晚于开始时间。", true);
    return null;
  }
  return { startMs, endMs };
}

function expandRetranscriptionRangeToSegments(): void {
  if(authorizeGeminiUpload)authorizeGeminiUpload.checked=false;
  activeRepairSignalId=null;
  if (!activeDetail || !retranscriptionStartInput || !retranscriptionEndInput) return;
  const range = retranscriptionRange();
  if (!range) return;
  const intersecting = activeDetail.segments.filter(
    (segment) => segment.start_ms < range.endMs && segment.end_ms > range.startMs,
  );
  if (intersecting.length === 0) {
    setRetranscriptionMessage("这个范围没有相交字幕块，边界保持不变。可用于补识别字幕空白。", false);
    return;
  }
  const startMs = Math.min(range.startMs, ...intersecting.map((segment) => segment.start_ms));
  const endMs = Math.max(range.endMs, ...intersecting.map((segment) => segment.end_ms));
  retranscriptionStartInput.value = formatPreciseTime(startMs);
  retranscriptionEndInput.value = formatPreciseTime(endMs);
  setRetranscriptionMessage(`已明确扩展到 ${intersecting.length} 个完整字幕块；不会再切穿它们。`);
}

function renderRetranscriptionItems(
  host: HTMLDivElement | null,
  items: Array<SubtitleSegment | RetranscriptionCandidate>,
): void {
  if (!host) return;
  host.replaceChildren();
  if (items.length === 0) {
    const empty = document.createElement("p");
    empty.className = "muted";
    empty.textContent = "这个范围没有字幕。";
    host.append(empty);
    return;
  }
  for (const item of items) {
    const row = document.createElement("article");
    const time = document.createElement("span");
    const text = document.createElement("p");
    time.textContent = `${formatPreciseTime(item.start_ms)} → ${formatPreciseTime(item.end_ms)}`;
    text.textContent = item.source_text;
    row.append(time, text);
    host.append(row);
  }
}

function showRetranscriptionPreview(preview: RetranscriptionPreview): void {
  if (retranscriptionPreviewSummary) {
    retranscriptionPreviewSummary.textContent = `${formatPreciseTime(preview.start_ms)} → ${formatPreciseTime(preview.end_ms)} · 当前 ${preview.original_segments.length} 段 · 候选 ${preview.candidate_segments.length} 段 · ${preview.provider_name ?? "识别候选"} · ${preview.model_identity ?? ""} · ${preview.context_mode === "isolated" ? "前文文字已隔离" : "独立识别候选"}`;
  }
  renderRetranscriptionItems(retranscriptionOriginalList, preview.original_segments);
  renderRetranscriptionItems(retranscriptionCandidateList, preview.candidate_segments);
  if (retranscriptionConfirmMessage) retranscriptionConfirmMessage.textContent = "";
  if(retranscriptionDialog) {
    retranscriptionDialog.querySelector(".candidate-diff")?.remove();
    retranscriptionDialog.querySelector(".refresh-comparison")?.remove();
    const refresh=document.createElement("button");refresh.type="button";refresh.className="secondary refresh-comparison";refresh.textContent="以当前字幕重新对照（不重跑识别）";
    refresh.onclick=()=>void editorProcessing.refresh();
    retranscriptionDialog.querySelector(".retranscription-comparison")?.before(refresh);
    const diff=document.createElement("details");diff.className="candidate-diff";
    diff.innerHTML=`<summary>文本差异与运行参数</summary><p>${candidateTextDiff(preview.original_segments.map(s=>s.source_text).join("\n"),preview.candidate_segments.map(s=>s.source_text).join("\n"))}</p><p>时间证据：${escapeHtml(preview.timing_granularities?.join(" / ")||"未知")} · 工作区版本：${preview.workspace_revision ?? "历史候选，采用前需重新对照"}</p><pre>${escapeHtml(JSON.stringify(preview.configuration??{},null,2))}</pre>`;
    retranscriptionDialog.querySelector(".retranscription-comparison")?.after(diff);
  }
  candidateTool.open();
}

async function previewSelectedRange(): Promise<void> {
  if (!activeDetail || editorProcessing.busy) return;
  if (currentCueEditor.dirty) {
    setRetranscriptionMessage("重新识别前请先保存或放弃当前文字修改。", true);
    return;
  }
  const range = retranscriptionRange();
  if (!range) return;
  editorSession.processingRange = range;
  const providerId = retranscriptionProvider?.value ?? "whisper.cpp";
  const authorizeCloudAudioUpload = authorizeGeminiUpload?.checked ?? false;
  if (providerId === "gemini-transcribe" && !authorizeCloudAudioUpload) {
    setRetranscriptionMessage("使用 Gemini 前，请确认只上传当前所选范围并勾选音频上传授权。", true);
    return;
  }
  if (authorizeGeminiUpload) authorizeGeminiUpload.checked = false;
  await editorProcessing.previewRange({ ...range, providerId, authorizeCloudAudioUpload, qualitySignalId: activeRepairSignalId });
}

async function confirmSelectedRangeReplacement(): Promise<void> {
  if (workspaceActionBusy || !activeDetail || !editorProcessing.preview || editorProcessing.busy) return;
  if (hasUnsavedSubtitleEdits()) {
    if (retranscriptionConfirmMessage) retranscriptionConfirmMessage.textContent = "采用候选前请先保存或放弃当前文字草稿。";
    return;
  }
  await editorProcessing.adopt();
}

function karaokeWindowDurationMs(): number {
  const value = Number(karaokeZoomInput?.value || "66");
  const minimum = 2_000;
  const maximum = 120_000;
  return Math.round(minimum * Math.pow(maximum / minimum, value / 100) / 10) * 10;
}

function karaokeZoomValueForDuration(durationMs: number): number {
  const minimum = 2_000;
  const maximum = 120_000;
  return Math.max(0, Math.min(100, 100 * Math.log(durationMs / minimum) / Math.log(maximum / minimum)));
}

function syncKaraokeZoomLabel(): void {
  if (!karaokeZoomLabel) return;
  const seconds = karaokeWindowDurationMs() / 1_000;
  karaokeZoomLabel.value = seconds < 10 ? `${seconds.toFixed(1)} 秒` : `${Math.round(seconds)} 秒`;
}

async function loadKaraokeWaveform(anchorMs: number, alignStart = false): Promise<void> {
  await editorWaveform.load(anchorMs, alignStart);
}

function renderKaraokeTimeline(): void {
  const waveform = editorWaveform.window;
  const drag = timelineGestures.timing;
  const all = drag?.draft ?? activeDetail?.segments ?? [];
  const original = drag?.before ?? all;
  const index = subtitleTimeline(original);
  const positions = new Set(waveform ? index.visible(waveform.start_ms, waveform.end_ms).map(cue => index.position(cue.id)) : []);
  if (drag) positions.add(drag.segmentIndex);
  editorTimeline.render({
    waveform, cues: [...positions].sort((a,b) => a-b).map(i => all[i]),
    selectedCueId: editorSession.selectedCueId, playbackCueId: editorSession.playbackCueId,
    draggedCueId: drag ? all[drag.segmentIndex]?.id ?? null : null,
    gain: karaokeWaveformGain, currentMs: Math.round((mediaPlayer.media?.currentTime ?? 0) * 1_000),
    emptyMessage: activeDetail ? "正在准备波形…" : "选择一个字幕任务",
  });
}

function karaokeTimingAnomalySummary(segments: SubtitleSegment[]): string {
  let gaps = 0;
  let overlaps = 0;
  for (let index = 0; index + 1 < segments.length; index += 1) {
    const difference = segments[index + 1].start_ms - segments[index].end_ms;
    if (difference > 0) gaps += 1;
    else if (difference < 0) overlaps += 1;
  }
  return [gaps ? `合法空白 ${gaps} 处` : "", overlaps ? `待修复重叠 ${overlaps} 处` : ""]
    .filter(Boolean)
    .join(" · ");
}

function syncKaraokeWaveformStatus(): void {
  if (!karaokeWaveformStatus || !editorWaveform.window || !activeDetail) return;
  const waveform = editorWaveform.window;
  const anomaly = karaokeTimingAnomalySummary(activeDetail.segments);
  karaokeWaveformStatus.textContent = `${formatPreciseTime(waveform.start_ms)} — ${formatPreciseTime(waveform.end_ms)} · 每个可视峰值约 ${Math.max(1, Math.round(waveform.point_duration_ms))} ms${anomaly ? ` · ${anomaly}` : " · 单轨无重叠"} · 拖动顶部红色把手定位；拖动波形背景平移`;
}

async function commitKaraokeTimingDrag(drag: TimingGesture<SubtitleSegment>): Promise<void> {
  if (!drag.moved || activeDetail?.job.job_id !== drag.jobId) {
    renderKaraokeTimeline();
    return;
  }
  const navigation = navigationRequestId;
  if (!await flushEditorStructureDrafts() || navigation !== navigationRequestId || activeDetail?.job.job_id !== drag.jobId) { renderKaraokeTimeline(); return; }
  const jobId = drag.jobId;
  const before = cloneSubtitleSegments(activeDetail.segments);
  let after: SubtitleSegment[];
  try { after = mergeDraggedCueTiming(before, drag.before[drag.segmentIndex], drag.draft[drag.segmentIndex]); }
  catch (error) { setSubtitleEditAction(String(error), true); renderKaraokeTimeline(); return; }
  setSubtitleEditAction("正在原子保存字幕块时间…");
  setEditorMutationBusy(true);
  try {
    const saved = await editorBackend.saveTiming(jobId, before, after);
    if (!activeDetail || activeDetail.job.job_id !== jobId) return;
    const segment = saved[drag.segmentIndex];
    const action = drag.mode === "move" ? "移动" : drag.mode === "start" ? "入点调整" : "出点调整";
    rememberStructureEdit(before, saved, `第 ${segment.segment_index + 1} 段${action}`);
    activeDetail.segments = saved;
    editorSession.reconcile(saved, segment.id);
    currentCueEditor.sync(karaokeDraftSegment(), true); renderEditorList();
    updateStructureUndoButton();
    syncKaraokeWaveformStatus();
    updateKaraokePosition((mediaPlayer.media?.currentTime ?? 0) * 1_000);
    renderKaraokeTimeline();
    setSubtitleEditAction(`第 ${segment.segment_index + 1} 段已保存为 ${formatPreciseTime(segment.start_ms)} → ${formatPreciseTime(segment.end_ms)}；相邻空白保持不变。`);
  } catch (error) {
    renderKaraokeTimeline();
    setSubtitleEditAction(`边界保存失败：${String(error)}`, true);
  } finally { setEditorMutationBusy(false); }
}

function karaokeDraftSegment(): SubtitleSegment | undefined {
  const segmentId = editorSession.selectedCueId;
  return activeDetail ? subtitleTimeline().get(segmentId) : undefined;
}

async function undoKaraokeTextSave(): Promise<void> {
  const segment = karaokeDraftSegment();
  if (!segment || !activeDetail || currentCueEditor.dirty) return;
  const key = subtitleUndoKey(segment);
  const history = subtitleUndoHistory.get(key);
  const entry = history?.at(-1);
  if (!entry || entry.afterFingerprint !== subtitleValueFingerprint(segment)) {
    setSubtitleEditAction("无法撤销：字幕在上次保存后已经改变。", true);
    if (karaokeUndoTextButton) karaokeUndoTextButton.disabled = true;
    return;
  }
  if (karaokeUndoTextButton) karaokeUndoTextButton.disabled = true;
  try {
    const restored = await restoreSegment(entry.before);
    if (activeDetail?.job.job_id !== restored.job_id) return;
    history?.pop();
    if (history?.length === 0) subtitleUndoHistory.delete(key);
    replaceActiveSegment(restored);
    if (editorSession.selectedCueId === restored.id) currentCueEditor.sync(restored, true);
    updateStructureUndoButton();
    updateKaraokePosition((mediaPlayer.media?.currentTime ?? 0) * 1_000);
    setSubtitleEditAction("已撤销当前字幕上一次文字保存；历史仅保留在本次 App 会话。");
  } catch (error) {
    if (karaokeUndoTextButton) karaokeUndoTextButton.disabled = false;
    setSubtitleEditAction(`文字撤销失败：${String(error)}`, true);
  }
}

function updateKaraokePosition(milliseconds: number): void {
  if (currentArea !== "karaoke") return;
  if (karaokeCurrentTime) {
    const value = formatPreciseTime(milliseconds);
    if (karaokeCurrentTime.textContent !== value) karaokeCurrentTime.textContent = value;
  }
  const playbackSegment = subtitleAt(milliseconds);
  const previousId = editorSession.selectedCueId, previousPlayback = editorSession.playbackCueId;
  // A recovered/failed draft must never be displaced by a clock update.
  if (editorSession.followPlayback && currentCueEditor.dirty) lockEditorCue();
  editorSession.updatePlayback(playbackSegment?.id ?? null);
  const segment = karaokeDraftSegment();
  editorCueList.mark(editorSession.selectedCueId, editorSession.playbackCueId, editorSession.listAutoScroll);
  if (previousId !== editorSession.selectedCueId || previousPlayback !== editorSession.playbackCueId) syncEditorSelectionMessage();
  if (karaokeSegmentTime) {
    const value = segment
      ? `${formatPreciseTime(segment.start_ms)} → ${formatPreciseTime(segment.end_ms)} · #${segment.segment_index + 1}`
      : "当前没有字幕";
    if (karaokeSegmentTime.textContent !== value) karaokeSegmentTime.textContent = value;
  }
  if (!currentCueEditor.dirty && !currentCueEditor.matches(segment)) currentCueEditor.sync(segment);
  const timeFingerprint = `${segment?.id}:${segment?.start_ms}:${segment?.end_ms}`;
  if (editorTimeFingerprint !== timeFingerprint && document.activeElement !== editorStart && document.activeElement !== editorEnd) {
    editorTimeFingerprint = timeFingerprint;
    editorStart.value = segment ? formatPreciseTime(segment.start_ms) : "";
    editorEnd.value = segment ? formatPreciseTime(segment.end_ms) : "";
  }
  editorStart.disabled = !segment; editorEnd.disabled = !segment;
  const segmentIndex = segment ? subtitleTimeline().position(segment.id) : -1;
  const playhead = Math.round(milliseconds);
  if (karaokeCutSegmentButton) {
    karaokeCutSegmentButton.disabled = !segment || segment.id !== playbackSegment?.id
      || !playbackSegment
      || !activeDetail
      || !matchesTerminalStatus(activeDetail.job.status)
      || playhead <= playbackSegment.start_ms
      || playhead >= playbackSegment.end_ms;
  }
  if (karaokeJoinSegmentButton) {
    const next = segmentIndex >= 0 ? activeDetail?.segments[segmentIndex + 1] : undefined;
    karaokeJoinSegmentButton.disabled = !segment || !next || segment.end_ms !== next.start_ms;
    karaokeJoinSegmentButton.title = segment && next && segment.end_ms !== next.start_ms
      ? "两块之间存在空白；连接会填满静音区，因此只允许连接边界真正相接的字幕"
      : "连接相接的下一字幕块";
  }
  const waveform = editorWaveform.window;
  if (karaokeFollowPlayhead && waveform && !editorWaveform.loading && !editorRoot.classList.contains("editor-waveform-hidden")) {
    const margin = (waveform.end_ms - waveform.start_ms) * 0.12;
    if (milliseconds < waveform.start_ms + margin || milliseconds > waveform.end_ms - margin) {
      void loadKaraokeWaveform(milliseconds);
      return;
    }
  }
  renderKaraokeTimeline();
}

function syncTimelineFollowState(): void {
  const state = document.querySelector("#editor-wave-follow-state");
  if (state) state.textContent = karaokeFollowPlayhead ? "视窗跟随中" : "视窗已暂停 · 点击跟随播放恢复";
}

function seekKaraokeBy(milliseconds: number): void {
  if (!mediaPlayer.media) return;
  const durationMs = Number.isFinite(mediaPlayer.media.duration) ? mediaPlayer.media.duration * 1_000 : Number.MAX_SAFE_INTEGER;
  seekTo(Math.max(0, Math.min(durationMs, mediaPlayer.positionMs + milliseconds)), false);
  updateKaraokePosition(mediaPlayer.positionMs);
}

async function changeKaraokeBlock(removeId: string | null, insertion: SubtitleSegment | null): Promise<void> {
  if (!activeDetail || workspaceActionBusy || !await flushEditorStructureDrafts()) return;
  const jobId = activeDetail.job.job_id;
  const before = cloneSubtitleSegments(activeDetail.segments);
  setEditorMutationBusy(true);
  try {
    const after = await invoke<SubtitleSegment[]>("edit_subtitle_block", {request:{jobId, beforeSegments:before, removeId, insertion}});
    rememberStructureEdit(before, after, removeId ? "删除字幕块" : "插入字幕块", jobId);
    if (activeDetail?.job.job_id === jobId) {
      applySubtitleStructure(after, insertion?.id ?? after[0]?.id);
      if (insertion) { seekTo(insertion.start_ms, false); updateActiveSubtitle(insertion.start_ms); }
      setSubtitleEditAction("已保存，可用“撤销上次打轴”撤回；仅实际依赖被删除块的机器译文需要核对。");
    }
  } catch(error) { setSubtitleEditAction(`字幕块操作失败：${String(error)}`, true); }
  finally { setEditorMutationBusy(false); }
}
async function openInsertSubtitleBlock(): Promise<void> {
  if (!activeDetail || workspaceActionBusy || !await flushEditorStructureDrafts()) return;
  if (currentCueEditor.dirty) { setSubtitleEditAction("插入前请先保存或放弃文字修改。", true); return; }
  if (!mediaPlayer.media || !Number.isFinite(mediaPlayer.media.duration) || mediaPlayer.media.duration <= 0) { setSubtitleEditAction("媒体时长尚未就绪，请等待加载或重新定位媒体。", true); return; }
  mediaPlayer.media.pause();
  const jobId = activeDetail.job.job_id;
  const position = Math.round((mediaPlayer.media?.currentTime ?? 0) * 1000 / 10) * 10;
  const current = activeDetail.segments.find(s=>s.start_ms <= position && s.end_ms > position);
  const start = current?.end_ms ?? position;
  const next = activeDetail.segments.find(s=>s.start_ms >= start);
  const duration = Number.isFinite(mediaPlayer.media?.duration) ? Math.round(mediaPlayer.media!.duration*1000) : start+2000;
  const end = Math.min(start+2000, next?.start_ms ?? duration, duration);
  const dialog = document.createElement("dialog"); dialog.className="rename-job-dialog insert-block-dialog";
  dialog.innerHTML='<form><h2>插入字幕块</h2><p>在空白区间新增字幕，不移动已有块。</p><label>开始<input name="start" required></label><label>结束<input name="end" required></label><label>原文<textarea name="source" rows="3" required></textarea></label><label>译文（可选）<textarea name="translation" rows="2"></textarea></label><p role="status"></p><div class="confirmation-actions"><button type="button" class="secondary" data-close>取消</button><button type="submit">插入</button></div></form>';
  const field = (name:string)=>dialog.querySelector<HTMLInputElement|HTMLTextAreaElement>(`[name="${name}"]`)!;
  field("start").value=formatPreciseTime(start); field("end").value=formatPreciseTime(Math.max(start,end));
  if (end <= start) dialog.querySelector('[role="status"]')!.textContent="当前位置后方没有空白，请手动选择其他空白区间，或先修剪已有字幕边界。";
  dialog.querySelector("[data-close]")!.addEventListener("click",()=>dialog.close());
  dialog.addEventListener("close",()=>dialog.remove());
  dialog.querySelector("form")!.onsubmit=async event=>{
    event.preventDefault();
    const startMs=parseTimecode(field("start").value), endMs=parseTimecode(field("end").value), source=field("source").value.trim();
    if (activeDetail?.job.job_id !== jobId) { dialog.close(); return; }
    if (startMs===null || endMs===null || startMs<0 || endMs<=startMs || endMs>duration || !source || activeDetail.segments.some(s=>s.start_ms<endMs && s.end_ms>startMs)) {
      dialog.querySelector('[role="status"]')!.textContent="请输入有效原文和媒体范围内的空白区间，不能与已有字幕重叠。"; return;
    }
    const insertion:SubtitleSegment={id:crypto.randomUUID(),job_id:jobId,segment_index:0,start_ms:startMs,end_ms:endMs,source_text:source,translated_text:field("translation").value.trim()||null,source_edited:true,translation_edited:!!field("translation").value.trim(),translation_stale:false,timing_edited:true};
    dialog.close(); await changeKaraokeBlock(null,insertion);
  };
  document.body.append(dialog); dialog.showModal();
}
async function deleteCurrentSubtitleBlock(): Promise<void> {
  if (!activeDetail || workspaceActionBusy) return;
  const selectedId = editorSession.selectedCueId;
  if (!await flushEditorStructureDrafts() || selectedId !== editorSession.selectedCueId) return;
  if (currentCueEditor.dirty) { setSubtitleEditAction("删除前请先保存或放弃文字修改。", true); return; }
  const jobId=activeDetail.job.job_id;
  const id=editorSession.selectedCueId;
  if (!id || !activeDetail.segments.some(s=>s.id===id)) { setSubtitleEditAction("请先选择要删除的字幕块。",true); return; }
  mediaPlayer.media?.pause();
  if (!await confirmAction({title:"删除当前字幕块？",message:"删除原文、译文及此块时间范围；不会移动其他字幕。学习收藏保留来源快照。当前会话可用“撤销上次打轴”恢复。",confirmLabel:"删除字幕块",danger:true})) return;
  if (activeDetail?.job.job_id===jobId) await changeKaraokeBlock(id,null);
}

async function cutKaraokeSegmentAtPlayhead(): Promise<void> {
  if (!activeDetail || workspaceActionBusy) return;
  const selectedId = editorSession.selectedCueId;
  const boundary = Math.round((mediaPlayer.media?.currentTime ?? 0) * 1_000 / 10) * 10;
  lockEditorCue();
  if (!await flushEditorStructureDrafts() || editorSession.selectedCueId !== selectedId) return;
  if (currentCueEditor.dirty) {
    setSubtitleEditAction("Cut 前请先保存或放弃当前文字修改。", true);
    return;
  }
  const segment = karaokeDraftSegment();
  if (!segment || boundary <= segment.start_ms || boundary >= segment.end_ms) {
    setSubtitleEditAction("播放头必须位于当前编辑段内部才能切开。", true);
    return;
  }
  const jobId = activeDetail.job.job_id;
  const before = cloneSubtitleSegments(activeDetail.segments);
  setEditorMutationBusy(true);
  setSubtitleEditAction(`正在播放头 ${formatPreciseTime(boundary)} 切开第 ${segment.segment_index + 1} 段…`);
  try {
    const after = await invoke<SubtitleSegment[]>("split_subtitle", {
      request: {
        jobId,
        segmentId: segment.id,
        boundaryMs: boundary,
        leftSourceText: segment.source_text,
        rightSourceText: segment.source_text,
        leftTranslatedText: segment.translated_text,
        rightTranslatedText: segment.translated_text,
      },
    });
    if (activeDetail?.job.job_id !== jobId) return;
    rememberStructureEdit(before, after, `第 ${segment.segment_index + 1} 段 Cut`);
    applySubtitleStructure(after, segment.id);
    setSubtitleEditAction("已在播放头切开；为保证连续播放观感，文字暂时复制到两块，请在当前编辑框继续整理。 ");
  } catch (error) {
    setSubtitleEditAction(`切开失败：${String(error)}`, true);
  } finally {
    setEditorMutationBusy(false);
  }
}

async function joinKaraokeSegmentWithNext(): Promise<void> {
  if (!activeDetail || !editorSession.selectedCueId || workspaceActionBusy) return;
  const selectedId = editorSession.selectedCueId;
  if (!await flushEditorStructureDrafts() || selectedId !== editorSession.selectedCueId) return;
  if (currentCueEditor.dirty) {
    setSubtitleEditAction("Join 前请先保存或放弃当前文字修改。", true);
    return;
  }
  const index = activeDetail.segments.findIndex((item) => item.id === editorSession.selectedCueId);
  const left = activeDetail.segments[index];
  const right = activeDetail.segments[index + 1];
  if (!left || !right) return;
  if (left.end_ms !== right.start_ms) {
    setSubtitleEditAction("两块之间存在空白；请保留为两个字幕块，或先独立修剪到同一边界。", true);
    return;
  }
  const jobId = activeDetail.job.job_id;
  const before = cloneSubtitleSegments(activeDetail.segments);
  setEditorMutationBusy(true);
  setSubtitleEditAction(`正在连接第 ${left.segment_index + 1}、${right.segment_index + 1} 段…`);
  try {
    const after = await invoke<SubtitleSegment[]>("merge_subtitles", {
      request: {
        jobId,
        leftSegmentId: left.id,
        rightSegmentId: right.id,
        sourceText: joinedSubtitleText(left.source_text, right.source_text, activeSourceLanguage()),
        translatedText: left.translated_text && right.translated_text
          ? joinedSubtitleText(left.translated_text, right.translated_text, activeTargetLanguage())
          : null,
      },
    });
    if (activeDetail?.job.job_id !== jobId) return;
    rememberStructureEdit(before, after, `第 ${left.segment_index + 1}、${right.segment_index + 1} 段 Join`);
    applySubtitleStructure(after, left.id);
    setSubtitleEditAction("已连接相接的字幕块；文字已顺序拼接。 ");
  } catch (error) {
    setSubtitleEditAction(`连接失败：${String(error)}`, true);
  } finally {
    setEditorMutationBusy(false);
  }
}


function showWorkspaceSection(section: WorkspaceSection): void {
  if (section === "review" && currentArea === "karaoke") { closeEditorTools(); return; }
  currentWorkspaceSection = section;
  for (const tab of workspaceSectionTabs) {
    const selected = tab.dataset.workspaceSection === section;
    tab.classList.toggle("active", selected);
    tab.setAttribute("aria-selected", String(selected));
    tab.tabIndex = selected ? 0 : -1;
  }
  for (const panel of workspaceSectionPanels) {
    panel.classList.toggle("hidden", panel.dataset.workspacePanel !== section);
  }
}

async function openJob(jobId: string, initialSection?: WorkspaceSection): Promise<void> {
  if (!jobId || !workspaceMessage) return;
  const known = latestJobs.find(job => job.job_id === jobId);
  if ((initialSection === undefined || initialSection === "review") && known && known.segment_count > 0 && matchesTerminalStatus(known.status)) { await enterSubtitleEditor(jobId); return; }
  if (workspaceActionBusy) throw new Error("请等待当前字幕操作完成后再离开");
  const requestId = ++navigationRequestId;
  await stopActivePlayback();
  if (requestId !== navigationRequestId) return;
  showWorkspace(true);
  showWorkspaceSection(initialSection ?? "review");
  workspaceView.querySelector<HTMLButtonElement>("#back-to-jobs")!.textContent = "← 返回任务";
  window.scrollTo({ top: 0, behavior: "auto" });
  workspaceMessage.textContent = "正在读取 字幕工作区…";
  setWorkspaceAction("");
  if (subtitleList) subtitleList.innerHTML = `<div class="empty-state">正在读取字幕…</div>`;
  try {
    const detail = await invoke<JobDetail>("get_job_detail", { jobId });
    if (requestId !== navigationRequestId || currentArea !== "workspace") return;
    activeDetail = detail;
    if ((initialSection === undefined || initialSection === "review") && detail.segments.length > 0 && matchesTerminalStatus(detail.job.status)) { await enterSubtitleEditor(jobId, detail); return; }
    renderWorkspace(detail);
    await refreshTranslationStatus();
  } catch (error) {
    if (requestId !== navigationRequestId || currentArea !== "workspace") return;
    activeDetail = null;
    workspaceMessage.textContent = `无法打开任务：${String(error)}`;
  }
}

async function enterSubtitleEditor(jobId: string, detail?: JobDetail): Promise<void> {
  if (!jobId) return;
  await showTopLevelArea("karaoke", detail, jobId);
}

function renderWorkspace(detail: JobDetail): void {
  taskProviderView?.render();
  subtitleExport.clearResult();
  if (workspaceTitle) workspaceTitle.textContent = `${displayName(detail.job)} · ${currentArea === "karaoke" ? "任务工具" : "任务进度"}`;
  if (workspaceMessage) {
    workspaceMessage.textContent = `${statusLabel(detail.job.status)} · ${detail.job.message} · ${jobTimingLabel(detail.job)}`;
  }
  if (segmentCount) segmentCount.textContent = `${detail.segments.length} 段`;
  if (openSubtitleEditorButton) {
    openSubtitleEditorButton.disabled = detail.segments.length === 0 || !matchesTerminalStatus(detail.job.status);
    openSubtitleEditorButton.title = openSubtitleEditorButton.disabled ? "转写结束并产生字幕后才能进入字幕编辑" : "打开当前任务的字幕编辑器";
  }
  renderGlossaryOptions();
  if (jobGlossaryStatus) {
    jobGlossaryStatus.textContent = detail.job.glossary_name
      ? `转写时使用：${detail.job.glossary_name}（已保存任务快照）`
      : "转写时未使用识别词表";
  }
  if (workspaceGlossary && detail.job.glossary_id && glossaries.some((item) => item.id === detail.job.glossary_id)) {
    workspaceGlossary.value = detail.job.glossary_id;
  }
  void renderWorkspaceGlossaryInspection();
  void renderJobGlossarySnapshot(detail.job.job_id);
  clearGlossaryPreview();
  const sourceMediaMissing = Boolean(detail.job.input_path && !detail.playback_path);
  relinkJobMediaButton?.classList.toggle("hidden", !sourceMediaMissing);
  if (relinkJobMediaButton) {
    relinkJobMediaButton.disabled = workspaceActionBusy;
    relinkJobMediaButton.title = sourceMediaMissing
      ? "选择移动后的原音频或视频；不会改动字幕和任务音频"
      : "";
  }
  if (currentArea !== "karaoke") mountMedia(detail.playback_path, detail.audio_fallback_path);
  if (sourceMediaMissing && mediaMessage) {
    mediaMessage.textContent = "原媒体已移动或删除；当前使用任务内保存的音频。可重新定位原媒体以恢复画面和视频烧录。";
  }
  if (currentArea !== "karaoke") renderSubtitleList(detail.segments);
  if (currentArea !== "karaoke") updateActiveSubtitle(0);
  renderSubtitleOverlayButton();
  updateTranslationControls();
  subtitleTranslation.renderActivity();

}

async function relinkActiveJobMedia(): Promise<void> {
  if (!activeDetail || workspaceActionBusy) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("请先保存或放弃尚未保存的字幕修改，再重新定位原媒体。", true);
    return;
  }
  setWorkspaceBusy(true);
  setWorkspaceAction("正在选择移动后的原媒体…");
  try {
    const updated = await invoke<LocalJob | null>("relink_job_media", {
      jobId: activeDetail.job.job_id,
    });
    if (!updated) {
      setWorkspaceAction("已取消重新定位。");
      return;
    }
    setWorkspaceBusy(false);
    if (currentArea === "karaoke") await enterSubtitleEditor(activeDetail.job.job_id);
    else await openJob(activeDetail.job.job_id, currentWorkspaceSection);
    setWorkspaceAction(`已重新定位原媒体：${updated.input_path ?? ""}`);
  } catch (error) {
    setWorkspaceAction(`重新定位原媒体失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
    updateTranslationControls();
  }
}

function isAudioPath(path: string): boolean {
  return /\.(mp3|m4a|aac|wav|flac|ogg)$/i.test(path);
}

function mountMedia(
  primaryPath: string | null,
  fallbackPath: string | null,
  host: HTMLDivElement | null = mediaHost,
  message: HTMLParagraphElement | null = mediaMessage,
  resumeMs = 0,
): void {
  if (!host || !message || !activeDetail) return;
  mediaPlayer.mount({ jobId: activeDetail.job.job_id, inputPath: activeDetail.job.input_path,
    primaryPath, fallbackPath, host, message, resumeMs, playbackRate: Number(activePlaybackRateSelect()?.value || "1") });
}

function cloneSubtitleSegments(segments: SubtitleSegment[]): SubtitleSegment[] {
  return segments.map((segment) => ({ ...segment }));
}

function subtitleStructureFingerprint(segments: SubtitleSegment[]): string {
  return JSON.stringify(segments);
}

function currentStructureUndoEntry(): SubtitleStructureUndoEntry | undefined {
  if (!activeDetail) return undefined;
  return subtitleStructureUndoHistory.get(activeDetail.job.job_id)?.at(-1);
}

function updateStructureUndoButton(): void {
  if (!undoSubtitleStructureButton) return;
  const entry = currentStructureUndoEntry();
  const available = Boolean(
    activeDetail && entry && subtitleStructureFingerprint(activeDetail.segments) === subtitleStructureFingerprint(entry.after),
  );
  undoSubtitleStructureButton.disabled = !available;
  undoSubtitleStructureButton.title = available
    ? `撤销${entry?.label ?? "上一次结构修改"}`
    : "当前会话还没有可撤销的打轴或结构修改";
}

function rememberStructureEdit(before: SubtitleSegment[], after: SubtitleSegment[], label: string, jobId = activeDetail?.job.job_id): void {
  if (!jobId) return;
  const history = subtitleStructureUndoHistory.get(jobId) ?? [];
  history.push({ before: cloneSubtitleSegments(before), after: cloneSubtitleSegments(after), label });
  if (history.length > 10) history.shift();
  subtitleStructureUndoHistory.set(jobId, history);
  for (const key of subtitleUndoHistory.keys()) {
    if (key.startsWith(`${jobId}:`)) subtitleUndoHistory.delete(key);
  }
}

function applySubtitleStructure(segments: SubtitleSegment[], expandedSegmentId?: string): void {
  if (!activeDetail) return;
  activeDetail.segments = segments;
  if (currentArea === "karaoke") lockEditorCue();
  editorSession.reconcile(segments, expandedSegmentId);
  if (currentArea === "karaoke") { currentCueEditor.sync(karaokeDraftSegment(), true); renderEditorList(); }
  if (segmentCount) segmentCount.textContent = `${segments.length} 段`;
  if (currentArea === "workspace") renderSubtitleListPreservingView(segments, expandedSegmentId);
  else {
    syncKaraokeWaveformStatus();
    renderKaraokeTimeline();
  }
  updateActiveSubtitle((mediaPlayer.media?.currentTime ?? 0) * 1_000);
  lastSubtitleOverlayKey = "";
  updateStructureUndoButton();
}

function joinedSubtitleText(left: string, right: string, language: LanguageCode): string {
  const leftText = left.trim();
  const rightText = right.trim();
  if (!leftText) return rightText;
  if (!rightText) return leftText;
  const noSeparator = /\s$/.test(left) || /^\s/.test(right) || /^[，。！？、,.!?;；:：…]/.test(rightText);
  const separator = noSeparator || (language !== "en" && language !== "ko") ? "" : " ";
  return `${leftText}${separator}${rightText}`;
}

async function undoSubtitleStructure(): Promise<void> {
  if (!activeDetail || !undoSubtitleStructureButton || !await flushEditorStructureDrafts()) return;
  if (currentCueEditor.dirty) {
    setSubtitleEditAction("撤销打轴前请先保存或放弃当前文字修改。", true);
    return;
  }
  const jobId = activeDetail.job.job_id;
  const selectedId = editorSession.selectedCueId;
  const history = subtitleStructureUndoHistory.get(jobId);
  const entry = history?.at(-1);
  if (!entry) return;
  undoSubtitleStructureButton.disabled = true;
  setEditorMutationBusy(true);
  try {
    const restored = await invoke<SubtitleSegment[]>("restore_subtitle_structure", {
      request: {
        jobId,
        beforeSegments: entry.before,
        afterSegments: entry.after,
      },
    });
    if (activeDetail?.job.job_id !== jobId) return;
    history?.pop();
    if (history?.length === 0) subtitleStructureUndoHistory.delete(activeDetail.job.job_id);
    applySubtitleStructure(restored, selectedId ?? undefined);
    setSubtitleEditAction(`已撤销${entry.label}。撤销历史只在当前 App 会话内保留。`);
  } catch (error) {
    setSubtitleEditAction(`无法撤销结构修改：${String(error)}`, true);
  } finally {
    setEditorMutationBusy(false);
    updateStructureUndoButton();
  }
}

function updateSubtitleDraftUI(draft: Draft<SubtitleSegment>, before?: SubtitleSegment): void {
  try { localStorage.setItem("atogaki.subtitleDrafts", subtitleDrafts.snapshot()); } catch { /* SQLite autosave still runs. */ }
  const saved = draft.saved;
  if (before) rememberSubtitleSave(before, saved);
  if (activeDetail?.job.job_id !== saved.job_id) return;
  if (before) {
    const index = subtitleTimeline(), position = index.position(saved.id);
    if (position >= 0 && index.replaceText(saved)) activeDetail.segments[position] = saved;
    else activeDetail.segments = activeDetail.segments.map(cue => cue.id === saved.id ? saved : cue);
    editorFilterCache = null;
  }
  const pending = subtitleDrafts.pending(draft);
  if (currentArea === "karaoke") { if (before && (editorSession.query.trim() || editorSession.filter !== "all")) renderEditorList(); else editorCueList.update(saved); }
  const status = draft.error ? `保存失败，草稿已保留：${draft.error}`
    : draft.saving ? "正在自动保存…" : pending ? "等待自动保存…" : savedSegmentState(saved);
  const card = subtitleList?.querySelector<HTMLElement>(`[data-segment-id="${CSS.escape(saved.id)}"]`);
  if (card) {
    card.dataset.dirty = String(pending);
    const state = card.querySelector<HTMLElement>(".subtitle-save-state");
    if (state) { state.textContent = status; state.classList.toggle("warning", !!draft.error || saved.translation_stale); }
    const save = card.querySelector<HTMLButtonElement>("[data-save-draft]");
    const discard = card.querySelector<HTMLButtonElement>("[data-discard-draft]");
    const undo = card.querySelector<HTMLButtonElement>("[data-undo-draft]");
    if (save) save.disabled = !pending || draft.saving;
    if (discard) discard.disabled = !pending || draft.saving;
    if (undo) undo.disabled = pending || !canUndoSegment(saved);
    const flags = card.querySelector<HTMLElement>(".edit-flags");
    if (flags) flags.textContent = editFlags(saved);
  }
  if (currentCueEditor.isShowing(saved)) {
    currentCueEditor.refresh(draft);
    setSubtitleEditAction(status, !!draft.error);
  }
  if (before) updateTranslationControls();
  if (before) { updateActiveSubtitle((mediaPlayer.media?.currentTime ?? 0) * 1_000); updateStructureUndoButton(); }
}

function renderSubtitleList(segments: SubtitleSegment[], _expandedSegmentId?: string): void {
  if (currentArea === "karaoke") { renderEditorList(); return; }
  if (!subtitleList) return;
  subtitleList.replaceChildren();
  const message = document.createElement("p");
  message.textContent = segments.length ? `已生成 ${segments.length} 段；完整识别后进入字幕编辑。` : "识别尚未完成，可在任务列表查看进度与已保存原文。";
  subtitleList.append(message);
}

function savedSegmentState(segment: SubtitleSegment): string {
  return segment.translation_stale ? "原文或翻译上下文已变化，请核对当前译文" : "已保存到本地";
}

function subtitleValueFingerprint(segment: SubtitleSegment): string {
  return JSON.stringify([
    segment.source_text,
    segment.translated_text,
    segment.start_ms,
    segment.end_ms,
    segment.source_edited,
    segment.translation_edited,
    segment.translation_stale,
    segment.timing_edited,
  ]);
}

function subtitleUndoKey(segment: SubtitleSegment): string {
  return `${segment.job_id}:${segment.id}`;
}

function canUndoSegment(segment: SubtitleSegment): boolean {
  const history = subtitleUndoHistory.get(subtitleUndoKey(segment));
  const latest = history?.at(-1);
  return Boolean(latest && latest.afterFingerprint === subtitleValueFingerprint(segment));
}

function rememberSubtitleSave(before: SubtitleSegment, after: SubtitleSegment): void {
  if (subtitleValueFingerprint(before) === subtitleValueFingerprint(after)) return;
  const key = subtitleUndoKey(after);
  const history = subtitleUndoHistory.get(key) ?? [];
  history.push({ before: { ...before }, afterFingerprint: subtitleValueFingerprint(after) });
  if (history.length > 20) history.shift();
  subtitleUndoHistory.set(key, history);
}

function renderSubtitleListPreservingView(segments: SubtitleSegment[], expandedSegmentId?: string): void {
  const scrollX = window.scrollX;
  const scrollY = window.scrollY;
  renderSubtitleList(segments, expandedSegmentId);
  const restoreScroll = (): void => window.scrollTo(scrollX, scrollY);
  restoreScroll();
  requestAnimationFrame(restoreScroll);
}

async function restoreSegment(snapshot: SubtitleSegment): Promise<SubtitleSegment> {
  return editorBackend.restoreText(snapshot);
}


function replaceActiveSegment(updated: SubtitleSegment): void {
  if (!activeDetail) return;
  if (activeDetail.job.job_id !== updated.job_id) return;
  activeDetail.segments = activeDetail.segments.map((item) => (item.id === updated.id ? updated : item));
  subtitleDrafts.ensure(updated);
  if (currentArea === "karaoke") { currentCueEditor.sync(karaokeDraftSegment()); renderEditorList(); }
}

function editFlags(segment: SubtitleSegment): string {
  const flags = [];
  if (segment.source_edited) flags.push("原文已编辑");
  if (segment.translation_edited) flags.push("译文已编辑");
  if (segment.translation_stale) flags.push("待重译");
  if (segment.timing_edited) flags.push("时间轴已编辑");
  return flags.join(" · ");
}

function formatTime(milliseconds: number): string {
  const totalSeconds = milliseconds / 1_000;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = Math.floor(totalSeconds % 60);
  const tenths = Math.floor((milliseconds % 1_000) / 100);
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${tenths}`;
}

function formatPreciseTime(milliseconds: number): string {
  return formatEditableTime(milliseconds);
}

function formatEditableTime(milliseconds: number): string {
  const safe = Math.max(0, Math.round(milliseconds));
  const hours = Math.floor(safe / 3_600_000);
  const minutes = Math.floor((safe % 3_600_000) / 60_000);
  const seconds = Math.floor((safe % 60_000) / 1_000);
  const millis = safe % 1_000;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}

function parseTimecode(value: string): number | null {
  const parts = value.trim().replace(",", ".").split(":");
  if (parts.length < 1 || parts.length > 3 || parts.some((part) => part.trim() === "")) return null;
  const numbers = parts.map(Number);
  if (numbers.some((part) => !Number.isFinite(part) || part < 0)) return null;
  const seconds = numbers.at(-1) ?? 0;
  const minutes = numbers.length >= 2 ? numbers.at(-2) ?? 0 : 0;
  const hours = numbers.length === 3 ? numbers[0] : 0;
  if (seconds >= 60 || (numbers.length === 3 && minutes >= 60)) return null;
  return Math.round((hours * 3_600 + minutes * 60 + seconds) * 1_000);
}

function seekTo(milliseconds: number, autoplay = true): void {
  if (!mediaPlayer.media) return;
  mediaPlayer.seek(milliseconds);
  if (autoplay) void mediaPlayer.media.play().catch(() => undefined);
}

function activePlaybackRateSelect(): HTMLSelectElement | null {
  if (currentArea === "listening") return listeningPlaybackRateSelect;
  if (currentArea === "karaoke") return karaokePlaybackRateSelect;
  return playbackRateSelect;
}

function schedulePlaybackPositionSave(): void {
  if (currentArea !== "listening" || !mediaPlayer.media || Date.now() - lastPlaybackPositionSavedAt < 5_000) return;
  if (playbackPositionSaveTimer !== null) return;
  playbackPositionSaveTimer = window.setTimeout(() => {
    playbackPositionSaveTimer = null;
    void persistPlaybackPosition();
  }, 300);
}

async function persistPlaybackPosition(): Promise<void> {
  if (currentArea !== "listening" || !activeDetail || !mediaPlayer.media) return;
  await savePlaybackPosition(activeDetail.job.job_id, mediaPlayer.media);
}

async function savePlaybackPosition(jobId: string, media: HTMLMediaElement, intendedMs = media.currentTime * 1_000): Promise<void> {
  const positionMs = Math.max(0, Math.round(intendedMs));
  lastPlaybackPositionSavedAt = Date.now();
  await invoke("save_playback_position", {
    request: { jobId, positionMs },
  }).catch(() => undefined);
}

function updatePlaybackControls(): void {
  const disabled = !mediaPlayer.media;
  for (const button of [previousSubtitleButton, rewindMediaButton, togglePlaybackButton, forwardMediaButton, nextSubtitleButton, listeningPreviousSubtitleButton, listeningRewindMediaButton, listeningTogglePlaybackButton, listeningForwardMediaButton, listeningNextSubtitleButton, karaokeTogglePlaybackButton]) {
    if (button) button.disabled = disabled;
  }
  for (const select of [playbackRateSelect, listeningPlaybackRateSelect, karaokePlaybackRateSelect]) if (select) select.disabled = disabled;
  if (togglePlaybackButton) togglePlaybackButton.textContent = mediaPlayer.media && !mediaPlayer.media.paused ? "暂停" : "播放";
  if (listeningTogglePlaybackButton) listeningTogglePlaybackButton.textContent = mediaPlayer.media && !mediaPlayer.media.paused ? "暂停" : "播放";
  if (karaokeTogglePlaybackButton) karaokeTogglePlaybackButton.textContent = mediaPlayer.media && !mediaPlayer.media.paused ? "暂停" : "播放";
  if (mediaPlayer.media) for (const select of [playbackRateSelect, listeningPlaybackRateSelect, karaokePlaybackRateSelect]) if (select) select.value = String(mediaPlayer.media.playbackRate);
  mediaPlayer.refreshAnimation();
  if (subtitleOverlayVisible) syncSubtitleOverlay(subtitleAt((mediaPlayer.media?.currentTime ?? 0) * 1_000));
}

function togglePlayback(): void {
  if (!mediaPlayer.media) return;
  if (mediaPlayer.media.paused) void mediaPlayer.media.play().catch(() => undefined);
  else mediaPlayer.media.pause();
}

function seekRelative(seconds: number): void {
  if (!mediaPlayer.media) return;
  seekTo(mediaPlayer.positionMs + seconds * 1_000, false);
}

function seekAdjacentSubtitle(direction: -1 | 1): void {
  if (!mediaPlayer.media || !activeDetail || activeDetail.segments.length === 0) return;
  const currentMs = mediaPlayer.media.currentTime * 1_000;
  const activeIndex = activeDetail.segments.findIndex((segment) => segment.id === activeSegmentId);
  let targetIndex: number;
  if (activeIndex >= 0) {
    targetIndex = Math.max(0, Math.min(activeDetail.segments.length - 1, activeIndex + direction));
  } else if (direction > 0) {
    const nextIndex = activeDetail.segments.findIndex((segment) => segment.start_ms > currentMs);
    targetIndex = nextIndex < 0 ? activeDetail.segments.length - 1 : nextIndex;
  } else {
    const previous = activeDetail.segments.filter((segment) => segment.end_ms <= currentMs);
    targetIndex = Math.max(0, previous.length - 1);
  }
  seekTo(activeDetail.segments[targetIndex].start_ms, false);
}

function changePlaybackRate(direction: -1 | 1): void {
  const rateSelect = activePlaybackRateSelect();
  if (!mediaPlayer.media || !rateSelect) return;
  const rates = Array.from(rateSelect.options).map((option) => Number(option.value));
  const currentIndex = rates.findIndex((rate) => rate === mediaPlayer.media?.playbackRate);
  const nextIndex = Math.max(0, Math.min(rates.length - 1, (currentIndex < 0 ? rates.indexOf(1) : currentIndex) + direction));
  mediaPlayer.setRate(rates[nextIndex]);
}

function executePlaybackAction(action: PlaybackAction): void {
  if (action === "toggle-playback") togglePlayback();
  else if (action === "rewind") seekRelative(-5);
  else if (action === "forward") seekRelative(5);
  else if (action === "previous-subtitle") seekAdjacentSubtitle(-1);
  else if (action === "next-subtitle") seekAdjacentSubtitle(1);
  else if (action === "slower") changePlaybackRate(-1);
  else if (action === "faster") changePlaybackRate(1);
  else if (action === "toggle-overlay") void openSubtitleOverlay();
}

const subtitleTimelines = new WeakMap<SubtitleSegment[], SubtitleTimeline<SubtitleSegment>>();
function subtitleTimeline(segments = activeDetail?.segments ?? []): SubtitleTimeline<SubtitleSegment> {
  let index = subtitleTimelines.get(segments);
  if (!index) { index = new SubtitleTimeline(segments); subtitleTimelines.set(segments, index); }
  return index;
}
function subtitleAt(milliseconds: number): SubtitleSegment | undefined {
  return activeDetail ? subtitleTimeline().at(milliseconds) : undefined;
}

function subtitleOverlayPayload(segment: SubtitleSegment | undefined): SubtitleOverlayPayload {
  return {
    sourceText: segment?.source_text ?? "当前时间没有字幕。",
    translatedText: segment?.translated_text ?? null,
    sourceLanguage: activeSourceLanguage(),
    targetLanguage: activeTargetLanguage(),
    playing: Boolean(mediaPlayer.media && !mediaPlayer.media.paused),
    playbackRate: mediaPlayer.media?.playbackRate ?? 1,
  };
}

function renderSubtitleOverlayButton(): void {
  if (!openSubtitleOverlayButton) return;
  const canOpen = Boolean(currentArea === "listening" && activeDetail && activeDetail.segments.length > 0);
  openSubtitleOverlayButton.disabled = !canOpen;
  openSubtitleOverlayButton.textContent = subtitleOverlayVisible ? "悬浮字幕已显示" : "打开悬浮字幕";
}

function syncSubtitleOverlay(segment: SubtitleSegment | undefined): void {
  if (!subtitleOverlayVisible || !activeDetail) return;
  const payload = subtitleOverlayPayload(segment);
  const key = `${activeDetail.job.job_id}:${segment?.id ?? "none"}:${payload.sourceText}:${payload.translatedText ?? ""}:${payload.playing}:${payload.playbackRate}`;
  if (key === lastSubtitleOverlayKey) return;
  lastSubtitleOverlayKey = key;
  void invoke("update_subtitle_overlay", { payload }).catch(() => {
    subtitleOverlayVisible = false;
    lastSubtitleOverlayKey = "";
    renderSubtitleOverlayButton();
  });
}

async function openSubtitleOverlay(): Promise<void> {
  if (currentArea !== "listening" || !activeDetail || activeDetail.segments.length === 0) return;
  if (subtitleOverlayVisible) {
    hideSubtitleOverlay();
    return;
  }
  subtitleOverlayVisible = true;
  lastSubtitleOverlayKey = "";
  renderSubtitleOverlayButton();
  const segment = subtitleAt((mediaPlayer.media?.currentTime ?? 0) * 1_000);
  try {
    await invoke("open_subtitle_overlay", { payload: subtitleOverlayPayload(segment) });
    syncSubtitleOverlay(segment);
  } catch (error) {
    subtitleOverlayVisible = false;
    lastSubtitleOverlayKey = "";
    renderSubtitleOverlayButton();
    if (listeningMediaMessage) listeningMediaMessage.textContent = `无法打开悬浮字幕：${String(error)}`;
  }
}

function hideSubtitleOverlay(): void {
  if (!subtitleOverlayVisible) return;
  subtitleOverlayVisible = false;
  lastSubtitleOverlayKey = "";
  renderSubtitleOverlayButton();
  void invoke("hide_subtitle_overlay").catch(() => undefined);
}

function updateActiveSubtitle(milliseconds: number): void {
  if (mediaPlayer.media && playbackLoop && mediaPlayer.media.currentTime * 1000 >= playbackLoop.endMs) {
    if (playbackLoop.repeat) mediaPlayer.seek(playbackLoop.startMs);
    else { mediaPlayer.media.pause(); playbackLoop = null; }
  }

  const segment = subtitleAt(milliseconds);
  const nextId = segment?.id ?? null;
  const sourceText = segment?.source_text ?? "当前时间没有字幕。";
  if (currentArea === "workspace" && currentSource && currentSource.textContent !== sourceText) currentSource.textContent = sourceText;
  if (currentArea === "workspace" && currentTranslation) {
    const translated = segment?.translated_text || `尚无${languageLabel(activeTargetLanguage())}翻译`;
    if (currentTranslation.textContent !== translated) currentTranslation.textContent = translated;
    currentTranslation.classList.toggle("stale", segment?.translation_stale ?? false);
  }
  if (currentArea === "listening" && listeningCurrentSource && listeningCurrentSource.textContent !== sourceText) listeningCurrentSource.textContent = sourceText;
  if (currentArea === "listening" && listeningCurrentTranslation) {
    const translated = segment?.translated_text || `尚无${languageLabel(activeTargetLanguage())}翻译`;
    if (listeningCurrentTranslation.textContent !== translated) listeningCurrentTranslation.textContent = translated;
  }
  if (activeSegmentId !== nextId) {
    activeSegmentId = nextId;
    highlightSegment(nextId);
  }
  updateKaraokePosition(milliseconds);
  syncSubtitleOverlay(segment);
}

function subtitleFollowState(host: HTMLElement): SubtitleFollowState {
  const existing = subtitleFollowStates.get(host);
  if (existing) return existing;
  const created = { userScrollingUntil: 0, autoScrollingUntil: 0, resumeTimer: null };
  subtitleFollowStates.set(host, created);
  return created;
}

function markUserSubtitleScrolling(host: HTMLElement): void {
  const state = subtitleFollowState(host);
  if (Date.now() < state.autoScrollingUntil) return;
  state.userScrollingUntil = Date.now() + 3_000;
  if (state.resumeTimer !== null) window.clearTimeout(state.resumeTimer);
  state.resumeTimer = window.setTimeout(() => {
    state.resumeTimer = null;
    if (Date.now() < state.userScrollingUntil || !mediaPlayer.media || mediaPlayer.media.paused) return;
    highlightSegment(activeSegmentId);
  }, 3_050);
}

function registerSubtitleFollowHost(host: HTMLElement | null): void {
  if (!host) return;
  subtitleFollowState(host);
  for (const eventName of ["wheel", "touchstart", "pointerdown"] as const) {
    host.addEventListener(eventName, () => markUserSubtitleScrolling(host), { passive: true });
  }
  host.addEventListener("scroll", () => markUserSubtitleScrolling(host), { passive: true });
}

function followActiveSubtitle(host: HTMLElement, card: HTMLElement): void {
  const state = subtitleFollowState(host);
  if (!mediaPlayer.media || mediaPlayer.media.paused || Date.now() < state.userScrollingUntil) return;
  if (host.scrollHeight <= host.clientHeight + 1) return;
  const targetTop = Math.max(
    0,
    host.scrollTop + card.getBoundingClientRect().top - host.getBoundingClientRect().top - 8,
  );
  if (Math.abs(host.scrollTop - targetTop) < 4) return;
  const pageScrollTop = window.scrollY;
  state.autoScrollingUntil = Date.now() + 250;
  host.scrollTop = targetTop;
  if (window.scrollY !== pageScrollTop) window.scrollTo({ top: pageScrollTop, behavior: "auto" });
}

function highlightSegment(segmentId: string | null): void {
  const host = currentArea === "workspace" ? subtitleList : currentArea === "listening" ? listeningSubtitleList : null;
  if (!host) return;
  host.querySelector<HTMLElement>(".active[data-segment-id]")?.classList.remove("active");
  if (!segmentId) return;
  const card = host.querySelector<HTMLElement>(`[data-segment-id="${CSS.escape(segmentId)}"]`);
  if (card) { card.classList.add("active"); followActiveSubtitle(host, card); }
}

function segmentNeedsTranslation(segment: SubtitleSegment): boolean {
  return needsTranslation(segment, activeDetail?.translation_context_stale_ids ?? []);
}

function addGlossaryTermRow(
  sourceText = "",
  targetText = "",
  promptScope: "core" | "content" | "correction_only" = "core",
  contentGroup = "",
): void {
  if (!glossaryTerms) return;
  const row = document.createElement("div");
  row.className = "glossary-term-row";
  const scope = document.createElement("select");
  scope.className = "term-scope";
  scope.innerHTML = `<option value="core">核心</option><option value="content">内容包</option><option value="correction_only">仅修正</option>`;
  scope.value = promptScope;
  const group = document.createElement("input");
  group.className = "term-group";
  group.value = contentGroup;
  group.placeholder = "内容包名称";
  const kind = document.createElement("select");
  kind.className = "term-kind";
  kind.innerHTML = `<option value="prompt">提示词</option><option value="correction">提示＋修正</option>`;
  kind.value = targetText ? "correction" : "prompt";
  const source = document.createElement("input");
  source.className = "term-source";
  source.value = sourceText;
  const arrow = document.createElement("span");
  arrow.textContent = "→";
  const target = document.createElement("input");
  target.className = "term-target";
  target.value = targetText;
  const remove = document.createElement("button");
  remove.type = "button";
  remove.className = "term-remove secondary";
  remove.textContent = "移除";
  remove.addEventListener("click", () => row.remove());
  const syncKind = (): void => {
    const correction = kind.value === "correction";
    row.dataset.termKind = kind.value;
    source.placeholder = correction ? "读音或常见误识别" : "希望 Whisper 识别出的写法";
    target.disabled = !correction;
    target.placeholder = correction ? "最终规范写法" : "提示词不需要目标写法";
    arrow.classList.toggle("inactive", !correction);
    if (!correction) target.value = "";
  };
  const syncScope = (): void => {
    const correctionOnly = scope.value === "correction_only";
    const content = scope.value === "content";
    const correctionOption = kind.querySelector<HTMLOptionElement>('option[value="correction"]');
    row.dataset.promptScope = scope.value;
    if (correctionOption) {
      correctionOption.textContent = correctionOnly ? "修正规则" : "提示＋修正";
    }
    group.disabled = !content;
    group.placeholder = content ? "例如：幻燈・夏の肖像" : "当前类型不使用内容包";
    if (!content) group.value = "";
    if (correctionOnly) kind.value = "correction";
    kind.disabled = correctionOnly;
    syncKind();
  };
  kind.addEventListener("change", syncKind);
  scope.addEventListener("change", syncScope);
  row.append(scope, group, kind, source, arrow, target, remove);
  syncScope();
  glossaryTerms.append(row);
}

async function editGlossary(glossaryId: string | null): Promise<void> {
  editingGlossaryId = glossaryId;
  renderGlossaryList();
  if (glossaryMessage) glossaryMessage.textContent = "";
  if (glossaryTerms) glossaryTerms.replaceChildren();
  if (!glossaryId) {
    if (glossaryName) glossaryName.value = "";
    if (glossaryLanguage) glossaryLanguage.value = selectedSourceLanguage();
    if (deleteGlossaryButton) deleteGlossaryButton.disabled = true;
    if (saveGlossaryButton) saveGlossaryButton.textContent = "保存词表";
    addGlossaryTermRow();
    glossaryName?.focus();
    return;
  }

  if (glossaryMessage) glossaryMessage.textContent = "正在读取词表…";
  try {
    const detail = await invoke<GlossaryDetail>("get_glossary", { glossaryId });
    if (glossaryName) glossaryName.value = detail.glossary.name;
    if (glossaryLanguage) glossaryLanguage.value = detail.glossary.source_language;
    if (deleteGlossaryButton) {
      deleteGlossaryButton.disabled = Boolean(detail.glossary.builtin_key);
      deleteGlossaryButton.title = detail.glossary.builtin_key ? "内置词表随 App 更新，不能删除" : "";
    }
    if (saveGlossaryButton) {
      saveGlossaryButton.textContent = detail.glossary.builtin_key ? "复制并保存" : "保存词表";
      saveGlossaryButton.title = detail.glossary.builtin_key
        ? "内置词表保持只读；保存会创建可编辑的自定义副本"
        : "";
    }
    if (glossaryTerms) glossaryTerms.replaceChildren();
    for (const term of detail.terms) {
      addGlossaryTermRow(
        term.source_text,
        term.target_text ?? "",
        term.prompt_scope,
        term.content_group ?? "",
      );
    }
    if (detail.terms.length === 0) addGlossaryTermRow();
    if (glossaryMessage) {
      const summary = `核心 ${detail.glossary.core_term_count} 条 · 内容 ${detail.glossary.content_term_count} 条／${detail.glossary.content_group_count} 包 · 仅修正 ${detail.glossary.correction_only_count} 条`;
      glossaryMessage.textContent = detail.glossary.builtin_key
        ? `${summary} · 内置词表随 App 更新，保存时会创建自定义副本`
        : summary;
    }
  } catch (error) {
    if (glossaryMessage) glossaryMessage.textContent = `读取失败：${String(error)}`;
  }
}

async function openGlossaryManager(preferredGlossaryId?: string | null): Promise<void> {
  await refreshGlossaries();
  glossaryDialog?.showModal();
  const preferred = preferredGlossaryId || taskGlossary?.value || glossaries[0]?.id || null;
  await editGlossary(preferred);
}

async function saveGlossaryEditor(): Promise<void> {
  if (!glossaryName || !glossaryLanguage || !glossaryTerms || !glossaryMessage) return;
  const name = glossaryName.value.trim();
  const rows = Array.from(glossaryTerms.querySelectorAll<HTMLElement>(".glossary-term-row"));
  if (
    rows.some(
      (row) =>
        row.dataset.termKind === "correction" &&
        !row.querySelector<HTMLInputElement>(".term-target")?.value.trim(),
    )
  ) {
    glossaryMessage.textContent = "修正规则必须填写最终规范写法。";
    return;
  }
  if (
    rows.some(
      (row) =>
        row.dataset.promptScope === "content" &&
        !row.querySelector<HTMLInputElement>(".term-group")?.value.trim(),
    )
  ) {
    glossaryMessage.textContent = "内容包词条必须填写内容包名称。";
    return;
  }
  const terms = rows
    .map((row) => ({
      sourceText: row.querySelector<HTMLInputElement>(".term-source")?.value.trim() ?? "",
      targetText:
        row.dataset.termKind === "correction"
          ? row.querySelector<HTMLInputElement>(".term-target")?.value.trim() || null
          : null,
      promptScope: row.dataset.promptScope ?? "core",
      contentGroup:
        row.dataset.promptScope === "content"
          ? row.querySelector<HTMLInputElement>(".term-group")?.value.trim() || null
          : null,
    }))
    .filter((term) => term.sourceText || term.targetText);
  glossaryMessage.textContent = "正在保存…";
  try {
    const copiedFromBuiltin = Boolean(
      glossaries.find((glossary) => glossary.id === editingGlossaryId)?.builtin_key,
    );
    const detail = await invoke<GlossaryDetail>("save_glossary", {
      request: {
        glossaryId: editingGlossaryId,
        name,
        sourceLanguage: glossaryLanguage.value as LanguageCode,
        terms,
      },
    });
    editingGlossaryId = detail.glossary.id;
    await refreshGlossaries();
    if (taskGlossary && detail.glossary.source_language === selectedSourceLanguage()) {
      taskGlossary.value = detail.glossary.id;
    }
    if (workspaceGlossary && detail.glossary.source_language === activeSourceLanguage()) {
      workspaceGlossary.value = detail.glossary.id;
    }
    await refreshTaskGlossaryConfiguration();
    glossaryMessage.textContent = copiedFromBuiltin
      ? `已从内置版本创建“${detail.glossary.name}”，保存 ${detail.terms.length} 条词条。`
      : `已保存 ${detail.terms.length} 条词条。`;
    await editGlossary(detail.glossary.id);
  } catch (error) {
    glossaryMessage.textContent = `保存失败：${String(error)}`;
  }
}

async function deleteGlossaryEditor(): Promise<void> {
  if (!editingGlossaryId) return;
  const selected = glossaries.find((glossary) => glossary.id === editingGlossaryId);
  const confirmed = await confirmAction({
    title: "删除识别词表？",
    message: `将删除词表“${selected?.name ?? editingGlossaryId}”。旧任务仍保留提交时的词表快照。`,
    confirmLabel: "删除词表",
    danger: true,
  });
  if (!confirmed) return;
  if (glossaryMessage) glossaryMessage.textContent = "正在删除…";
  try {
    await invoke("delete_glossary", { glossaryId: editingGlossaryId });
    editingGlossaryId = null;
    await refreshGlossaries();
    await editGlossary(glossaries[0]?.id ?? null);
  } catch (error) {
    if (glossaryMessage) glossaryMessage.textContent = `删除失败：${String(error)}`;
  }
}

function captureGlossaryCorrection(
  segment: SubtitleSegment,
  sourceInput: HTMLTextAreaElement,
  state: HTMLElement,
): void {
  const glossaryId = workspaceGlossary?.value;
  if (!glossaryId) {
    setWorkspaceAction("请先在工作区选择一个词表。", true);
    return;
  }
  pendingGlossaryCorrection = { glossaryId, state };
  if (glossaryCorrectionSource) glossaryCorrectionSource.value = segment.source_text;
  if (glossaryCorrectionTarget) glossaryCorrectionTarget.value = sourceInput.value.trim();
  if (glossaryCorrectionMessage) glossaryCorrectionMessage.textContent = "";
  glossaryCorrectionDialog?.showModal();
  glossaryCorrectionSource?.focus();
  glossaryCorrectionSource?.select();
}

async function saveGlossaryCorrection(): Promise<void> {
  if (!pendingGlossaryCorrection || !glossaryCorrectionSource || !glossaryCorrectionTarget) return;
  const { glossaryId, state } = pendingGlossaryCorrection;
  const sourceText = glossaryCorrectionSource.value.trim();
  const targetText = glossaryCorrectionTarget.value.trim();
  if (!sourceText || !targetText) {
    if (glossaryCorrectionMessage) glossaryCorrectionMessage.textContent = "请填写误识别和规范写法。";
    return;
  }
  if (sourceText === targetText) {
    if (glossaryCorrectionMessage) glossaryCorrectionMessage.textContent = "误识别与规范写法相同，没有创建规则。";
    return;
  }

  state.textContent = "正在加入识别词表…";
  if (glossaryCorrectionMessage) glossaryCorrectionMessage.textContent = "正在保存…";
  try {
    const detail = await invoke<GlossaryDetail>("get_glossary", { glossaryId });
    const terms = detail.terms.map((term) => ({
      sourceText: term.source_text,
      targetText: term.target_text,
      promptScope: term.prompt_scope,
      contentGroup: term.content_group,
    }));
    if (terms.some((term) => term.sourceText === sourceText && term.targetText === targetText)) {
      setWorkspaceAction("这条修正规则已经存在于词表中。", true);
      state.textContent = "这条修正规则已经存在";
      glossaryCorrectionDialog?.close();
      return;
    }
    terms.push({ sourceText, targetText, promptScope: "correction_only", contentGroup: null });
    const saved = await invoke<GlossaryDetail>("save_glossary", {
      request: {
        glossaryId,
        name: detail.glossary.name,
        sourceLanguage: detail.glossary.source_language,
        terms,
      },
    });
    await refreshGlossaries();
    if (workspaceGlossary) workspaceGlossary.value = saved.glossary.id;
    setWorkspaceAction(`已把“${sourceText} → ${targetText}”加入 ${saved.glossary.name}。字幕修改仍需单独保存。`);
    state.textContent = "修正规则已加入词表；字幕修改需单独保存";
    state.classList.remove("warning");
    glossaryCorrectionDialog?.close();
  } catch (error) {
    state.textContent = `加入词表失败：${String(error)}`;
    state.classList.add("warning");
    if (glossaryCorrectionMessage) glossaryCorrectionMessage.textContent = `保存失败：${String(error)}`;
  }
}

function clearGlossaryPreview(): void {
  pendingGlossaryPreview = null;
  glossaryPreviewHost?.classList.add("hidden");
  if (glossaryPreviewHost) glossaryPreviewHost.replaceChildren();
}

async function previewGlossaryApplication(): Promise<void> {
  if (!activeDetail || !workspaceGlossary?.value || workspaceActionBusy) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("请先保存字幕修改，再预览词表修正。", true);
    return;
  }
  setWorkspaceBusy(true);
  setWorkspaceAction("正在比较词表与 原文字幕…");
  try {
    pendingGlossaryPreview = await invoke<GlossaryPreview>("preview_glossary_application", {
      jobId: activeDetail.job.job_id,
      glossaryId: workspaceGlossary.value,
    });
    renderGlossaryPreview(pendingGlossaryPreview);
    setWorkspaceAction(
      pendingGlossaryPreview.changes.length
        ? `词表预览完成：${pendingGlossaryPreview.changes.length} 段会改变。`
        : "词表预览完成，没有需要修正的字幕。",
    );
  } catch (error) {
    clearGlossaryPreview();
    setWorkspaceAction(`词表预览失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

function renderGlossaryPreview(preview: GlossaryPreview): void {
  if (!glossaryPreviewHost) return;
  const staleCount = preview.changes.filter((change) => change.translation_will_be_stale).length;
  const examples = preview.changes
    .slice(0, 4)
    .map(
      (change) => `<li><span>#${change.segment_index + 1}</span><del>${escapeHtml(change.before_text)}</del><strong>${escapeHtml(change.after_text)}</strong></li>`,
    )
    .join("");
  glossaryPreviewHost.innerHTML = preview.changes.length
    ? `<div><strong>${escapeHtml(preview.glossary_name)} 将修改 ${preview.changes.length} 段</strong><span>${staleCount} 段已有译文，应用后会标记为待重译。</span></div><ul>${examples}</ul><button id="apply-previewed-glossary" type="button">确认应用</button>`
    : `<div><strong>${escapeHtml(preview.glossary_name)} 无匹配修正</strong><span>提示词只影响新转写；这里只预览“错误写法 → 规范写法”规则。</span></div>`;
  glossaryPreviewHost.classList.remove("hidden");
  document.querySelector<HTMLButtonElement>("#apply-previewed-glossary")?.addEventListener("click", () => void applyPreviewedGlossary());
}

async function applyPreviewedGlossary(): Promise<void> {
  if (!activeDetail || !pendingGlossaryPreview || workspaceActionBusy) return;
  setWorkspaceBusy(true);
  setWorkspaceAction("正在把词表修正写入本地…");
  try {
    const applied = await invoke<GlossaryApplyResult>("apply_glossary_to_workspace", {
      jobId: activeDetail.job.job_id,
      glossaryId: pendingGlossaryPreview.glossary_id,
    });
    activeDetail.segments = applied.segments;
    clearGlossaryPreview();
    renderSubtitleListPreservingView(activeDetail.segments);
    updateActiveSubtitle((mediaPlayer.media?.currentTime ?? 0) * 1_000);
    setWorkspaceAction(
      `已修正 ${applied.changed_segments} 段并保存到本地；${applied.stale_translations} 段译文需要重译。`,
    );
  } catch (error) {
    setWorkspaceAction(`应用词表失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

document.querySelector<HTMLButtonElement>("#refresh")?.addEventListener("click", () => void refresh());
document.querySelector<HTMLButtonElement>("#show-workbench")?.addEventListener("click", () => void showTopLevelArea("workbench"));
document.querySelector<HTMLButtonElement>("#show-listening")?.addEventListener("click", () => void showTopLevelArea("listening"));
document.querySelector<HTMLButtonElement>("#show-learning")?.addEventListener("click", () => void showTopLevelArea("learning"));
document.querySelector<HTMLButtonElement>("#refresh-learning")?.addEventListener("click", () => void refreshLearningItems());
learningLanguageTabs?.addEventListener("click", (event) => {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>("[data-learning-language]");
  const language = button?.dataset.learningLanguage;
  if (language === "ja" || language === "en" || language === "ko") selectLearningLanguage(language);
});
document.querySelector<HTMLButtonElement>("#close-learning-dictionary")?.addEventListener("click", closeLearningDictionary);
learningDictionaryDialog?.addEventListener("close", () => {
  activeLearningItemId = null;
  activeLearningProviderId = "summary";
});
listeningSubtitleList?.addEventListener("mouseup", captureListeningSelection);
listeningSubtitleList?.addEventListener("keyup", captureListeningSelection);
saveLearningSelectionButton?.addEventListener("click", () => {
  if (pendingLearningSelection) void persistLearningSelection(pendingLearningSelection, "selection");
});
saveLearningSentenceButton?.addEventListener("click", () => {
  if (pendingLearningSelection) void saveLearningSentence(pendingLearningSelection.segmentId);
});
document.querySelector<HTMLButtonElement>("#close-learning-selection")?.addEventListener("click", () => closeLearningSelection());
document.querySelector<HTMLButtonElement>("#open-settings")?.addEventListener("click", () => {
  settingsDirtyFields.clear();
  if (!settingsDialog?.open) settingsDialog?.showModal();
  void loadDesktopSettings(false);
});
document.querySelector<HTMLButtonElement>("#close-settings")?.addEventListener("click", () => settingsDialog?.close());
settingsDialog?.addEventListener("close", () => settingsDirtyFields.clear());
settingsForm?.addEventListener("input", (event) => {
  const target = event.target;
  if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement) {
    settingsDirtyFields.add(target.id);
  }
});
settingsForm?.addEventListener("change", (event) => {
  const target = event.target;
  if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement) {
    settingsDirtyFields.add(target.id);
  }
});
document.querySelector<HTMLButtonElement>("#settings-choose-whisper")?.addEventListener("click", () => void chooseSettingsModel("whisper"));
document.querySelector<HTMLButtonElement>("#settings-choose-vad")?.addEventListener("click", () => void chooseSettingsModel("vad"));
document.querySelector<HTMLButtonElement>("#settings-choose-hy-mt2")?.addEventListener("click", () => void chooseSettingsModel("hy-mt2"));
const providerDrafts = new Map<string, { model: string; endpoint: string; style: string }>();
const providerDirtyDrafts = new Set<string>();
const providerReadiness = new Map<string, string>();
type TranslationProfileSummary = { provider_id: string; model: string | null; base_url: string | null; style: string; readiness: string };
function updateProviderProfiles(profiles: TranslationProfileSummary[]): void {
  for (const profile of profiles) {
    providerReadiness.set(profile.provider_id, profile.readiness);
    if (!providerDirtyDrafts.has(profile.provider_id)) providerDrafts.set(profile.provider_id, {
      model: profile.model ?? "", endpoint: profile.base_url ?? (profile.provider_id === "openai-compatible" ? "https://api.openai.com/v1" : "https://api.deepseek.com"), style: profile.style,
    });
  }
}
for (const input of [settingsProviderModel, settingsProviderBaseUrl, settingsTranslationStyle]) input?.addEventListener("input", () => {
  if (settingsProvider) {
    providerDirtyDrafts.add(settingsProvider.value);
    providerDrafts.set(settingsProvider.value, { model: settingsProviderModel?.value ?? "", endpoint: settingsProviderBaseUrl?.value ?? "", style: settingsTranslationStyle?.value ?? "" });
  }
  renderProviderCards();
});
let lastSettingsProvider = "none";
settingsProvider?.addEventListener("change", () => {
  // Never carry a secret or delete intent into another provider.
  if (settingsApiKey) settingsApiKey.value = "";
  if (settingsClearApiKey) settingsClearApiKey.checked = false;
  providerDrafts.set(lastSettingsProvider, { model: settingsProviderModel?.value ?? "", endpoint: settingsProviderBaseUrl?.value ?? "", style: settingsTranslationStyle?.value ?? "" });
  lastSettingsProvider = settingsProvider.value;
  if (settingsProvider.value === "deepseek") {
    if (settingsProviderBaseUrl) settingsProviderBaseUrl.value = "https://api.deepseek.com";
    if (settingsProviderModel) settingsProviderModel.value = "deepseek-v4-flash";
  } else if (settingsProvider.value === "gemini") {
    if (settingsProviderBaseUrl) settingsProviderBaseUrl.value = "https://generativelanguage.googleapis.com/v1beta/openai";
    if (settingsProviderModel) settingsProviderModel.value = "gemini-3.8-flash";
  } else if (settingsProvider.value === "openai-compatible") {
    if (settingsProviderBaseUrl) settingsProviderBaseUrl.value = "https://api.openai.com/v1";
    if (settingsProviderModel) settingsProviderModel.value = "";
  } else if (settingsProviderModel) {
    settingsProviderModel.value = "";
  }
  if ((settingsProvider.value === "gemini" || settingsProvider.value === "deepseek" || settingsProvider.value === "openai-compatible") && settingsTranslationStyle && !settingsTranslationStyle.value.trim()) {
    settingsTranslationStyle.value = "准确、自然的简体中文口语字幕；保留说话语气，不补充原文没有的信息。";
  }
  const draft = providerDrafts.get(settingsProvider.value);
  if (draft) {
    if (settingsProviderModel) settingsProviderModel.value = draft.model;
    if (settingsProviderBaseUrl) settingsProviderBaseUrl.value = draft.endpoint;
    if (settingsTranslationStyle) settingsTranslationStyle.value = draft.style;
  }
  syncProviderSettings();
  if (apiKeyStatus && settingsProvider.value !== "none") {
    apiKeyStatus.textContent = "尚未检查当前 provider；可点击“检查所选 Key”确认系统凭据是否存在。";
    apiKeyStatus.classList.remove("warning");
  }
  renderProviderCards();
});
checkApiKeyButton?.addEventListener("click", () => void checkSelectedApiKey());
document.querySelector("#dictionary-credentials")?.addEventListener("click", (event) => {
  const target = event.target;
  if (!(target instanceof HTMLButtonElement)) return;
  const saveId = target.dataset.saveDictionaryKey;
  const clearId = target.dataset.clearDictionaryKey;
  const checkId = target.dataset.checkDictionaryKey;
  if (saveId) void saveDictionaryCredential(saveId);
  else if (clearId) void saveDictionaryCredential(clearId, true);
  else if (checkId) void checkDictionaryCredential(checkId);
});
settingsProxyMode?.addEventListener("change", syncNetworkSettings);
testNetworkButton?.addEventListener("click", () => void testNetworkConnection());
settingsForm?.addEventListener("submit", (event) => {
  event.preventDefault();
  void saveSettings(false);
});
document.querySelector<HTMLButtonElement>("#finish-settings")?.addEventListener("click", () => void saveSettings(true));
workspaceView.querySelector<HTMLButtonElement>("#back-to-jobs")?.addEventListener("click", () => {
  if (currentArea === "karaoke") { closeEditorTools(); return; }
  void showTopLevelArea("workbench").then(() => refresh());
});
workspaceView.querySelector<HTMLButtonElement>("#reload-detail")?.addEventListener("click", () => {
  if (activeDetail) void openJob(activeDetail.job.job_id, currentWorkspaceSection);
});
relinkJobMediaButton?.addEventListener("click", () => void relinkActiveJobMedia());
openSubtitleEditorButton?.addEventListener("click", () => {
  if (!activeDetail) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("进入字幕编辑前，请先保存或放弃当前字幕编辑中的文字草稿。", true);
    return;
  }
  void enterSubtitleEditor(activeDetail.job.job_id);
});
workspaceSectionTabs.forEach((tab) => {
  tab.addEventListener("click", () => {
    const section = tab.dataset.workspaceSection as WorkspaceSection | undefined;
    if (section) showWorkspaceSection(section);
  });
  tab.addEventListener("keydown", (event) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    const currentIndex = workspaceSectionTabs.indexOf(tab);
    const direction = event.key === "ArrowRight" ? 1 : -1;
    const nextIndex = (currentIndex + direction + workspaceSectionTabs.length) % workspaceSectionTabs.length;
    const next = workspaceSectionTabs[nextIndex];
    const section = next?.dataset.workspaceSection as WorkspaceSection | undefined;
    if (next && section) {
      showWorkspaceSection(section);
      next.focus();
    }
    event.preventDefault();
  });
});
translatePendingButton?.addEventListener("click", () => void subtitleTranslation.batch(true));
translateAllButton?.addEventListener("click", () => void subtitleTranslation.batch(false));
openSubtitleOverlayButton?.addEventListener("click", () => void openSubtitleOverlay());
previousSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(-1));
rewindMediaButton?.addEventListener("click", () => seekRelative(-5));
togglePlaybackButton?.addEventListener("click", togglePlayback);
forwardMediaButton?.addEventListener("click", () => seekRelative(5));
nextSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(1));
playbackRateSelect?.addEventListener("change", () => {
  if (playbackRateSelect) mediaPlayer.setRate(Number(playbackRateSelect.value));
});
listeningPreviousSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(-1));
listeningRewindMediaButton?.addEventListener("click", () => seekRelative(-5));
listeningTogglePlaybackButton?.addEventListener("click", togglePlayback);
listeningForwardMediaButton?.addEventListener("click", () => seekRelative(5));
listeningNextSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(1));
listeningPlaybackRateSelect?.addEventListener("change", () => {
  if (listeningPlaybackRateSelect) mediaPlayer.setRate(Number(listeningPlaybackRateSelect.value));
});
syncKaraokeZoomLabel();
karaokeTogglePlaybackButton?.addEventListener("click", togglePlayback);
karaokePlaybackRateSelect?.addEventListener("change", () => {
  if (karaokePlaybackRateSelect) mediaPlayer.setRate(Number(karaokePlaybackRateSelect.value));
});
document.querySelector<HTMLButtonElement>("#karaoke-step-back-small")?.addEventListener("click", () => seekKaraokeBy(-10));
document.querySelector<HTMLButtonElement>("#karaoke-step-back")?.addEventListener("click", () => seekKaraokeBy(-100));
document.querySelector<HTMLButtonElement>("#karaoke-step-forward")?.addEventListener("click", () => seekKaraokeBy(100));
document.querySelector<HTMLButtonElement>("#karaoke-step-forward-small")?.addEventListener("click", () => seekKaraokeBy(10));
karaokeWaveformGainInput?.addEventListener("input", () => {
  karaokeWaveformGain = Number(karaokeWaveformGainInput.value);
  if (karaokeWaveformGainLabel) karaokeWaveformGainLabel.value = `${karaokeWaveformGain.toFixed(karaokeWaveformGain % 1 ? 2 : 0)}×`;
  renderKaraokeTimeline();
});
karaokeCutSegmentButton?.addEventListener("click", () => void cutKaraokeSegmentAtPlayhead());
karaokeJoinSegmentButton?.addEventListener("click", () => void joinKaraokeSegmentWithNext());
editorRepairDialog.querySelector("#retranscription-use-current")?.addEventListener("click", () => usePlayheadSubtitleForRetranscription());
retranscriptionUseWindowButton?.addEventListener("click", useVisibleWindowForRetranscription);
retranscriptionExpandSegmentsButton?.addEventListener("click", expandRetranscriptionRangeToSegments);
retranscriptionProvider?.addEventListener("change", () => {
  const cloud = retranscriptionProvider.value === "gemini-transcribe";
  editorRepairDialog.querySelector<HTMLElement>("#gemini-upload-consent")?.classList.toggle("hidden", !cloud);
  if (!cloud && authorizeGeminiUpload) authorizeGeminiUpload.checked = false;
});

previewRetranscriptionButton?.addEventListener("click", () => void previewSelectedRange());
retranscriptionDialog?.querySelector<HTMLButtonElement>("#close-retranscription-preview")?.addEventListener("click", () => candidateTool.close());
retranscriptionDialog?.querySelector<HTMLButtonElement>("#cancel-retranscription")?.addEventListener("click", () => candidateTool.close());
confirmRetranscriptionButton?.addEventListener("click", () => void confirmSelectedRangeReplacement());
saveGeminiApiKeyButton?.addEventListener("click", () => void saveGeminiCredential(false));
clearGeminiApiKeyButton?.addEventListener("click", () => void saveGeminiCredential(true));
checkGeminiApiKeyButton?.addEventListener("click", () => void checkGeminiCredential());
karaokeOpenWorkspaceButton?.addEventListener("click", () => void showEditorTools("translation"));

document.querySelector<HTMLButtonElement>("#manage-glossaries")?.addEventListener("click", () => void openGlossaryManager());
workspaceView.querySelector<HTMLButtonElement>("#manage-workspace-glossary")?.addEventListener("click", () => void openGlossaryManager(workspaceGlossary?.value));
document.querySelector<HTMLButtonElement>("#close-glossaries")?.addEventListener("click", () => glossaryDialog?.close());
document.querySelector<HTMLButtonElement>("#new-glossary")?.addEventListener("click", () => void editGlossary(null));
document.querySelector<HTMLButtonElement>("#add-glossary-term")?.addEventListener("click", () => addGlossaryTermRow());
document.querySelector<HTMLButtonElement>("#save-glossary")?.addEventListener("click", () => void saveGlossaryEditor());
deleteGlossaryButton?.addEventListener("click", () => void deleteGlossaryEditor());
workspaceView.querySelector<HTMLButtonElement>("#preview-glossary")?.addEventListener("click", () => void previewGlossaryApplication());
workspaceGlossary?.addEventListener("change", () => {
  clearGlossaryPreview();
  void renderWorkspaceGlossaryInspection();
  updateTranslationControls();
});
renameJobForm?.addEventListener("submit", (event) => {
  event.preventDefault();
  void saveRenamedJob();
});
document.querySelector<HTMLButtonElement>("#cancel-rename-job")?.addEventListener("click", () => {
  renamingJob = null;
  renameJobDialog?.close();
});
renameJobDialog?.addEventListener("close", () => {
  renamingJob = null;
  if (renameJobMessage) renameJobMessage.textContent = "";
});
confirmationForm?.addEventListener("submit", (event) => {
  event.preventDefault();
  settleConfirmation(true);
});
document.querySelector<HTMLButtonElement>("#cancel-confirmation")?.addEventListener("click", () => {
  settleConfirmation(false);
});
confirmationDialog?.addEventListener("cancel", (event) => {
  event.preventDefault();
  settleConfirmation(false);
});
confirmationDialog?.addEventListener("close", () => {
  if (confirmationResolver) settleConfirmation(false);
});
undoSubtitleStructureButton?.addEventListener("click", () => void undoSubtitleStructure());
glossaryCorrectionForm?.addEventListener("submit", (event) => {
  event.preventDefault();
  void saveGlossaryCorrection();
});
document.querySelector<HTMLButtonElement>("#cancel-glossary-correction")?.addEventListener("click", () => {
  glossaryCorrectionDialog?.close();
});
glossaryCorrectionDialog?.addEventListener("close", () => {
  pendingGlossaryCorrection = null;
  if (glossaryCorrectionMessage) glossaryCorrectionMessage.textContent = "";
});

function isTextEntryTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || target instanceof HTMLButtonElement
    || (target instanceof HTMLElement && (target.isContentEditable || target.getAttribute("role") === "separator"));
}

document.addEventListener("keydown", (event) => {
  if (!activeDetail || !mediaPlayer.media || event.altKey || isTextEntryTarget(event.target)) return;
  if (document.querySelector("dialog[open]") || workspaceView?.classList.contains("editor-task-tools")) return;
  if (currentArea === "karaoke" && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b") {
    void cutKaraokeSegmentAtPlayhead();
    event.preventDefault();
    return;
  }
  if (event.metaKey || event.ctrlKey) return;
  if (currentArea === "karaoke" && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
    seekKaraokeBy((event.key === "ArrowLeft" ? -1 : 1) * (event.shiftKey ? 10 : 100));
    event.preventDefault();
    return;
  }
  const action = playbackActionForKey(event);
  if (!action || (event.repeat && (action === "toggle-playback" || action === "toggle-overlay"))) return;
  executePlaybackAction(action);
  event.preventDefault();
});

async function chooseFile(kind: "media" | "model" | "vad"): Promise<void> {
  const buttonSelector = kind === "media" ? "#choose-media" : kind === "model" ? "#choose-model" : "#choose-vad-model";
  const button = document.querySelector<HTMLButtonElement>(
    buttonSelector,
  );
  if (button) button.disabled = true;
  const label = kind === "media" ? "媒体" : kind === "model" ? "Whisper 模型" : "VAD 模型";
  if (taskMessage) taskMessage.textContent = `正在打开${label}选择器…`;
  try {
    const command = kind === "media" ? "pick_media_file" : kind === "model" ? "pick_model_file" : "pick_vad_model_file";
    const selected = kind === "media" ? await invoke<string[]>("pick_media_files") : null;
    const path = selected ? selected[0] : await invoke<string | null>(command);
    if (kind === "media") selectedMediaFiles = selected ?? [];
    if (typeof path === "string") {
      const input = kind === "media" ? mediaPath : kind === "model" ? modelPath : vadModelPath;
      if (input) input.value = path;
      if (taskMessage) taskMessage.textContent = `已选择${label}${kind === "media" && selectedMediaFiles.length > 1 ? ` ${selectedMediaFiles.length} 个文件` : ""}。`;
    } else if (taskMessage) {
      taskMessage.textContent = "已取消选择。";
    }
  } catch (error) {
    if (taskMessage) taskMessage.textContent = `无法打开文件选择器：${String(error)}`;
  } finally {
    if (button) button.disabled = kind === "vad" && !vadEnabled?.checked;
  }
}

function syncVadControls(): void {
  const enabled = vadEnabled?.checked ?? false;
  if (vadModelPath) {
    vadModelPath.disabled = !enabled;
    vadModelPath.required = enabled;
  }
  const chooseButton = document.querySelector<HTMLButtonElement>("#choose-vad-model");
  if (chooseButton) chooseButton.disabled = !enabled;
}

document.querySelector<HTMLButtonElement>("#choose-media")?.addEventListener("click", () => void chooseFile("media"));
document.querySelector<HTMLButtonElement>("#choose-model")?.addEventListener("click", () => void chooseFile("model"));
document.querySelector<HTMLButtonElement>("#choose-vad-model")?.addEventListener("click", () => void chooseFile("vad"));
vadEnabled?.addEventListener("change", syncVadControls);
taskGlossary?.addEventListener("change", () => void refreshTaskGlossaryConfiguration());
sourceLanguage?.addEventListener("change", () => {
  renderGlossaryOptions();
  void refreshTaskGlossaryConfiguration();
});

document.querySelector<HTMLFormElement>("#task-form")?.addEventListener("submit", (event) => {
  event.preventDefault();
  if (!mediaPath?.value || !modelPath?.value || !taskMessage || !submitButton) return;
  if (vadEnabled?.checked && !vadModelPath?.value.trim()) {
    taskMessage.textContent = "启用 VAD 时需要选择 Silero VAD 模型。";
    vadModelPath?.focus();
    return;
  }
  submitButton.disabled = true;
  taskMessage.textContent = "正在创建本地任务…";
  const request = {
      inputPath: mediaPath.value,
      modelPath: modelPath.value,
      sourceLanguage: selectedSourceLanguage(),
      translationProviderId: (document.querySelector<HTMLSelectElement>("#task-translation-provider")?.value || "none"),
      vadModelPath: vadEnabled?.checked ? vadModelPath?.value.trim() || null : null,
      glossaryId: taskGlossary?.value || null,
      selectedContentGroups: taskGlossary?.value ? Array.from(selectedTaskContentGroups) : [],
    };
  const inputs = selectedMediaFiles.length && selectedMediaFiles[0] === mediaPath.value ? [...selectedMediaFiles] : [mediaPath.value];
  void (async () => {
    let succeeded = 0; const failures: string[] = [];
    for (const inputPath of inputs) {
      taskMessage.textContent = `正在排队 ${succeeded + failures.length + 1}/${inputs.length}…`;
      try { await invoke<string>("submit_transcription", {request: {...request,inputPath}}); succeeded++; }
      catch(error) { failures.push(`${inputPath}: ${String(error)}`); }
      await refresh();
    }
    taskMessage.textContent = `已排队 ${succeeded} 个任务${failures.length ? `；${failures.length} 个失败：${failures.join("；")}` : ""}`;
  })().finally(() => { submitButton.disabled = false; });
});

registerSubtitleFollowHost(subtitleList);
registerSubtitleFollowHost(listeningSubtitleList);
syncVadControls();
renderSubtitleOverlayButton();
void loadDesktopSettings(true);

void invoke<string>("data_directory")
  .then((path) => {
    if (dataPath) dataPath.textContent = `本地数据：${path}`;
  })
  .catch(() => {
    if (dataPath) dataPath.textContent = "本地数据目录暂不可用。";
  });
void refreshTranslationStatus();
void refreshGlossaries();
void refresh();
void listen<boolean>("subtitle-overlay-visibility", (event) => {
  subtitleOverlayVisible = event.payload;
  lastSubtitleOverlayKey = "";
  renderSubtitleOverlayButton();
});
void listen<{ action: PlaybackAction }>("subtitle-overlay-playback-action", (event) => {
  if (!activeDetail || !mediaPlayer.media) return;
  executePlaybackAction(event.payload.action);
});
window.setInterval(() => void refresh(), 2_000);
window.setInterval(() => void refreshActiveJob(), 2_000);
window.setInterval(updateVisibleJobTimings, 1_000);


async function showPartialTranscript(jobId: string): Promise<void> {
  const dialog = document.createElement("dialog");
  dialog.className = "settings-dialog";
  dialog.innerHTML =
    '<div class="dialog-heading"><h2>已保存原文 · 只读</h2><button type="button">关闭</button></div><p>完整识别后开放校对、手动重译、收藏和导出。只显示通过校验的完成块；已选择云端翻译时，稳定原文可在后台提前翻译。</p><div class="partial-media"></div><div class="partial-text">正在读取…</div>';
  document.body.append(dialog);
  dialog.querySelector("button")!.onclick = () => dialog.close();
  dialog.addEventListener("close", () => {
    dialog.querySelector("audio")?.pause();
    dialog.remove();
  });
  dialog.showModal();
  try {
    const [segments, detail] = await Promise.all([
      invoke<SubtitleSegment[]>("partial_transcript", { jobId }),
      invoke<JobDetail>("get_job_detail", { jobId }),
    ]);
    const audio = document.createElement("audio");
    audio.controls = true;
    audio.src = detail.audio_fallback_path ?? detail.playback_path ?? "";
    dialog.querySelector(".partial-media")!.append(audio);
    dialog.querySelector(".partial-text")!.innerHTML =
      segments
        .map(
          (segment) =>
            `<p><button type="button" data-time="${segment.start_ms}" class="secondary">${formatTime(segment.start_ms)}</button> ${escapeHtml(segment.source_text)}</p>`,
        )
        .join("") || "还没有已完成的音频块。";
    dialog.querySelectorAll<HTMLButtonElement>("[data-time]").forEach(
      (button) =>
        (button.onclick = () => {
          audio.currentTime = Number(button.dataset.time) / 1000;
          void audio.play();
        }),
    );
  } catch (error) {
    dialog.querySelector(".partial-text")!.textContent = String(error);
  }
}

window.addEventListener("atogaki-theme-change", () => renderKaraokeTimeline());
let providerEditor: HTMLDivElement | null = null;
function renderProviderCards(): void {
  const selected = settingsProvider?.value ?? "none";
  const labels: Record<string, string> = {
    disabled: "仅转录", unconfigured: "未配置凭据", resources_missing: "本地资源缺失",
    unverified: "凭据已配置，尚未验证连接", verified: "上次连接测试通过", validation_failed: "上次连接验证失败", configuration_missing: "配置不完整，请保存模型与端点",
  };
  for (const card of document.querySelectorAll<HTMLElement>("[data-provider-card]")) {
    const id = card.dataset.providerCard!;
    card.classList.toggle("active", selected === id);
    const status = card.querySelector<HTMLElement>("[data-provider-readiness]");
    if (status) status.textContent = id === "hy-mt2-local" && providerReadiness.get(id) === "unverified" ? "本地资源就绪，尚未验证" : labels[providerReadiness.get(id) ?? ""] ?? "正在读取配置状态…";
    if (status && providerDirtyDrafts.has(id)) status.textContent += "（当前草稿未保存）";
    const body = card.querySelector<HTMLElement>("[data-provider-editor]");
    if (selected === id && body && providerEditor) {
      if (providerEditor.parentElement !== body) body.append(providerEditor);
      providerEditor.classList.remove("hidden");
      const advanced = card.closest("details");
      if (advanced) advanced.open = true;
    }
  }
  if (selected === "none" || selected === "hy-mt2-local") providerEditor?.classList.add("hidden");
  document.querySelector("[data-save-provider-key]")?.classList.toggle("hidden", selected === "none" || selected === "hy-mt2-local");
  document.querySelector("[data-delete-provider-key]")?.classList.toggle("hidden", selected === "none" || selected === "hy-mt2-local");
  const selector = document.querySelector<HTMLSelectElement>("#local-translation-model-choice");
  if (selector) {
    const installed = modelDownloads.filter(state => state.status === "done" && state.path && availableModels.some(m => m.id === state.modelId && m.kind === "hy-mt2"));
    const current = settingsHyMt2Model?.value ?? "";
    const entries = installed.map(state => ({path:state.path!, name:availableModels.find(m=>m.id===state.modelId)!.name}));
    if (current && !entries.some(e=>e.path===current)) entries.unshift({path:current,name:current.split(/[\\/]/).pop() ?? "当前模型"});
    selector.innerHTML = '<option value="">选择已安装模型</option>'+entries.map(e=>`<option value="${escapeHtml(e.path)}">${escapeHtml(e.name)}</option>`).join("");
    selector.value = current;
  }
}

async function refreshProviderReadiness(): Promise<void> {
  updateProviderProfiles(await invoke<TranslationProfileSummary[]>("translation_profiles"));
  renderProviderCards();
}

function organizeTranslationCards(section: HTMLElement): void {
  const heading = section.querySelector(".settings-section-heading");
  const selector = settingsProvider?.closest("label");
  if (!selector) return;
  // Card actions replace the older shared check/delete controls.
  settingsClearApiKey?.closest("label")?.remove();
  checkApiKeyButton?.closest("div")?.remove();
  selector.firstChild!.textContent = "下次新任务的默认翻译方式（保存后生效）";
  providerEditor = document.createElement("div");
  providerEditor.className = "provider-editor hidden";
  for (const child of Array.from(section.children)) if (child !== heading && child !== selector) providerEditor.append(child);
  const cards = document.createElement("div"); cards.className = "provider-cards";
  const entries = [
    ["deepseek", "DeepSeek", "云端文本翻译；只发送字幕与上下文。"],
    ["gemini", "Gemini", "只发送字幕文本和上下文；与 Gemini 语音识别共用 Key。保存或删除 Key 会同时影响两处，音频上传仍需单独授权。"],
    ["deepl", "DeepL", "云端文本翻译；使用独立的 DeepL 凭据。"],
    ["hy-mt2-local", "Hy-MT2 · 本地翻译", "离线翻译，无需上传字幕。1.8B 占用较少，7B 需要更多内存和处理时间。"],
    ["openai-compatible", "OpenAI-compatible", "高级：自定义端点、模型与独立凭据。"],
  ];
  const advanced = document.createElement("details"); advanced.innerHTML = "<summary>高级：自定义兼容服务</summary>";
  for (const [id, title, description] of entries) {
    const card = document.createElement("article"); card.className = "provider-card"; card.dataset.providerCard = id;
    card.innerHTML = `<h4>${escapeHtml(title)}</h4><p>${escapeHtml(description)}</p><p data-provider-readiness role="status"></p><button type="button" class="secondary" data-configure-provider>配置此后端</button><button type="button" class="secondary" data-test-provider>测试已保存配置（发送 Hello.，可能计费）</button>${id === "hy-mt2-local" ? '<label class="local-model-choice">翻译模型<select id="local-translation-model-choice"></select></label><p>模型详情与下载位于“模型与存储”。</p>' : '<button type="button" class="secondary" data-check-provider>检查凭据是否存在</button>'}<div data-provider-editor></div>`;
    if (id === "hy-mt2-local") card.querySelector("[data-test-provider]")?.classList.add("hidden");
    card.querySelector<HTMLButtonElement>("[data-configure-provider]")!.onclick = () => {
      if (!settingsProvider) return;
      if (settingsProvider.value !== id) { settingsProvider.value = id; settingsProvider.dispatchEvent(new Event("change", { bubbles: true })); }
      renderProviderCards();
    };
    card.querySelector<HTMLButtonElement>("[data-test-provider]")!.onclick = async (event) => {
      const button = event.currentTarget as HTMLButtonElement; button.disabled = true;
      try { await invoke<string>("test_translation_connection", { providerId: id }); }
      catch (error) { if (settingsMessage) settingsMessage.textContent = `连接测试失败：${String(error)}`; }
      finally { button.disabled = false; await refreshProviderReadiness().catch(() => {}); }
    };
    const check = card.querySelector<HTMLButtonElement>("[data-check-provider]");
    if (check) check.onclick = async () => {
      check.disabled = true;
      try {
        const result = await invoke<TranslationCredentialCheck>("check_translation_api_key", { providerId: id });
        if (settingsMessage) settingsMessage.textContent = `${result.providerName}：${result.storedInSystem || result.availableFromEnvironment ? "凭据存在，尚不代表连接已通过" : "未找到凭据"}；本次未调用翻译 API。`;
        await refreshProviderReadiness();
      } catch (error) { if (settingsMessage) settingsMessage.textContent = String(error); }
      finally { check.disabled = false; }
    };
    if (id === "openai-compatible") advanced.append(card); else cards.append(card);
  }
  cards.append(advanced); section.append(cards);
  document.querySelector<HTMLSelectElement>("#local-translation-model-choice")?.addEventListener("change", event => {
    if (settingsHyMt2Model) { settingsHyMt2Model.value = (event.target as HTMLSelectElement).value; settingsHyMt2Model.dispatchEvent(new Event("input", {bubbles:true})); }
  });
  const actions = document.createElement("div"); actions.className = "offline-tools";
  actions.innerHTML = '<button type="button" data-save-provider-key>保存此后端凭据</button><button type="button" class="secondary" data-delete-provider-key>删除此后端凭据</button>';
  providerEditor.append(actions);
  for (const button of actions.querySelectorAll<HTMLButtonElement>("button")) button.onclick = async () => {
    const id = settingsProvider?.value, secret = settingsApiKey?.value ?? "";
    if (!id) return;
    button.disabled = true;
    try {
      await invoke("save_translation_credential", { providerId: id, apiKey: button.hasAttribute("data-save-provider-key") ? secret : null, clear: button.hasAttribute("data-delete-provider-key") });
      if (settingsProvider?.value === id && settingsApiKey?.value === secret) settingsApiKey.value = "";
      if (settingsClearApiKey) settingsClearApiKey.checked = false;
      await refreshProviderReadiness();
      if (id === "gemini") {
        renderAsrCredentialStatus(await invoke<AsrCredentialStatus>("asr_credential_status"));
        if (settingsGeminiApiKey) settingsGeminiApiKey.value = "";
      }
      if (settingsProvider?.value === id && apiKeyStatus) apiKeyStatus.textContent = `${id} 凭据已${button.hasAttribute("data-delete-provider-key") ? "删除" : "保存"}；连接状态请查看卡片。`;
      if (settingsMessage) settingsMessage.textContent = `${id} 凭据已${button.hasAttribute("data-delete-provider-key") ? "删除" : "保存"}；${id === "gemini" ? "语音识别与翻译共用此 Key，音频上传仍需单独授权。" : "其他后端配置未改变。"}`;
    } catch (error) { if (settingsMessage) settingsMessage.textContent = String(error); }
    finally { button.disabled = false; }
  };
  renderProviderCards();
}

function organizeSettings(): void {
  const form = document.querySelector<HTMLFormElement>("#settings-form");
  if (!form) return;
  const sections = Array.from(form.querySelectorAll<HTMLElement>(":scope > .settings-section"));
  const models = sections[0],
    network = sections[1],
    asr = sections[2],
    translation = sections[3],
    dictionaries = sections[4];
  if (!models || !network || !asr || !translation || !dictionaries) return;
  const downloads = document.createElement("section");
  downloads.className = "settings-section";
  downloads.innerHTML = "<h3>模型与存储</h3><p>统一下载、校验与卸载。模型按需安装。</p>";
  for (const selector of ["#models-directory", "#model-catalog", "#model-download-message"]) {
    const item = form.querySelector(selector);
    if (item) downloads.append(item);
  }
  const localModel = document.querySelector("#settings-hy-mt2-model")?.closest(".settings-path-row");
  if (localModel) downloads.append(localModel);
  const appearance = document.createElement("section"); appearance.className = "settings-section appearance-settings";
  appearance.innerHTML = '<h3>外观</h3><p>选择页面主题，即时生效并自动保存。字幕导出样式保持独立。</p><div class="theme-options"><button type="button" data-app-theme="moonlight"><span class="theme-swatch moonlight-swatch"></span><strong>月夜</strong><span>颜料笔触、月光与青灰纸面</span></button><button type="button" data-app-theme="letter"><span class="theme-swatch letter-swatch"></span><strong>来信</strong><span>纸纤维、折痕与信笺层次</span></button></div>';
  const syncTheme = () => appearance.querySelectorAll<HTMLButtonElement>("[data-app-theme]").forEach(button => button.setAttribute("aria-pressed", String(button.dataset.appTheme === document.documentElement.dataset.theme)));
  appearance.querySelectorAll<HTMLButtonElement>("[data-app-theme]").forEach(button => button.onclick = () => { applyTheme(button.dataset.appTheme as "moonlight"|"letter"); syncTheme(); });
  syncTheme();
  window.addEventListener("atogaki-theme-change",syncTheme);
  models.querySelector("strong")!.textContent = "本地转录";
  const optionalAsr = document.createElement("details"); optionalAsr.className="onboarding-optional";
  optionalAsr.innerHTML='<summary>可选：Gemini 局部修复</summary>';
  optionalAsr.append(asr); models.append(optionalAsr);
  translation.querySelector("strong")!.textContent = "翻译";
  organizeTranslationCards(translation);
  network.querySelector("strong")!.textContent = "网络与高级";
  dictionaries.querySelector("strong")!.textContent = "词典";
  const nav = document.createElement("nav");
  nav.className = "settings-tabs";
  nav.setAttribute("aria-label", "设置分区");
  const pages = [
    ["appearance", "外观", appearance],
    ["transcription", "转录", models],
    ["translation", "翻译", translation],
    ["models", "模型与存储", downloads],
    ["dictionary", "词典", dictionaries],
    ["network", "网络与高级", network],
  ] as const;
  const footer = form.querySelector(".settings-footer");
  const content = document.createElement("div");
  content.className = "settings-content";
  form.insertBefore(content, footer);
  pages.forEach(([key, label, page]) => {
    page.dataset.settingsPage = key;
    content.append(page);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "secondary";
    button.textContent = label;
    button.dataset.settingsTab = key;
    button.onclick = () => {
      pages.forEach(([candidate, , section]) => section.classList.toggle("hidden", candidate !== key));
      nav.querySelectorAll("button").forEach((item) => item.classList.toggle("active", item === button));
      content.scrollTop = 0;
    };
    nav.append(button);
    page.classList.toggle("hidden", key !== "transcription");
  });
  form.querySelector(".dialog-help")?.after(nav);
  const heading = form.querySelector("h2");
  if (heading) heading.textContent = "设置";
  const advanced = document.createElement("details");
  advanced.innerHTML = "<summary>高级：模型路径与 VAD 参数</summary>";
  const taskForm = document.querySelector("#task-form");
  for (const selector of ["#model-path", "#choose-model", "#vad-model-path", "#choose-vad-model"]) {
    const control = document.querySelector(selector);
    if (control) advanced.append(control.tagName === "INPUT" ? control.closest("label")! : control);
  }
  taskForm?.querySelector(".form-footer")?.before(advanced);
}
organizeSettings();

function addOfflineTools(): void {
  const jobsSearch = document.createElement("input");
  jobsSearch.type = "search";
  jobsSearch.placeholder = "搜索任务标题";
  jobsSearch.setAttribute("aria-label", "搜索任务标题");
  jobsSearch.oninput = () => {
    taskTitleQuery = jobsSearch.value;
    renderJobs(latestJobs);
  };
  jobList?.before(jobsSearch);
  const learningBar = document.createElement("div");
  learningBar.className = "offline-tools";
  learningBar.innerHTML =
    '<input id="learning-search" type="search" aria-label="搜索学习条目" placeholder="搜索词句、释义、例句"/><select id="learning-task-filter" aria-label="按来源任务筛选"><option value="">全部来源任务</option></select><button id="export-anki" type="button" class="secondary">导出当前单词本到 Anki</button>';
  learningItemList?.before(learningBar);
  learningBar.querySelector<HTMLInputElement>("input")!.oninput = (event) => {
    learningQuery = (event.target as HTMLInputElement).value;
    renderLearningItems();
  };
  learningBar.querySelector<HTMLSelectElement>("select")!.onchange = (event) => {
    learningTaskFilter = (event.target as HTMLSelectElement).value;
    renderLearningItems();
  };
  learningBar.querySelector<HTMLButtonElement>("button")!.onclick = async () => {
    try {
      const path = await invoke<string | null>("export_learning_anki", { language: activeLearningLanguage });
      if (learningMessage)
        learningMessage.textContent = path
          ? `已导出 ${path}；在 Anki 中导入 TSV 并按列映射。`
          : "已取消导出。";
    } catch (error) {
      if (learningMessage) learningMessage.textContent = String(error);
    }
  };
  for (const host of document.querySelectorAll(".playback-tools")) {
    const tools = document.createElement("div");
    tools.className = "offline-tools loop-tools";
    tools.innerHTML =
      '<button type="button" class="secondary" data-loop="sentence">循环当前句</button><button type="button" class="secondary" data-loop="a">设置 A</button><button type="button" class="secondary" data-loop="b">设置 B 并循环</button><button type="button" class="secondary" data-loop="head">试听句首衔接</button><button type="button" class="secondary" data-loop="tail">试听句尾衔接</button><button type="button" class="secondary" data-loop="off">关闭循环</button><span class="loop-state">循环已关闭</span>';
    host.after(tools);
    tools.querySelectorAll<HTMLButtonElement>("[data-loop]").forEach(
      (button) =>
        (button.onclick = () => {
          if (!mediaPlayer.media) return;
          const time = mediaPlayer.media.currentTime * 1000,
            cue = subtitleAt(time),
            action = button.dataset.loop;
          if (action === "off") {
            playbackLoop = null;
            loopStartMs = null;
          } else if (action === "a") {
            loopStartMs = time;
            playbackLoop = null;
          } else if (action === "b") {
            if (loopStartMs === null || time <= loopStartMs) {
              tools.querySelector("span")!.textContent = "请先设置早于 B 的 A 点";
              return;
            }
            playbackLoop = { startMs: loopStartMs, endMs: time, repeat: true };
          } else if (cue) {
            playbackLoop =
              action === "sentence"
                ? { startMs: cue.start_ms, endMs: cue.end_ms, repeat: true }
                : action === "head"
                  ? {
                      startMs: Math.max(0, cue.start_ms - 500),
                      endMs: Math.min(cue.end_ms, cue.start_ms + 1000),
                      repeat: false,
                    }
                  : {
                      startMs: Math.max(cue.start_ms, cue.end_ms - 1000),
                      endMs: Math.min(
                        Number.isFinite(mediaPlayer.media.duration)
                          ? mediaPlayer.media.duration * 1000
                          : cue.end_ms + 500,
                        cue.end_ms + 500,
                      ),
                      repeat: false,
                    };
          }
          if (playbackLoop) {
            mediaPlayer.seek(playbackLoop.startMs);
            void mediaPlayer.media.play();
          }
          document
            .querySelectorAll<HTMLElement>(".loop-state")
            .forEach(
              (e) =>
                (e.textContent = playbackLoop
                  ? `${playbackLoop.repeat ? "循环" : "试听"} ${formatPreciseTime(playbackLoop.startMs)} → ${formatPreciseTime(playbackLoop.endMs)}`
                  : loopStartMs !== null
                    ? `A: ${formatPreciseTime(loopStartMs)}`
                    : "循环已关闭"),
            );
        }),
    );
  }
  for (const input of [retranscriptionStartInput, retranscriptionEndInput])
    input?.addEventListener("input", () => {
      if (authorizeGeminiUpload) authorizeGeminiUpload.checked = false;
    });

}
addOfflineTools();

mediaPath?.addEventListener("input", () => {
  selectedMediaFiles = [];
});
const presetTools = document.createElement("div");
presetTools.className = "offline-tools";
presetTools.innerHTML =
  '<select aria-label="任务预设"><option value="">选择任务预设</option></select><input aria-label="预设名称" placeholder="预设名称" maxlength="64"/><button type="button" class="secondary">保存当前配置为预设</button><span>预设不保存媒体路径或 API Key。</span>';
document.querySelector("#task-form")?.prepend(presetTools);
let taskPresets: Array<{
  name: string;
  configuration: {
    modelPath: string;
    vadModelPath: string | null;
    sourceLanguage: LanguageCode;
    glossaryId: string | null;
    selectedContentGroups: string[];
    translationProviderId: string;
  };
}> = [];
async function loadTaskPresets(): Promise<void> {
  try {
    taskPresets = await invoke("task_presets");
    presetTools.querySelector("select")!.innerHTML =
      '<option value="">选择任务预设</option>' +
      taskPresets.map((p, i) => `<option value="${i}">${escapeHtml(p.name)}</option>`).join("");
  } catch (e) {
    if (taskMessage) taskMessage.textContent = `预设读取失败：${String(e)}`;
  }
}
presetTools.querySelector<HTMLSelectElement>("select")!.onchange = async (event) => {
  const value = (event.target as HTMLSelectElement).value;
  if (!value) return;
  const config = taskPresets[Number(value)]?.configuration;
  if (!config) return;
  if (modelPath) modelPath.value = config.modelPath;
  if (sourceLanguage) sourceLanguage.value = config.sourceLanguage;
  renderGlossaryOptions();
  if (vadEnabled) vadEnabled.checked = Boolean(config.vadModelPath);
  if (vadModelPath) vadModelPath.value = config.vadModelPath ?? "";
  if (taskGlossary) taskGlossary.value = config.glossaryId ?? "";
  taskGlossaryConfigurationId = taskGlossary?.value || null;
  const provider = document.querySelector<HTMLSelectElement>("#task-translation-provider");
  if (provider) provider.value = config.translationProviderId;
  selectedTaskContentGroups.clear();
  config.selectedContentGroups.forEach((g) => selectedTaskContentGroups.add(g));
  syncVadControls();
  await refreshTaskGlossaryConfiguration();
  updateTaskPipelineSummary();
};
presetTools.querySelector<HTMLButtonElement>("button")!.onclick = async () => {
  try {
    await invoke("save_task_preset", {
      name: presetTools.querySelector("input")!.value,
      configuration: {
        modelPath: modelPath?.value,
        vadModelPath: vadEnabled?.checked ? vadModelPath?.value : null,
        sourceLanguage: selectedSourceLanguage(),
        glossaryId: taskGlossary?.value || null,
        selectedContentGroups: Array.from(selectedTaskContentGroups),
        translationProviderId:
          document.querySelector<HTMLSelectElement>("#task-translation-provider")?.value || "none",
      },
    });
    await loadTaskPresets();
    if (taskMessage) taskMessage.textContent = "预设已保存。";
  } catch (e) {
    if (taskMessage) taskMessage.textContent = String(e);
  }
};
void loadTaskPresets();

function updateTaskPipelineSummary():void {
  const summary=document.querySelector<HTMLElement>("#task-pipeline-summary");
  if(!summary)return;
  const provider=document.querySelector<HTMLSelectElement>("#task-translation-provider");
  const model=(modelPath?.value??"").split(/[\\/]/).at(-1)||"未选择模型";
  summary.textContent=`转录：Whisper · ${model} · 本地；翻译：${provider?.selectedOptions[0]?.textContent??"未选择"}`;
}
const pipelineSummary=document.createElement("p");pipelineSummary.id="task-pipeline-summary";pipelineSummary.className="settings-message";
document.querySelector("#task-form")?.before(pipelineSummary);
for(const input of [modelPath,document.querySelector<HTMLSelectElement>("#task-translation-provider")])input?.addEventListener("change",updateTaskPipelineSummary);
updateTaskPipelineSummary();


document.querySelector("#karaoke-insert-block")?.addEventListener("click",openInsertSubtitleBlock);
document.querySelector("#karaoke-delete-block")?.addEventListener("click",()=>void deleteCurrentSubtitleBlock());

const appearanceHost = document.querySelector<HTMLElement>(".appearance-settings");
if (appearanceHost) void setupImageThemes(appearanceHost).catch(error => { if(settingsMessage) settingsMessage.textContent=String(error); });

// External websites are opened by the OS, rather than a blocked WebView popup.
document.addEventListener("click", event => {
  const link=(event.target as HTMLElement).closest<HTMLAnchorElement>("a[href]");
  if(!link||!/^https?:/i.test(link.href))return;
  event.preventDefault();
  void invoke("open_external_url",{url:link.href}).catch(error=>{if(settingsMessage)settingsMessage.textContent=String(error);});
});

try { subtitleDrafts.recover(localStorage.getItem("atogaki.subtitleDrafts") ?? "[]"); } catch { /* SQLite remains authoritative. */ }
let subtitleCloseReady = false;
void getCurrentWindow().onCloseRequested(async event => {
  if (subtitleCloseReady) return;
  event.preventDefault();
  try {
    await subtitleDrafts.flushAll();
    subtitleCloseReady = true;
    await getCurrentWindow().close();
  } catch (error) {
    setSubtitleEditAction(`保存失败，窗口保持打开；草稿已保留：${String(error)}`, true);
  }
});
