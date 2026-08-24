import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { playbackActionForKey, type PlaybackAction } from "./playback-shortcuts";
import "./styles.css";

type LanguageCode = "ja" | "en" | "ko" | "zh-Hans";

type LocalJob = {
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

type SubtitleSegment = {
  id: string;
  job_id: string;
  segment_index: number;
  start_ms: number;
  end_ms: number;
  source_text: string;
  translated_text: string | null;
  source_edited: boolean;
  translation_edited: boolean;
  translation_stale: boolean;
  timing_edited: boolean;
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

type WaveformPeak = {
  min: number;
  max: number;
};

type WaveformWindow = {
  duration_ms: number;
  start_ms: number;
  end_ms: number;
  point_duration_ms: number;
  peaks: WaveformPeak[];
};

type KaraokeTimingDrag = {
  pointerId: number;
  segmentIndex: number;
  mode: "move" | "start" | "end";
  pointerStartX: number;
  before: SubtitleSegment[];
  draft: SubtitleSegment[];
  moved: boolean;
};

type KaraokeTimelineHit = {
  segmentIndex: number;
  mode: "move" | "start" | "end";
};

type KaraokePanDrag = {
  pointerId: number;
  pointerStartX: number;
  viewStartMs: number;
  moved: boolean;
};

type KaraokeGestureEvent = Event & {
  scale: number;
  clientX: number;
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
  completed_at_unix: number;
};

type TranslationStatus = {
  provider_id: string;
  provider: string;
  configured: boolean;
  model: string | null;
  endpoint_kind: string;
  configuration_hint: string | null;
};

type RecognitionDefaults = {
  whisperModelPath: string | null;
  vadModelPath: string | null;
};

type DesktopSettings = {
  onboardingCompleted: boolean;
  needsOnboarding: boolean;
  whisperModelPath: string | null;
  whisperModelReady: boolean;
  vadModelPath: string | null;
  vadModelReady: boolean;
  translationProviderId: "none" | "deepl" | "deepseek" | "openai-compatible";
  translationModel: string | null;
  translationBaseUrl: string | null;
  translationStyleInstruction: string;
  translationApiKeyConfigured: boolean;
  translationApiKeySource: "system" | "environment" | "saved" | "deferred" | null;
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

type ModelCatalogItem = {
  id: string;
  kind: "whisper" | "vad";
  name: string;
  fileName: string;
  sizeLabel: string;
  recommendedFor: string;
  sourceUrl: string;
};

type ModelDownloadState = {
  modelId: string;
  status: "queued" | "downloading" | "done" | "failed";
  downloadedBytes: number;
  totalBytes: number | null;
  path: string | null;
  error: string | null;
  source: string | null;
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
  providerId: "cambridge" | "collins" | "merriam-webster";
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

type SubtitleExport = {
  source_srt: string;
  translated_srt: string;
  bilingual_srt: string;
  bilingual_ass: string;
  missing_translation_count: number;
};

type SubtitleExportPlan = {
  output_directory: string;
  base_name: string;
  source_srt: string;
  translated_srt: string;
  bilingual_srt: string;
  bilingual_ass: string;
  existing_files: string[];
};

type SubtitleExportArtifact = "source_srt" | "translated_srt" | "bilingual_srt" | "bilingual_ass";

type MediaCapabilities = {
  binary_path: string;
  version: string;
  ass_filter: boolean;
  videotoolbox_encoder: boolean;
  mpeg4_encoder: boolean;
  ready_for_hard_subtitles: boolean;
};

type VideoRender = {
  id: string;
  source_job_id: string;
  input_path: string;
  subtitle_path: string;
  output_path: string;
  subtitle_track: "source" | "translation" | "bilingual";
  status: "queued" | "running" | "done" | "failed" | "cancelled";
  progress: number;
  encoder: string | null;
  audio_encoder: string | null;
  fallback_reason: string | null;
  error_message: string | null;
  created_at_unix: number;
  updated_at_unix: number;
};

type VideoOutputSelection = {
  path: string;
  alreadyExists: boolean;
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
        <p>选择节目语言、本地媒体与 Whisper 模型后，任务会先写入本地 SQLite，再在后台开始转写。</p>
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
      <p id="learning-message" class="learning-message" role="status"></p>
      <div id="learning-item-list" class="learning-item-list"></div>
    </section>
    <section id="karaoke-view" class="karaoke-view hidden" aria-labelledby="karaoke-title">
      <div class="module-heading karaoke-heading">
        <div><p class="eyebrow">ADVANCED SUBTITLE EDITOR</p><h2 id="karaoke-title">字幕编辑</h2><p>针对当前任务精确播放、调整时间轴并集中修正字幕文字。</p></div>
      </div>
      <div class="karaoke-layout">
        <section class="karaoke-editor" aria-label="波形时间轴编辑器">
          <div class="panel-heading compact">
            <div><p class="eyebrow">TIMING SESSION</p><h3 id="karaoke-job-title">正在读取当前任务</h3></div>
            <button id="karaoke-open-workspace" type="button" class="secondary" disabled>← 返回字幕校对</button>
          </div>
          <div class="karaoke-player-strip">
            <div id="karaoke-media-host" class="media-host karaoke-media-host"></div>
            <div class="karaoke-transport">
              <div class="karaoke-clock"><span>当前时间</span><strong id="karaoke-current-time">00:00:00.000</strong></div>
              <div class="karaoke-transport-buttons">
                <button id="karaoke-step-back-small" type="button" class="secondary" title="后退 10 ms（Shift+←）">−10 ms</button>
                <button id="karaoke-step-back" type="button" class="secondary" title="后退 100 ms（←）">−100 ms</button>
                <button id="karaoke-toggle-playback" type="button">播放</button>
                <button id="karaoke-step-forward" type="button" class="secondary" title="前进 100 ms（→）">+100 ms</button>
                <button id="karaoke-step-forward-small" type="button" class="secondary" title="前进 10 ms（Shift+→）">+10 ms</button>
              </div>
              <div class="karaoke-transport-options">
                <label>速度<select id="karaoke-playback-rate"><option value="0.5">0.5×</option><option value="0.75">0.75×</option><option value="1" selected>1×</option><option value="1.25">1.25×</option><option value="1.5">1.5×</option><option value="2">2×</option></select></label>
                <label>时间缩放<input id="karaoke-zoom" type="range" min="0" max="100" value="66" aria-label="时间轴缩放"><output id="karaoke-zoom-label">30 秒</output></label>
                <label>波形强度<input id="karaoke-waveform-gain" type="range" min="1" max="8" step="0.25" value="1" aria-label="波形显示强度"><output id="karaoke-waveform-gain-label">1×</output></label>
              </div>
              <p class="shortcut-help">空格/K 播放暂停 · ←/→ 100 ms · Shift+←/→ 10 ms · ⌘B 切开 · 触控板横向平移／捏合缩放</p>
              <p id="karaoke-media-message" class="media-message">正在打开当前任务的媒体。</p>
            </div>
          </div>
          <section class="waveform-panel" aria-labelledby="waveform-heading">
            <div class="panel-heading compact">
              <div><p class="eyebrow">WAVEFORM</p><h3 id="waveform-heading">声音与字幕时间轴</h3></div>
              <div class="waveform-navigation"><button id="karaoke-window-back" type="button" class="secondary">← 前一屏</button><button id="karaoke-follow-playhead" type="button" class="secondary active">跟随播放头</button><button id="karaoke-window-forward" type="button" class="secondary">后一屏 →</button><button id="undo-subtitle-structure" type="button" class="secondary" disabled>撤销上次打轴</button></div>
            </div>
            <div id="karaoke-waveform-status" class="waveform-status">选择任务后生成本地波形缓存。拖动字幕块可移动，拖动左右边缘可单独修剪；空白合法，同轨不能重叠。</div>
            <canvas id="karaoke-waveform" class="karaoke-waveform" width="1200" height="260" aria-label="可点击定位的声音波形与字幕时间轴"></canvas>
          </section>
          <section class="karaoke-current-segment" aria-live="polite">
            <div class="karaoke-text-editor"><span id="karaoke-segment-time">当前没有字幕</span><label>原文<textarea id="karaoke-current-source" rows="3" placeholder="当前时间没有原文字幕" disabled></textarea></label><label>译文<textarea id="karaoke-current-translation" rows="3" placeholder="尚无译文" disabled></textarea></label></div>
            <div class="karaoke-segment-actions"><button id="karaoke-save-text" type="button" disabled>保存文字</button><button id="karaoke-discard-text" type="button" class="secondary" disabled>放弃文字修改</button><button id="karaoke-undo-text" type="button" class="secondary" disabled>撤销上次文字保存</button><button id="karaoke-cut-segment" type="button" class="secondary" disabled>在播放头切开 ⌘B</button><button id="karaoke-join-segment" type="button" class="secondary" disabled>连接下一块</button><p id="karaoke-timing-message">拖动按 10 ms 网格吸附；默认不会移动相邻字幕，修改会原子写入 SQLite。</p></div>
          </section>
        </section>
      </div>
    </section>
    <section id="workspace-view" class="workspace hidden" aria-labelledby="workspace-title">
      <div class="workspace-heading">
        <button id="back-to-jobs" class="secondary" type="button">← 返回任务</button>
        <div>
          <p class="eyebrow">TASK WORKSPACE</p>
          <h2 id="workspace-title">任务详情</h2>
          <p id="workspace-message" class="workspace-message"></p>
        </div>
        <button id="reload-detail" type="button" class="secondary">重新读取</button>
      </div>
      <nav class="workspace-sections" role="tablist" aria-label="任务详情功能">
        <button id="workspace-tab-translation" class="workspace-section-tab active" type="button" role="tab" aria-selected="true" aria-controls="workspace-translation-panel" data-workspace-section="translation">
          <strong>翻译与词表</strong><span>批量翻译与识别修正</span>
        </button>
        <button id="workspace-tab-review" class="workspace-section-tab" type="button" role="tab" aria-selected="false" aria-controls="workspace-review-panel" data-workspace-section="review">
          <strong>字幕校对</strong><span>播放与文字粗修</span>
        </button>
        <button id="workspace-tab-export" class="workspace-section-tab" type="button" role="tab" aria-selected="false" aria-controls="workspace-export-panel" data-workspace-section="export">
          <strong>导出成品</strong><span>字幕文件与烧录视频</span>
        </button>
      </nav>
      <p id="workspace-action-message" class="workspace-action-message" role="status"></p>
      <section id="workspace-review-panel" class="workspace-panel hidden" role="tabpanel" aria-labelledby="workspace-tab-review" data-workspace-panel="review">
        <div class="module-heading compact">
          <div><p class="eyebrow">SUBTITLE REVIEW</p><h2>字幕校对</h2><p>逐段修正原文和译文；需要精确打轴、Cut 或 Join 时进入高级编辑。</p></div>
          <button id="open-subtitle-editor" type="button">进入字幕编辑 →</button>
        </div>
        <div class="review-grid">
          <section class="media-panel" aria-label="媒体播放器">
            <div class="panel-heading compact"><div><p class="eyebrow">PLAYBACK</p><h3>媒体与当前字幕</h3></div><button id="relink-job-media" type="button" class="secondary hidden">重新定位原媒体</button></div>
          <div id="media-host" class="media-host"></div>
          <div class="playback-tools" aria-label="播放快捷操作">
            <button id="previous-subtitle" type="button" class="secondary" title="上一句（[）">上一句</button>
            <button id="rewind-media" type="button" class="secondary" title="后退 5 秒（← 或 J）">−5 秒</button>
            <button id="toggle-playback" type="button" title="播放／暂停（空格或 K）">播放</button>
            <button id="forward-media" type="button" class="secondary" title="前进 5 秒（→ 或 L）">+5 秒</button>
            <button id="next-subtitle" type="button" class="secondary" title="下一句（]）">下一句</button>
            <label>速度
              <select id="playback-rate" aria-label="播放速度">
                <option value="0.5">0.5×</option>
                <option value="0.75">0.75×</option>
                <option value="1" selected>1×</option>
                <option value="1.25">1.25×</option>
                <option value="1.5">1.5×</option>
                <option value="2">2×</option>
              </select>
            </label>
          </div>
          <p class="shortcut-help">快捷键：空格/K 播放暂停 · ←/J 与 →/L 跳 5 秒 · [ ] 切换字幕 · , . 调整速度 · O 开关悬浮字幕</p>
          <p id="media-message" class="media-message"></p>
          <div class="current-caption" aria-live="polite">
            <p id="current-source">播放时将在这里显示当前原文。</p>
            <p id="current-translation">简体中文翻译</p>
          </div>
          </section>
          <section class="timeline" aria-labelledby="timeline-title">
            <div class="section-heading">
              <h2 id="timeline-title">字幕粗修</h2>
              <span id="segment-count"></span>
            </div>
            <div id="subtitle-list" class="subtitle-list"></div>
          </section>
        </div>
      </section>
      <section id="workspace-translation-panel" class="workspace-panel" role="tabpanel" aria-labelledby="workspace-tab-translation" data-workspace-panel="translation">
        <div class="module-heading">
          <div><p class="eyebrow">LANGUAGE WORKFLOW</p><h2>翻译与识别修正</h2><p>这里的操作会更新当前 SQLite 字幕工作区；原始识别快照保持不变。</p></div>
        </div>
        <section class="workspace-toolbar module-card" aria-label="翻译与词表">
          <div>
            <strong>简体中文翻译</strong>
            <span id="translation-status">正在读取翻译配置…</span>
            <span id="translation-run-status" class="muted"></span>
          </div>
          <div class="workspace-action-buttons">
            <button id="translate-all" type="button">全部翻译／重译</button>
          </div>
          <div class="workspace-glossary-row">
            <div>
              <strong>识别词表修正</strong>
              <span id="job-glossary-status">当前任务未记录识别词表</span>
            </div>
            <select id="workspace-glossary"><option value="">选择词表…</option></select>
            <button id="preview-glossary" type="button" class="secondary">预览应用</button>
            <button id="manage-workspace-glossary" type="button" class="secondary">管理完整词表</button>
          </div>
          <div class="workspace-glossary-inspection">
            <section>
              <strong>可应用的修正映射</strong>
              <p>这里只列出会修改当前原文的规则；纯提示词不会改写已有字幕。</p>
              <div id="workspace-glossary-mappings" class="glossary-mapping-list muted">请选择词表。</div>
            </section>
            <details>
              <summary>查看转写时冻结的词表快照</summary>
              <p>该快照只用于说明本任务当时的识别输入，不会随当前词表修改而变化。</p>
              <pre id="job-glossary-snapshot">当前任务没有词表快照。</pre>
            </details>
          </div>
          <div id="glossary-preview" class="glossary-preview hidden"></div>
        </section>
      </section>
      <section id="workspace-export-panel" class="workspace-panel hidden" role="tabpanel" aria-labelledby="workspace-tab-export" data-workspace-panel="export">
        <div class="module-heading">
          <div><p class="eyebrow">DELIVERABLES</p><h2>导出成品</h2><p>每次导出都读取当前已保存的字幕；未保存草稿不会进入文件或视频。</p></div>
        </div>
        <div class="export-grid">
          <article class="export-card">
            <div><span class="module-number">01</span><h3>字幕文件</h3><p>按需要选择原文、译文或双语 SRT/ASS；只导出原文时不要求先完成翻译。</p></div>
            <div class="export-card-actions"><button id="export-subtitles" type="button">导出字幕…</button><button id="reveal-export" type="button" class="secondary hidden">在 ${fileManagerLabel} 中显示</button></div>
          </article>
          <article class="export-card">
            <div><span class="module-number">02</span><h3>带字幕视频</h3><p>把当前字幕冻结为一次独立烧录任务；提交后仍可返回字幕校对继续工作。</p></div>
            <div class="export-card-actions"><button id="render-video" type="button">导出带字幕视频…</button></div>
          </article>
        </div>
      </section>
    </section>
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
    <dialog id="subtitle-export-dialog" class="rename-job-dialog subtitle-export-dialog">
      <form id="subtitle-export-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">SUBTITLE FILES</p><h2>选择字幕文件</h2></div>
          <button id="cancel-subtitle-export" type="button" class="secondary">取消</button>
        </div>
        <p>原文 SRT 可以独立导出；含译文的文件要求全部译文已完成且没有过期。</p>
        <div class="subtitle-export-options">
          <label><input type="checkbox" name="subtitle-artifact" value="source_srt" checked />原文 SRT</label>
          <label><input type="checkbox" name="subtitle-artifact" value="translated_srt" checked />译文 SRT</label>
          <label><input type="checkbox" name="subtitle-artifact" value="bilingual_srt" checked />双语 SRT</label>
          <label><input type="checkbox" name="subtitle-artifact" value="bilingual_ass" checked />双语 ASS</label>
        </div>
        <div class="rename-job-footer"><span id="subtitle-export-message" role="status"></span><button type="submit">选择目录</button></div>
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
    <dialog id="video-render-dialog" class="video-render-dialog">
      <div class="dialog-heading">
        <div><p class="eyebrow">BURN SUBTITLES</p><h2>导出带字幕视频</h2></div>
        <button id="close-video-render" type="button" class="secondary">关闭</button>
      </div>
      <div id="media-capabilities" class="media-capabilities">正在检查内置 FFmpeg sidecar…</div>
      <div class="video-render-form">
        <label>字幕内容
          <select id="video-subtitle-track">
            <option value="bilingual">原文＋译文（推荐）</option>
            <option value="translation">仅译文</option>
            <option value="source">仅原文</option>
          </select>
        </label>
        <label>输出视频
          <input id="video-output-path" placeholder="选择 MP4 保存位置，或粘贴完整路径" />
        </label>
        <button id="choose-video-output" type="button" class="secondary">选择位置</button>
        <p>${desktopPlatform === "macos" ? "优先使用 VideoToolbox，失败时回退" : "使用"}内置 LGPL MPEG-4 软件编码。提交时会把 SQLite 当前字幕冻结为本次 ASS 快照；可读取源视频码率时，成品会以源码率加约 20% 余量编码。</p>
      </div>
      <div class="video-render-footer">
        <span id="video-render-message" role="status"></span>
        <button id="submit-video-render" type="button">开始烧录</button>
      </div>
      <section class="video-render-history" aria-labelledby="video-render-history-title">
        <div class="section-heading"><h2 id="video-render-history-title">本任务烧录记录</h2><span id="video-render-count"></span></div>
        <div id="video-render-list" class="video-render-list"></div>
      </section>
    </dialog>
    <dialog id="settings-dialog" class="settings-dialog">
      <form id="settings-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">FIRST RUN & SETTINGS</p><h2>启动配置</h2></div>
          <button id="close-settings" type="button" class="secondary">稍后再说</button>
        </div>
        <p class="dialog-help">媒体和模型保留在本机；只有启用翻译 provider 后，原文字幕才会发送到对应云端。</p>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>1. 本地识别模型</strong><span id="models-directory"></span></div><span id="model-readiness"></span></div>
          <div class="settings-path-row">
            <label>Whisper 模型<input id="settings-whisper-model" placeholder="选择已有 ggml-*.bin，或从下方下载" /></label>
            <button id="settings-choose-whisper" type="button" class="secondary">选择文件</button>
          </div>
          <div class="settings-path-row">
            <label>Silero VAD 模型（推荐）<input id="settings-vad-model" placeholder="选择已有 ggml-silero-*.bin，或从下方下载" /></label>
            <button id="settings-choose-vad" type="button" class="secondary">选择文件</button>
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
          <div class="network-test-row"><button id="test-network" type="button" class="secondary">测试当前网络配置</button><span>下载会自动保存当前网络配置；官方源受阻时可先尝试填写 https://hf-mirror.com。</span></div>
          <div id="network-test-results" class="network-test-results" role="status"></div>
          <div id="model-catalog" class="model-catalog"></div>
          <p id="model-download-message" class="settings-message" role="status"></p>
        </section>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>3. 云端翻译（可选）</strong><span>不配置也可以完成本地转写、编辑和原文字幕导出。</span></div><span id="credential-store-label"></span></div>
          <label>翻译 provider
            <select id="settings-provider">
              <option value="deepl">DeepL（传统翻译 API）</option>
              <option value="deepseek">DeepSeek（LLM API）</option>
              <option value="openai-compatible">OpenAI-compatible（高级）</option>
              <option value="none">关闭云端翻译</option>
            </select>
          </label>
          <div id="llm-provider-fields" class="settings-grid hidden">
            <label id="provider-base-url-field">OpenAI-compatible Base URL
              <input id="settings-provider-base-url" type="url" placeholder="https://api.openai.com/v1" />
            </label>
            <label>模型
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
          <div class="settings-section-heading"><div><strong>4. 学习词典</strong><span>离线包由你明确点击后下载到正式应用数据目录；商业词典的 Key 分来源保存。</span></div><span id="dictionary-directory"></span></div>
          <div>
            <strong class="settings-subheading">离线词典包</strong>
            <p class="settings-message">JMdict 与 Tomoshi 用于日语；FreeDict 英中是免费离线补充，不等同于 Cambridge／Collins 的学习词典质量。下载会复用上方代理设置并校验发布方摘要。</p>
          </div>
          <div id="dictionary-catalog" class="model-catalog"></div>
          <p id="dictionary-download-message" class="settings-message" role="status"></p>
          <div>
            <strong class="settings-subheading">在线词典 API</strong>
            <p class="settings-message">每个来源独立配置；Key 只写入系统凭据库，不写入 SQLite、日志或任务目录。检查只验证凭据是否存在，不调用词典 API。</p>
          </div>
          <div id="dictionary-credentials" class="dictionary-credentials">
            ${[
              ["cambridge", "Cambridge Dictionary", "适合英中学习释义；需从 Cambridge Dictionary API 申请。"],
              ["collins", "Collins Dictionary", "可提供英中结果；额度与展示要求以账户协议为准。"],
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
const workspaceView = document.querySelector<HTMLElement>("#workspace-view");
const workspaceSectionTabs = Array.from(document.querySelectorAll<HTMLButtonElement>("[data-workspace-section]"));
const workspaceSectionPanels = Array.from(document.querySelectorAll<HTMLElement>("[data-workspace-panel]"));
const openSubtitleEditorButton = document.querySelector<HTMLButtonElement>("#open-subtitle-editor");
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
const karaokeFollowPlayheadButton = document.querySelector<HTMLButtonElement>("#karaoke-follow-playhead");
const karaokeWaveformStatus = document.querySelector<HTMLDivElement>("#karaoke-waveform-status");
const karaokeWaveform = document.querySelector<HTMLCanvasElement>("#karaoke-waveform");
const karaokeSegmentTime = document.querySelector<HTMLSpanElement>("#karaoke-segment-time");
const karaokeCurrentSource = document.querySelector<HTMLTextAreaElement>("#karaoke-current-source");
const karaokeCurrentTranslation = document.querySelector<HTMLTextAreaElement>("#karaoke-current-translation");
const karaokeSaveTextButton = document.querySelector<HTMLButtonElement>("#karaoke-save-text");
const karaokeDiscardTextButton = document.querySelector<HTMLButtonElement>("#karaoke-discard-text");
const karaokeUndoTextButton = document.querySelector<HTMLButtonElement>("#karaoke-undo-text");
const karaokeCutSegmentButton = document.querySelector<HTMLButtonElement>("#karaoke-cut-segment");
const karaokeJoinSegmentButton = document.querySelector<HTMLButtonElement>("#karaoke-join-segment");
const karaokeTimingMessage = document.querySelector<HTMLParagraphElement>("#karaoke-timing-message");
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
const workspaceTitle = document.querySelector<HTMLHeadingElement>("#workspace-title");
const workspaceMessage = document.querySelector<HTMLParagraphElement>("#workspace-message");
const mediaHost = document.querySelector<HTMLDivElement>("#media-host");
const mediaMessage = document.querySelector<HTMLParagraphElement>("#media-message");
const relinkJobMediaButton = document.querySelector<HTMLButtonElement>("#relink-job-media");
const previousSubtitleButton = document.querySelector<HTMLButtonElement>("#previous-subtitle");
const rewindMediaButton = document.querySelector<HTMLButtonElement>("#rewind-media");
const togglePlaybackButton = document.querySelector<HTMLButtonElement>("#toggle-playback");
const forwardMediaButton = document.querySelector<HTMLButtonElement>("#forward-media");
const nextSubtitleButton = document.querySelector<HTMLButtonElement>("#next-subtitle");
const playbackRateSelect = document.querySelector<HTMLSelectElement>("#playback-rate");
const subtitleList = document.querySelector<HTMLDivElement>("#subtitle-list");
const segmentCount = document.querySelector<HTMLSpanElement>("#segment-count");
const currentSource = document.querySelector<HTMLParagraphElement>("#current-source");
const currentTranslation = document.querySelector<HTMLParagraphElement>("#current-translation");
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
const learningDictionaryDialog = document.querySelector<HTMLDialogElement>("#learning-dictionary-dialog");
const learningDictionaryTitle = document.querySelector<HTMLHeadingElement>("#learning-dictionary-title");
const learningDictionarySubtitle = document.querySelector<HTMLParagraphElement>("#learning-dictionary-subtitle");
const learningProviderTabs = document.querySelector<HTMLElement>("#learning-provider-tabs");
const learningProviderPanel = document.querySelector<HTMLElement>("#learning-provider-panel");
const translationStatusText = document.querySelector<HTMLSpanElement>("#translation-status");
const translationRunStatusText = document.querySelector<HTMLSpanElement>("#translation-run-status");
const translateAllButton = document.querySelector<HTMLButtonElement>("#translate-all");
const openSubtitleOverlayButton = document.querySelector<HTMLButtonElement>("#open-subtitle-overlay");
const exportButton = document.querySelector<HTMLButtonElement>("#export-subtitles");
const renderVideoButton = document.querySelector<HTMLButtonElement>("#render-video");
const revealExportButton = document.querySelector<HTMLButtonElement>("#reveal-export");
const subtitleExportDialog = document.querySelector<HTMLDialogElement>("#subtitle-export-dialog");
const subtitleExportForm = document.querySelector<HTMLFormElement>("#subtitle-export-form");
const subtitleExportMessage = document.querySelector<HTMLSpanElement>("#subtitle-export-message");
const workspaceActionMessage = document.querySelector<HTMLParagraphElement>("#workspace-action-message");
const workspaceGlossary = document.querySelector<HTMLSelectElement>("#workspace-glossary");
const jobGlossaryStatus = document.querySelector<HTMLSpanElement>("#job-glossary-status");
const workspaceGlossaryMappings = document.querySelector<HTMLDivElement>("#workspace-glossary-mappings");
const jobGlossarySnapshot = document.querySelector<HTMLElement>("#job-glossary-snapshot");
const glossaryPreviewHost = document.querySelector<HTMLDivElement>("#glossary-preview");
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
const videoRenderDialog = document.querySelector<HTMLDialogElement>("#video-render-dialog");
const mediaCapabilitiesHost = document.querySelector<HTMLDivElement>("#media-capabilities");
const videoSubtitleTrack = document.querySelector<HTMLSelectElement>("#video-subtitle-track");
const videoOutputPath = document.querySelector<HTMLInputElement>("#video-output-path");
const videoRenderMessage = document.querySelector<HTMLSpanElement>("#video-render-message");
const submitVideoRenderButton = document.querySelector<HTMLButtonElement>("#submit-video-render");
const videoRenderList = document.querySelector<HTMLDivElement>("#video-render-list");
const videoRenderCount = document.querySelector<HTMLSpanElement>("#video-render-count");
const settingsDialog = document.querySelector<HTMLDialogElement>("#settings-dialog");
const settingsForm = document.querySelector<HTMLFormElement>("#settings-form");
const settingsWhisperModel = document.querySelector<HTMLInputElement>("#settings-whisper-model");
const settingsVadModel = document.querySelector<HTMLInputElement>("#settings-vad-model");
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
const modelCatalogHost = document.querySelector<HTMLDivElement>("#model-catalog");
const modelDownloadMessage = document.querySelector<HTMLParagraphElement>("#model-download-message");
const modelsDirectory = document.querySelector<HTMLSpanElement>("#models-directory");
const modelReadiness = document.querySelector<HTMLSpanElement>("#model-readiness");
const dictionaryCatalogHost = document.querySelector<HTMLDivElement>("#dictionary-catalog");
const dictionaryDirectoryLabel = document.querySelector<HTMLSpanElement>("#dictionary-directory");
const dictionaryDownloadMessage = document.querySelector<HTMLParagraphElement>("#dictionary-download-message");
const dictionaryCredentialMessage = document.querySelector<HTMLParagraphElement>("#dictionary-credential-message");

let refreshing = false;
let latestJobs: LocalJob[] = [];
let currentArea: TopLevelArea = "workbench";
let renderedJobsFingerprint: string | null = null;
let activeDetail: JobDetail | null = null;
let activeMedia: HTMLMediaElement | null = null;
let mediaSessionId = 0;
let navigationRequestId = 0;
let activeSegmentId: string | null = null;
let subtitleOverlayVisible = false;
let lastSubtitleOverlayKey = "";
const subtitleUndoHistory = new Map<string, SubtitleUndoEntry[]>();
const subtitleStructureUndoHistory = new Map<string, SubtitleStructureUndoEntry[]>();
let workspaceActionBusy = false;
let lastExportedSubtitlePath: string | null = null;
let glossaries: Glossary[] = [];
let editingGlossaryId: string | null = null;
let pendingGlossaryPreview: GlossaryPreview | null = null;
let renamingJob: LocalJob | null = null;
let confirmationResolver: ((confirmed: boolean) => void) | null = null;
let subtitleExportResolver: ((artifacts: SubtitleExportArtifact[] | null) => void) | null = null;
let pendingGlossaryCorrection: {
  glossaryId: string;
  state: HTMLSpanElement;
} | null = null;
let taskGlossaryConfigurationId: string | null = null;
let selectedTaskContentGroups = new Set<string>();
let mediaCapabilities: MediaCapabilities | null = null;
let videoRenders: VideoRender[] = [];
let selectedVideoOutputAlreadyExists = false;
let videoRenderSubmitting = false;
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
let pendingLearningSelection: PendingLearningSelection | null = null;
let activeLearningItemId: string | null = null;
let activeLearningProviderId = "summary";
let learningActionBusy = false;
let learningLookupBusy = false;
let workspaceElapsedTimer: number | null = null;
let playbackPositionSaveTimer: number | null = null;
let lastPlaybackPositionSavedAt = 0;
let karaokeWaveformWindow: WaveformWindow | null = null;
let karaokeWaveformRequestId = 0;
let karaokeViewStartMs = 0;
let karaokeFollowPlayhead = true;
let karaokeWaveformLoading = false;
let karaokeAnimationFrame: number | null = null;
let karaokeSelectedJobId: string | null = null;
let karaokeResumeMs = 0;
let karaokeTimingDrag: KaraokeTimingDrag | null = null;
let karaokePanDrag: KaraokePanDrag | null = null;
let karaokeSuppressClick = false;
let karaokeWaveformGain = 1;
let karaokeWaveformReloadTimer: number | null = null;
let karaokePendingWaveformStartMs = 0;
let karaokeGestureStart: { durationMs: number; anchorMs: number; ratio: number } | null = null;
let karaokeTextDraftSegmentId: string | null = null;
let karaokeTextDraftDirty = false;
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

function jobTimingLabel(job: Pick<LocalJob, "status" | "created_at_unix" | "started_at_unix" | "completed_at_unix">): string {
  const now = Math.floor(Date.now() / 1_000);
  if (job.status === "queued") return `排队 ${formatElapsed(now - job.created_at_unix)}`;
  if (matchesTerminalStatus(job.status)) {
    if (job.started_at_unix !== null && job.completed_at_unix !== null && job.completed_at_unix >= job.started_at_unix) {
      return `处理用时 ${formatElapsed(job.completed_at_unix - job.started_at_unix)}`;
    }
    return "历史任务未记录处理用时";
  }
  if (job.started_at_unix !== null) return `已运行 ${formatElapsed(now - job.started_at_unix)}`;
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
      started_at_unix: startedAt,
      completed_at_unix: completedAt,
      status,
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
  const enabled = provider !== "none";
  const llmEnabled = provider === "deepseek" || provider === "openai-compatible";
  const customEndpoint = provider === "openai-compatible";
  document.querySelector<HTMLElement>("#api-key-field")?.classList.toggle("hidden", !enabled);
  settingsClearApiKey?.closest("label")?.classList.toggle("hidden", !enabled);
  checkApiKeyButton?.closest("div")?.classList.toggle("hidden", !enabled);
  document.querySelector<HTMLElement>("#llm-provider-fields")?.classList.toggle("hidden", !llmEnabled);
  document.querySelector<HTMLElement>("#provider-base-url-field")?.classList.toggle("hidden", !customEndpoint);
  const providerLabel = provider === "deepseek" ? "DeepSeek" : provider === "openai-compatible" ? "OpenAI-compatible" : "DeepL";
  const apiKeyLabel = document.querySelector<HTMLSpanElement>("#api-key-label");
  const clearApiKeyLabel = document.querySelector<HTMLSpanElement>("#clear-api-key-label");
  if (apiKeyLabel) apiKeyLabel.textContent = `${providerLabel} API Key`;
  if (clearApiKeyLabel) clearApiKeyLabel.textContent = `删除系统凭据库中已保存的 ${providerLabel} Key`;
}

async function checkSelectedApiKey(): Promise<void> {
  if (!settingsProvider || !checkApiKeyButton || !apiKeyStatus || settingsProvider.value === "none") return;
  checkApiKeyButton.disabled = true;
  apiKeyStatus.textContent = `正在检查 ${settingsProvider.value} 的系统凭据；macOS 可能请求解锁 Keychain…`;
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
  overwriteSettingsField(settingsWhisperModel, settings.whisperModelPath ?? "");
  overwriteSettingsField(settingsVadModel, settings.vadModelPath ?? "");
  overwriteSettingsField(settingsProxyMode, settings.networkProxyMode);
  overwriteSettingsField(settingsProxyUrl, settings.networkProxyUrl ?? "");
  overwriteSettingsField(settingsModelMirror, settings.modelMirrorUrl ?? "");
  overwriteSettingsField(settingsProvider, settings.translationProviderId);
  overwriteSettingsField(settingsProviderModel, settings.translationModel ?? "");
  overwriteSettingsField(settingsProviderBaseUrl, settings.translationBaseUrl ?? "");
  overwriteSettingsField(settingsTranslationStyle, settings.translationStyleInstruction);
  overwriteSettingsField(settingsApiKey, "");
  overwriteSettingsField(settingsClearApiKey, false);
  if (modelsDirectory) modelsDirectory.textContent = `下载目录：${settings.modelsDirectory}`;
  if (modelReadiness) {
    modelReadiness.textContent = settings.whisperModelReady
      ? settings.vadModelReady ? "Whisper 与 VAD 已就绪" : "Whisper 已就绪 · VAD 可选"
      : "需要配置 Whisper 模型";
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
        : "尚未配置；Key 不会写入 SQLite 或任务目录";
    apiKeyStatus.textContent = settings.credentialError
      ? `${source}。系统凭据库提示：${settings.credentialError}`
      : source;
    apiKeyStatus.classList.toggle("warning", Boolean(settings.credentialError));
  }
  if (modelPath && settings.whisperModelPath) modelPath.value = settings.whisperModelPath;
  if (vadModelPath && settings.vadModelPath) vadModelPath.value = settings.vadModelPath;
  syncProviderSettings();
  syncNetworkSettings();
  renderModelCatalog();
}

function renderModelCatalog(): void {
  if (!modelCatalogHost) return;
  const activeDownload = modelDownloads.find((download) =>
    download.status === "queued" || download.status === "downloading"
  );
  modelCatalogHost.innerHTML = availableModels.map((model) => {
    const download = modelDownloads.find((item) => item.modelId === model.id);
    const progress = download?.totalBytes
      ? Math.min(1, download.downloadedBytes / download.totalBytes)
      : null;
    const source = download?.source ? ` · ${download.source}` : "";
    const state = download?.status === "done"
      ? `已下载并设为默认${source}`
      : download?.status === "failed"
        ? `失败：${download.error ?? "未知错误"}${source}`
        : download?.status === "downloading"
          ? `${formatBytes(download.downloadedBytes)}${download.totalBytes ? ` / ${formatBytes(download.totalBytes)}` : ""}${source}`
          : download?.status === "queued" ? `等待下载${source}` : "";
    return `<article class="model-card">
      <div><strong>${escapeHtml(model.name)}</strong><span>${escapeHtml(model.sizeLabel)} · ${escapeHtml(model.recommendedFor)}</span></div>
      ${progress === null ? "" : `<progress max="1" value="${progress}"></progress>`}
      <div class="model-card-action"><span class="${download?.status === "failed" ? "warning" : ""}">${escapeHtml(state)}</span><button type="button" class="secondary" data-download-model="${escapeHtml(model.id)}" ${activeDownload ? "disabled" : ""}>${download?.status === "failed" ? "重试下载" : "下载"}</button></div>
    </article>`;
  }).join("");
  modelCatalogHost.querySelectorAll<HTMLButtonElement>("[data-download-model]").forEach((button) => {
    button.addEventListener("click", () => void startModelDownload(button.dataset.downloadModel ?? ""));
  });
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
  if (dictionaryCredentialMessage) dictionaryCredentialMessage.textContent = "正在检查系统凭据；macOS 可能请求解锁 Keychain…";
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
    const [settings, catalog, downloads] = await Promise.all([
      invoke<DesktopSettings>("desktop_settings"),
      availableModels.length ? Promise.resolve(availableModels) : invoke<ModelCatalogItem[]>("model_catalog"),
      invoke<ModelDownloadState[]>("model_download_states"),
    ]);
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
  try {
    translationStatus = await invoke<TranslationStatus>("translation_status");
    updateTranslationControls();
  } catch (error) {
    setWorkspaceAction(`无法读取翻译配置：${String(error)}`, true);
  }
}

async function saveSettings(finishOnboarding: boolean): Promise<void> {
  if (!settingsWhisperModel || !settingsVadModel || !settingsProvider || !settingsApiKey || !settingsProxyMode || !settingsProxyUrl || !settingsModelMirror || !settingsProviderModel || !settingsProviderBaseUrl || !settingsTranslationStyle) return;
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

async function chooseSettingsModel(kind: "whisper" | "vad"): Promise<void> {
  const command = kind === "whisper" ? "pick_model_file" : "pick_vad_model_file";
  try {
    const path = await invoke<string | null>(command);
    const input = kind === "whisper" ? settingsWhisperModel : settingsVadModel;
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
      download.status === "queued" || download.status === "downloading"
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
      const tokens = latest.input_tokens !== null || latest.output_tokens !== null
        ? ` · Token ${latest.input_tokens ?? "?"} 入 / ${latest.output_tokens ?? "?"} 出`
        : "";
      translationRunStatusText.textContent = `最近批次：${latest.provider_name}${model} · ${latest.segment_count} 段${tokens} · ${new Date(latest.completed_at_unix * 1_000).toLocaleString()}`;
    }
  }
  if (translateAllButton) {
    translateAllButton.disabled = workspaceActionBusy || !hasSegments || !translationStatus.configured;
  }
  if (exportButton) exportButton.disabled = workspaceActionBusy || !hasSegments;
  if (renderVideoButton) {
    const inputPath = activeDetail?.job.input_path;
    renderVideoButton.disabled =
      workspaceActionBusy || !hasSegments || !inputPath || !activeDetail?.playback_path || isAudioPath(inputPath);
    renderVideoButton.title = inputPath && isAudioPath(inputPath)
      ? "音频任务不能烧录视频"
      : inputPath && !activeDetail?.playback_path
        ? "原视频已移动，请先在字幕校对中重新定位"
        : "";
  }
  if (revealExportButton) {
    revealExportButton.disabled = workspaceActionBusy || !lastExportedSubtitlePath;
  }
  const previewButton = document.querySelector<HTMLButtonElement>("#preview-glossary");
  if (previewButton) {
    previewButton.disabled = workspaceActionBusy || !hasSegments || !workspaceGlossary?.value;
  }
  subtitleList?.querySelectorAll<HTMLButtonElement>(".translate-segment").forEach((button) => {
    button.disabled = workspaceActionBusy || !translationStatus.configured;
  });
  subtitleList?.querySelectorAll<HTMLButtonElement>(".capture-glossary-term").forEach((button) => {
    button.disabled = workspaceActionBusy || !workspaceGlossary?.value;
  });
}

function setWorkspaceAction(message: string, isError = false): void {
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

function startWorkspaceElapsed(message: string): () => void {
  const startedAt = Date.now();
  const update = () => setWorkspaceAction(`${message} · 已运行 ${formatElapsed((Date.now() - startedAt) / 1_000)}`);
  if (workspaceElapsedTimer !== null) window.clearInterval(workspaceElapsedTimer);
  update();
  workspaceElapsedTimer = window.setInterval(update, 1_000);
  return () => {
    if (workspaceElapsedTimer !== null) window.clearInterval(workspaceElapsedTimer);
    workspaceElapsedTimer = null;
  };
}

function setWorkspaceBusy(busy: boolean): void {
  workspaceActionBusy = busy;
  if (relinkJobMediaButton) relinkJobMediaButton.disabled = busy;
  updateTranslationControls();
}

function hasUnsavedSubtitleEdits(): boolean {
  return karaokeTextDraftDirty || Boolean(subtitleList?.querySelector<HTMLElement>('[data-dirty="true"]'));
}

function renderJobs(jobs: LocalJob[]): void {
  if (!jobList || !jobCount) return;
  jobCount.textContent = `${jobs.length} 个任务`;
  if (jobs.length === 0) {
    jobList.innerHTML = `<div class="empty-state"><strong>还没有本地任务。</strong><span>选择媒体与模型，创建第一个离线转写任务。</span></div>`;
    return;
  }

  jobList.innerHTML = jobs
    .map(
      (job) => `
        <article class="job-card">
          <button class="job-open" data-job-id="${escapeHtml(job.job_id)}" type="button">
            <div><h3>${escapeHtml(displayName(job))}</h3><p>${escapeHtml(job.message)} · <span data-job-timing data-created-at="${job.created_at_unix}" data-started-at="${job.started_at_unix ?? ""}" data-completed-at="${job.completed_at_unix ?? ""}" data-status="${escapeHtml(job.status)}">${escapeHtml(jobTimingLabel(job))}</span></p>${runningJob(job) ? '<progress class="job-progress"></progress>' : ""}</div>
            <span class="job-statuses">
              <span class="status status-${escapeHtml(job.status)}">${escapeHtml(statusLabel(job.status))}</span>
              <span class="status translation-status translation-status-${escapeHtml(job.translation_status)}">${escapeHtml(translationStatusLabel(job))}</span>
            </span>
          </button>
          <div class="job-actions">
            ${job.status === "failed" ? `<button type="button" class="secondary" data-retry-job="${escapeHtml(job.job_id)}">重试</button>` : ""}
            <button type="button" class="secondary" data-rename-job="${escapeHtml(job.job_id)}">重命名</button>
            <button type="button" class="danger" data-delete-job="${escapeHtml(job.job_id)}" ${matchesTerminalStatus(job.status) ? "" : "disabled"} title="${matchesTerminalStatus(job.status) ? "删除任务数据，但保留原始媒体" : "任务结束后才能删除"}">删除</button>
          </div>
        </article>`,
    )
    .join("");
  jobList.querySelectorAll<HTMLButtonElement>("[data-job-id]").forEach((button) => {
    button.addEventListener("click", () => void openJob(button.dataset.jobId ?? ""));
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
    message: `将删除“${displayName(job)}”在 Atogaki 中的音频、字幕、中间产物和 SQLite 记录。原始媒体文件不会被删除。`,
    confirmLabel: "删除任务",
    danger: true,
  });
  if (!confirmed) return;
  if (jobManagementMessage) jobManagementMessage.textContent = "正在删除任务数据…";
  try {
    await invoke("delete_job", { jobId: job.job_id });
    if (jobManagementMessage) jobManagementMessage.textContent = "任务已删除，原始媒体文件未改变。";
    await refresh();
  } catch (error) {
    if (jobManagementMessage) jobManagementMessage.textContent = `删除失败：${String(error)}`;
  }
}

async function refresh(): Promise<void> {
  if (!jobList || refreshing) return;
  refreshing = true;
  if (!activeDetail && renderedJobsFingerprint === null && jobList.childElementCount === 0) {
    jobList.innerHTML = `<div class="empty-state">正在读取任务…</div>`;
  }
  try {
    const jobs = await invoke<LocalJob[]>("list_jobs");
    latestJobs = jobs;
    const fingerprint = JSON.stringify(jobs);
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
  workspaceView?.classList.toggle("hidden", !show);
  listeningView?.classList.add("hidden");
  learningView?.classList.add("hidden");
  karaokeView?.classList.add("hidden");
  if (show) currentArea = "workspace";
  document.querySelector<HTMLButtonElement>("#refresh")?.classList.toggle("hidden", show);
}

async function stopActivePlayback(): Promise<void> {
  const media = activeMedia;
  const detail = activeDetail;
  const area = currentArea;
  mediaSessionId += 1;
  karaokeWaveformRequestId += 1;
  if (karaokeAnimationFrame !== null) {
    window.cancelAnimationFrame(karaokeAnimationFrame);
    karaokeAnimationFrame = null;
  }
  karaokeTimingDrag = null;
  karaokePanDrag = null;
  karaokeGestureStart = null;
  karaokeTextDraftSegmentId = null;
  karaokeTextDraftDirty = false;
  if (karaokeWaveformReloadTimer !== null) {
    window.clearTimeout(karaokeWaveformReloadTimer);
    karaokeWaveformReloadTimer = null;
  }
  karaokePendingWaveformStartMs = 0;
  activeMedia = null;
  if (playbackPositionSaveTimer !== null) {
    window.clearTimeout(playbackPositionSaveTimer);
    playbackPositionSaveTimer = null;
  }
  if (media) {
    media.pause();
    if (area === "listening" && detail) {
      await savePlaybackPosition(detail.job.job_id, media);
    } else if (area === "karaoke" && detail) {
      karaokeResumeMs = Math.round(media.currentTime * 1_000);
    }
    media.removeAttribute("src");
    media.load();
    media.remove();
  }
  activeSegmentId = null;
  updatePlaybackControls();
  hideSubtitleOverlay();
}

async function showTopLevelArea(area: Exclude<TopLevelArea, "workspace">): Promise<void> {
  closeLearningSelection();
  const requestId = ++navigationRequestId;
  await stopActivePlayback();
  if (requestId !== navigationRequestId) return;
  activeDetail = null;
  currentArea = area;
  shell?.classList.toggle("workspace-open", area === "karaoke");
  workspaceView?.classList.add("hidden");
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
  if (area === "learning") {
    await Promise.all([refreshLearningItems(), loadDictionarySettings()]);
    if (learningDictionaryDialog?.open) renderLearningDictionary();
  }
  if (area === "karaoke" && karaokeSelectedJobId) {
    await openKaraokeJob(karaokeSelectedJobId);
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
    activeDetail = detail;
    activeSegmentId = null;
    if (listeningJobTitle) listeningJobTitle.textContent = displayName(detail.job);
    renderListeningJobs(latestJobs);
    renderListeningSubtitles(detail.segments);
    const resumeMs = resumeOverrideMs ?? savedResumeMs;
    mountMedia(detail.playback_path, detail.audio_fallback_path, listeningMediaHost, listeningMediaMessage, resumeMs);
    updateActiveSubtitle(resumeMs);
    renderSubtitleOverlayButton();
    if (autoplay && activeMedia) await activeMedia.play();
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
        <p class="listening-source" data-learning-source title="选择词语、短语或语法表达后收藏">${escapeHtml(segment.source_text)}</p>
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
        ? `${learningItems.length} 个学习条目保存在本机。`
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
  if (learningItems.length === 0) {
    learningItemList.innerHTML = '<div class="empty-state"><strong>单词本还是空的。</strong><span>到收听区选择词语、短语或语法表达；也可以直接收藏整句。</span></div>';
    return;
  }
  learningItemList.innerHTML = learningItems.map((detail) => {
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
      <div class="learning-card-actions">
        <button type="button" data-save-learning-meaning="${escapeHtml(detail.item.id)}">保存译义</button>
        <button type="button" class="secondary" data-open-learning-dictionary="${escapeHtml(detail.item.id)}">查看词典</button>
        ${occurrence?.job_id ? `<button type="button" class="secondary" data-play-learning-source="${escapeHtml(detail.item.id)}">播放例句</button>` : ""}
        <button type="button" class="danger" data-delete-learning-item="${escapeHtml(detail.item.id)}">删除</button>
      </div>
    </article>`;
  }).join("");
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

function learningProviderOptions(detail: LearningItemDetail): LearningProviderOption[] {
  const providers: LearningProviderOption[] = [{ id: "summary", name: "简明", kind: "summary" }];
  if (detail.item.item_type === "sentence") return providers;
  if (detail.item.source_language === "en") {
    providers.push(
      { id: "freedict", name: "FreeDict 英中", kind: "offline" },
      { id: "cambridge", name: "Cambridge", kind: "api" },
      { id: "collins", name: "Collins", kind: "api" },
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
    const supported = provider && !["cambridge", "collins"].includes(provider.id);
    const message = dictionaryProviderEmptyMessage(provider);
    learningProviderPanel.innerHTML = `<div class="learning-provider-empty">
      <span>${provider?.kind === "offline" ? "LOCAL DICTIONARY PACK" : "DICTIONARY API"}</span>
      <h3>${escapeHtml(provider?.name ?? activeLearningProviderId)}</h3>
      <p>${escapeHtml(message)}</p>
      ${supported ? `<button type="button" data-lookup-learning-provider="${escapeHtml(provider.id)}" ${learningLookupBusy ? "disabled" : ""}>${learningLookupBusy ? "正在查询…" : "查询这个来源"}</button>` : ""}
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
    const packageId = provider.id === "jmdict" ? "jmdict-en" : provider.id === "tomoshi" ? "tomoshi-open" : "freedict-eng-zho";
    return dictionaryDownloads.some((item) => item.dictionaryId === packageId && item.status === "done") ? "可查询" : "未下载";
  }
  if (["cambridge", "collins"].includes(provider.id)) return "待接入";
  return dictionaryCredentials.some((item) => item.providerId === provider.id && item.configured) ? "可查询" : "未配置";
}

function dictionaryProviderEmptyMessage(provider: LearningProviderOption | undefined): string {
  if (!provider) return "词典来源不可用。";
  if (["cambridge", "collins"].includes(provider.id)) return `${provider.name} 目前只保留独立 API 配置边界，尚未接入正式查询协议。`;
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

async function openKaraokeJob(jobId: string): Promise<void> {
  if (!jobId || !karaokeMediaMessage) return;
  const reopeningSelectedJob = karaokeSelectedJobId === jobId;
  const requestId = ++navigationRequestId;
  await stopActivePlayback();
  if (requestId !== navigationRequestId || currentArea !== "karaoke") return;
  karaokeSelectedJobId = jobId;
  if (!reopeningSelectedJob) karaokeResumeMs = 0;
  activeDetail = null;
  karaokeWaveformWindow = null;
  karaokeTextDraftSegmentId = null;
  karaokeTextDraftDirty = false;
  karaokeViewStartMs = 0;
  karaokeFollowPlayhead = true;
  syncKaraokeFollowButton();
  renderKaraokeTimeline();
  karaokeMediaMessage.textContent = "正在读取字幕编辑任务…";
  if (karaokeWaveformStatus) karaokeWaveformStatus.textContent = "正在准备本地波形…";
  try {
    const detail = await invoke<JobDetail>("get_job_detail", { jobId });
    if (requestId !== navigationRequestId || currentArea !== "karaoke") return;
    activeDetail = detail;
    activeSegmentId = null;
    if (karaokeJobTitle) karaokeJobTitle.textContent = displayName(detail.job);
    if (karaokeOpenWorkspaceButton) karaokeOpenWorkspaceButton.disabled = false;
    mountMedia(detail.playback_path, detail.audio_fallback_path, karaokeMediaHost, karaokeMediaMessage, karaokeResumeMs);
    updateActiveSubtitle(karaokeResumeMs);
    updateStructureUndoButton();
    setSubtitleEditAction("拖动字幕块可整体移动，拖动左右边缘可单独修剪；按 10 ms 吸附，空白合法，同轨不能重叠。");
    await loadKaraokeWaveform(karaokeResumeMs, true);
  } catch (error) {
    if (requestId !== navigationRequestId || currentArea !== "karaoke") return;
    karaokeMediaMessage.textContent = `无法打开字幕编辑任务：${String(error)}`;
    if (karaokeWaveformStatus) karaokeWaveformStatus.textContent = "波形不可用。";
  }
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

function scheduleKaraokeWaveformReload(startMs: number): void {
  karaokePendingWaveformStartMs = startMs;
  if (karaokeWaveformReloadTimer !== null) return;
  karaokeWaveformReloadTimer = window.setTimeout(() => {
    karaokeWaveformReloadTimer = null;
    void loadKaraokeWaveform(karaokePendingWaveformStartMs, true);
  }, 16);
}

async function loadKaraokeWaveform(anchorMs: number, alignStart = false): Promise<void> {
  if (!activeDetail || currentArea !== "karaoke") return;
  const requestId = ++karaokeWaveformRequestId;
  const windowDuration = karaokeWindowDurationMs();
  const knownDuration = karaokeWaveformWindow?.duration_ms ?? Math.round((activeMedia?.duration ?? 0) * 1_000);
  const unclampedStart = alignStart ? anchorMs : anchorMs - windowDuration / 2;
  const maximumStart = Math.max(0, knownDuration - windowDuration);
  const startMs = knownDuration > 0
    ? Math.max(0, Math.min(Math.round(unclampedStart), maximumStart))
    : Math.max(0, Math.round(unclampedStart));
  const endMs = startMs + windowDuration;
  karaokeViewStartMs = startMs;
  karaokeWaveformLoading = true;
  if (karaokeWaveformStatus && !karaokeWaveformWindow) karaokeWaveformStatus.textContent = "正在读取可见时间范围的波形…";
  try {
    const width = karaokeWaveform?.getBoundingClientRect().width ?? 1_200;
    const pointCount = Math.max(240, Math.min(2_400, Math.round(width * window.devicePixelRatio)));
    const waveform = await invoke<WaveformWindow>("get_waveform_window", {
      request: {
        jobId: activeDetail.job.job_id,
        startMs,
        endMs,
        pointCount,
      },
    });
    if (requestId !== karaokeWaveformRequestId || currentArea !== "karaoke") return;
    karaokeWaveformWindow = waveform;
    karaokeViewStartMs = waveform.start_ms;
    syncKaraokeWaveformStatus();
    renderKaraokeTimeline();
  } catch (error) {
    if (requestId !== karaokeWaveformRequestId || currentArea !== "karaoke") return;
    karaokeWaveformWindow = null;
    if (karaokeWaveformStatus) karaokeWaveformStatus.textContent = `无法生成波形：${String(error)}`;
    renderKaraokeTimeline();
  } finally {
    if (requestId === karaokeWaveformRequestId) karaokeWaveformLoading = false;
  }
}

function renderKaraokeTimeline(): void {
  const canvas = karaokeWaveform;
  if (!canvas) return;
  const bounds = canvas.getBoundingClientRect();
  const scale = window.devicePixelRatio || 1;
  const width = Math.max(1, Math.round(bounds.width * scale));
  const height = Math.max(1, Math.round(bounds.height * scale));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  const context = canvas.getContext("2d");
  if (!context) return;
  context.setTransform(scale, 0, 0, scale, 0, 0);
  const cssWidth = width / scale;
  const cssHeight = height / scale;
  context.clearRect(0, 0, cssWidth, cssHeight);
  context.fillStyle = "#f8f5ec";
  context.fillRect(0, 0, cssWidth, cssHeight);
  const waveform = karaokeWaveformWindow;
  if (!waveform || waveform.peaks.length === 0) {
    context.fillStyle = "#777a72";
    context.font = "13px system-ui";
    context.textAlign = "center";
    context.fillText(activeDetail ? "正在准备波形…" : "选择一个字幕任务", cssWidth / 2, cssHeight / 2);
    return;
  }

  const range = waveform.end_ms - waveform.start_ms;
  const rulerHeight = 28;
  const waveformTop = rulerHeight;
  const waveformHeight = Math.max(70, cssHeight * 0.5);
  const centerY = waveformTop + waveformHeight / 2;
  context.strokeStyle = "#d6d0c3";
  context.lineWidth = 1;
  context.beginPath();
  context.moveTo(0, centerY);
  context.lineTo(cssWidth, centerY);
  context.stroke();
  context.strokeStyle = "#4f766a";
  context.lineWidth = Math.max(1, cssWidth / waveform.peaks.length);
  context.beginPath();
  waveform.peaks.forEach((peak, index) => {
    const x = (index + 0.5) / waveform.peaks.length * cssWidth;
    const minimum = Math.max(-1, peak.min * karaokeWaveformGain);
    const maximum = Math.min(1, peak.max * karaokeWaveformGain);
    context.moveTo(x, centerY + minimum * waveformHeight * 0.46);
    context.lineTo(x, centerY + maximum * waveformHeight * 0.46);
  });
  context.stroke();

  context.fillStyle = "#74776f";
  context.font = "10px ui-monospace, monospace";
  context.textAlign = "center";
  for (let marker = 0; marker <= 5; marker += 1) {
    const x = marker / 5 * cssWidth;
    const time = waveform.start_ms + range * marker / 5;
    context.fillText(formatPreciseTime(time), Math.min(cssWidth - 45, Math.max(45, x)), 17);
    context.strokeStyle = "rgba(112, 115, 106, .2)";
    context.beginPath();
    context.moveTo(x, rulerHeight - 5);
    context.lineTo(x, cssHeight);
    context.stroke();
  }

  const trackTop = waveformTop + waveformHeight + 14;
  const trackHeight = Math.max(38, cssHeight - trackTop - 10);
  const timelineSegments = karaokeTimingDrag?.draft ?? activeDetail?.segments ?? [];
  for (const segment of timelineSegments) {
    if (segment.end_ms < waveform.start_ms || segment.start_ms > waveform.end_ms) continue;
    const left = Math.max(0, (segment.start_ms - waveform.start_ms) / range * cssWidth);
    const right = Math.min(cssWidth, (segment.end_ms - waveform.start_ms) / range * cssWidth);
    const active = segment.id === (karaokeTextDraftDirty ? karaokeTextDraftSegmentId : activeSegmentId);
    context.fillStyle = active ? "#2e5d52" : "#dfe9e2";
    context.strokeStyle = active ? "#173f36" : "#8ca89c";
    context.lineWidth = active ? 2 : 1;
    context.beginPath();
    context.roundRect(left + 1, trackTop, Math.max(3, right - left - 2), trackHeight, 5);
    context.fill();
    context.stroke();
    const dragged = karaokeTimingDrag?.segmentIndex === segment.segment_index;
    context.strokeStyle = dragged ? "#b34b36" : "#587c70";
    context.lineWidth = dragged ? 3 : 2;
    context.beginPath();
    context.moveTo(left + 3, trackTop + 5);
    context.lineTo(left + 3, trackTop + trackHeight - 5);
    context.moveTo(right - 3, trackTop + 5);
    context.lineTo(right - 3, trackTop + trackHeight - 5);
    context.stroke();
    if (right - left > 45) {
      context.save();
      context.beginPath();
      context.rect(left + 5, trackTop + 2, Math.max(1, right - left - 10), trackHeight - 4);
      context.clip();
      context.fillStyle = active ? "#fffaf1" : "#36564d";
      context.font = "11px system-ui";
      context.textAlign = "left";
      context.fillText(segment.source_text, left + 7, trackTop + trackHeight / 2 + 4);
      context.restore();
    }
  }

  for (let index = 0; index + 1 < timelineSegments.length; index += 1) {
    const left = timelineSegments[index];
    const right = timelineSegments[index + 1];
    if (left.end_ms <= right.start_ms) continue;
    const warningLeft = Math.max(0, (right.start_ms - waveform.start_ms) / range * cssWidth);
    const warningRight = Math.min(cssWidth, (left.end_ms - waveform.start_ms) / range * cssWidth);
    context.fillStyle = "rgba(179, 75, 54, .3)";
    context.fillRect(warningLeft, trackTop, Math.max(2, warningRight - warningLeft), trackHeight);
  }

  const currentMs = Math.round((activeMedia?.currentTime ?? 0) * 1_000);
  if (currentMs >= waveform.start_ms && currentMs <= waveform.end_ms) {
    const playheadX = (currentMs - waveform.start_ms) / range * cssWidth;
    context.strokeStyle = "#b34b36";
    context.lineWidth = 2;
    context.beginPath();
    context.moveTo(playheadX, rulerHeight - 4);
    context.lineTo(playheadX, cssHeight);
    context.stroke();
    context.fillStyle = "#b34b36";
    context.beginPath();
    context.moveTo(playheadX - 6, rulerHeight - 5);
    context.lineTo(playheadX + 6, rulerHeight - 5);
    context.lineTo(playheadX, rulerHeight + 4);
    context.closePath();
    context.fill();
  }
}

function karaokeTimelineHitAtPoint(clientX: number, clientY: number): KaraokeTimelineHit | null {
  const canvas = karaokeWaveform;
  const waveform = karaokeWaveformWindow;
  const segments = karaokeTimingDrag?.draft ?? activeDetail?.segments;
  if (!canvas || !waveform || !segments) return null;
  const bounds = canvas.getBoundingClientRect();
  const localY = clientY - bounds.top;
  const waveformHeight = Math.max(70, bounds.height * 0.5);
  const trackTop = 28 + waveformHeight + 14;
  if (localY < trackTop || localY > bounds.height - 8) return null;
  const range = waveform.end_ms - waveform.start_ms;
  for (let index = segments.length - 1; index >= 0; index -= 1) {
    const segment = segments[index];
    const left = bounds.left + (segment.start_ms - waveform.start_ms) / range * bounds.width;
    const right = bounds.left + (segment.end_ms - waveform.start_ms) / range * bounds.width;
    if (clientX < left - 8 || clientX > right + 8) continue;
    const leftDistance = Math.abs(clientX - left);
    const rightDistance = Math.abs(clientX - right);
    if (leftDistance <= 8 || rightDistance <= 8) {
      return { segmentIndex: index, mode: leftDistance <= rightDistance ? "start" : "end" };
    }
    if (clientX >= left && clientX <= right) return { segmentIndex: index, mode: "move" };
  }
  return null;
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
  if (!karaokeWaveformStatus || !karaokeWaveformWindow || !activeDetail) return;
  const waveform = karaokeWaveformWindow;
  const anomaly = karaokeTimingAnomalySummary(activeDetail.segments);
  karaokeWaveformStatus.textContent = `${formatPreciseTime(waveform.start_ms)} — ${formatPreciseTime(waveform.end_ms)} · 每个可视峰值约 ${Math.max(1, Math.round(waveform.point_duration_ms))} ms${anomaly ? ` · ${anomaly}` : " · 单轨无重叠"}`;
}

function updateKaraokeTimingDrag(clientX: number): void {
  const drag = karaokeTimingDrag;
  const waveform = karaokeWaveformWindow;
  const canvas = karaokeWaveform;
  if (!drag || !waveform || !canvas) return;
  const bounds = canvas.getBoundingClientRect();
  const delta = Math.round(((clientX - drag.pointerStartX) / bounds.width * (waveform.end_ms - waveform.start_ms)) / 10) * 10;
  const original = drag.before[drag.segmentIndex];
  const segment = drag.draft[drag.segmentIndex];
  const previous = drag.draft[drag.segmentIndex - 1];
  const next = drag.draft[drag.segmentIndex + 1];
  const durationLimit = waveform.duration_ms || Number.MAX_SAFE_INTEGER;
  if (drag.mode === "move") {
    const minimumDelta = (previous?.end_ms ?? 0) - original.start_ms;
    const maximumDelta = (next?.start_ms ?? durationLimit) - original.end_ms;
    const boundedDelta = Math.max(minimumDelta, Math.min(maximumDelta, delta));
    segment.start_ms = original.start_ms + boundedDelta;
    segment.end_ms = original.end_ms + boundedDelta;
  } else if (drag.mode === "start") {
    const minimum = previous?.end_ms ?? 0;
    segment.start_ms = Math.max(minimum, Math.min(segment.end_ms - 10, original.start_ms + delta));
  } else {
    const maximum = next?.start_ms ?? durationLimit;
    segment.end_ms = Math.max(segment.start_ms + 10, Math.min(maximum, original.end_ms + delta));
  }
  drag.moved = segment.start_ms !== original.start_ms || segment.end_ms !== original.end_ms;
  if (karaokeTimingMessage) {
    const action = drag.mode === "move" ? "移动" : drag.mode === "start" ? "修剪入点" : "修剪出点";
    karaokeTimingMessage.textContent = `预览：${action}第 ${segment.segment_index + 1} 段至 ${formatPreciseTime(segment.start_ms)} → ${formatPreciseTime(segment.end_ms)}；松开后保存。`;
    karaokeTimingMessage.classList.remove("warning");
  }
  renderKaraokeTimeline();
}

async function commitKaraokeTimingDrag(): Promise<void> {
  const drag = karaokeTimingDrag;
  karaokeTimingDrag = null;
  if (!drag || !drag.moved || !activeDetail) {
    renderKaraokeTimeline();
    return;
  }
  const jobId = activeDetail.job.job_id;
  setSubtitleEditAction("正在原子保存字幕块时间…");
  try {
    const saved = await invoke<SubtitleSegment[]>("save_subtitle_timing", {
      request: {
        jobId,
        beforeSegments: drag.before,
        afterSegments: drag.draft,
      },
    });
    if (!activeDetail || activeDetail.job.job_id !== jobId) return;
    const segment = saved[drag.segmentIndex];
    const action = drag.mode === "move" ? "移动" : drag.mode === "start" ? "入点调整" : "出点调整";
    rememberStructureEdit(drag.before, saved, `第 ${segment.segment_index + 1} 段${action}`);
    activeDetail.segments = saved;
    activeSegmentId = segment.id;
    updateStructureUndoButton();
    syncKaraokeWaveformStatus();
    updateKaraokePosition((activeMedia?.currentTime ?? 0) * 1_000);
    renderKaraokeTimeline();
    setSubtitleEditAction(`第 ${segment.segment_index + 1} 段已保存为 ${formatPreciseTime(segment.start_ms)} → ${formatPreciseTime(segment.end_ms)}；相邻空白保持不变。`);
  } catch (error) {
    renderKaraokeTimeline();
    setSubtitleEditAction(`边界保存失败：${String(error)}`, true);
  }
}

function cancelKaraokeTimingDrag(): void {
  karaokeTimingDrag = null;
  karaokeSuppressClick = false;
  renderKaraokeTimeline();
  setSubtitleEditAction("已放弃本次边界拖动。");
}

function karaokeDraftSegment(): SubtitleSegment | undefined {
  const segmentId = karaokeTextDraftDirty ? karaokeTextDraftSegmentId : activeSegmentId;
  return activeDetail?.segments.find((item) => item.id === segmentId);
}

function syncKaraokeTextDraft(segment: SubtitleSegment | undefined, force = false): void {
  if (!karaokeCurrentSource || !karaokeCurrentTranslation) return;
  if (karaokeTextDraftDirty && !force) return;
  karaokeTextDraftSegmentId = segment?.id ?? null;
  karaokeTextDraftDirty = false;
  karaokeCurrentSource.value = segment?.source_text ?? "";
  karaokeCurrentTranslation.value = segment?.translated_text ?? "";
  karaokeCurrentSource.disabled = !segment;
  karaokeCurrentTranslation.disabled = !segment;
  if (karaokeSaveTextButton) karaokeSaveTextButton.disabled = true;
  if (karaokeDiscardTextButton) karaokeDiscardTextButton.disabled = true;
  if (karaokeUndoTextButton) karaokeUndoTextButton.disabled = !segment || !canUndoSegment(segment);
}

function markKaraokeTextDirty(): void {
  const segment = karaokeDraftSegment();
  if (!segment) return;
  activeMedia?.pause();
  karaokeTextDraftSegmentId = segment.id;
  karaokeTextDraftDirty = true;
  if (karaokeSaveTextButton) karaokeSaveTextButton.disabled = false;
  if (karaokeDiscardTextButton) karaokeDiscardTextButton.disabled = false;
  if (karaokeUndoTextButton) karaokeUndoTextButton.disabled = true;
  setSubtitleEditAction(`第 ${segment.segment_index + 1} 段有尚未保存的文字修改。`);
}

async function saveKaraokeTextDraft(): Promise<void> {
  if (!activeDetail || !karaokeCurrentSource || !karaokeCurrentTranslation || !karaokeTextDraftSegmentId || !karaokeTextDraftDirty) return;
  const before = activeDetail.segments.find((item) => item.id === karaokeTextDraftSegmentId);
  if (!before) return;
  if (!karaokeCurrentSource.value.trim()) {
    setSubtitleEditAction("原文不能为空。", true);
    return;
  }
  if (karaokeSaveTextButton) karaokeSaveTextButton.disabled = true;
  try {
    const updated = await persistSegment(before.id, karaokeCurrentSource.value, karaokeCurrentTranslation.value);
    rememberSubtitleSave(before, updated);
    replaceActiveSegment(updated);
    karaokeTextDraftDirty = false;
    syncKaraokeTextDraft(updated, true);
    updateStructureUndoButton();
    updateKaraokePosition((activeMedia?.currentTime ?? 0) * 1_000);
    setSubtitleEditAction(`第 ${updated.segment_index + 1} 段文字已保存到 SQLite。`);
  } catch (error) {
    if (karaokeSaveTextButton) karaokeSaveTextButton.disabled = false;
    setSubtitleEditAction(`文字保存失败：${String(error)}`, true);
  }
}

function discardKaraokeTextDraft(): void {
  const saved = activeDetail?.segments.find((item) => item.id === karaokeTextDraftSegmentId);
  syncKaraokeTextDraft(saved, true);
  setSubtitleEditAction("已放弃当前字幕尚未保存的文字修改。");
}

async function undoKaraokeTextSave(): Promise<void> {
  const segment = karaokeDraftSegment();
  if (!segment || !activeDetail || karaokeTextDraftDirty) return;
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
    history?.pop();
    if (history?.length === 0) subtitleUndoHistory.delete(key);
    replaceActiveSegment(restored);
    syncKaraokeTextDraft(restored, true);
    updateStructureUndoButton();
    updateKaraokePosition((activeMedia?.currentTime ?? 0) * 1_000);
    setSubtitleEditAction("已撤销当前字幕上一次文字保存；历史仅保留在本次 App 会话。");
  } catch (error) {
    if (karaokeUndoTextButton) karaokeUndoTextButton.disabled = false;
    setSubtitleEditAction(`文字撤销失败：${String(error)}`, true);
  }
}

function updateKaraokePosition(milliseconds: number): void {
  if (currentArea !== "karaoke") return;
  if (karaokeCurrentTime) karaokeCurrentTime.textContent = formatPreciseTime(milliseconds);
  const playbackSegment = subtitleAt(milliseconds);
  const segment = karaokeDraftSegment() ?? playbackSegment;
  if (karaokeSegmentTime) {
    karaokeSegmentTime.textContent = segment
      ? `${formatPreciseTime(segment.start_ms)} → ${formatPreciseTime(segment.end_ms)} · #${segment.segment_index + 1}`
      : "当前没有字幕";
  }
  if (!karaokeTextDraftDirty && karaokeTextDraftSegmentId !== segment?.id) syncKaraokeTextDraft(segment);
  const segmentIndex = segment ? activeDetail?.segments.findIndex((item) => item.id === segment.id) ?? -1 : -1;
  const playhead = Math.round(milliseconds);
  if (karaokeCutSegmentButton) {
    karaokeCutSegmentButton.disabled = !playbackSegment
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
  const waveform = karaokeWaveformWindow;
  if (karaokeFollowPlayhead && waveform && !karaokeWaveformLoading) {
    const margin = (waveform.end_ms - waveform.start_ms) * 0.12;
    if (milliseconds < waveform.start_ms + margin || milliseconds > waveform.end_ms - margin) {
      void loadKaraokeWaveform(milliseconds);
      return;
    }
  }
  renderKaraokeTimeline();
}

function syncKaraokeFollowButton(): void {
  karaokeFollowPlayheadButton?.classList.toggle("active", karaokeFollowPlayhead);
  if (karaokeFollowPlayheadButton) karaokeFollowPlayheadButton.textContent = karaokeFollowPlayhead ? "正在跟随播放头" : "跟随播放头";
}

function moveKaraokeWindow(direction: number): void {
  if (!activeDetail) return;
  karaokeFollowPlayhead = false;
  syncKaraokeFollowButton();
  const nextStart = Math.max(0, karaokeViewStartMs + karaokeWindowDurationMs() * 0.8 * direction);
  void loadKaraokeWaveform(nextStart, true);
}

function seekKaraokeBy(milliseconds: number): void {
  if (!activeMedia) return;
  const durationMs = Number.isFinite(activeMedia.duration) ? activeMedia.duration * 1_000 : Number.MAX_SAFE_INTEGER;
  seekTo(Math.max(0, Math.min(durationMs, activeMedia.currentTime * 1_000 + milliseconds)), false);
  updateKaraokePosition(activeMedia.currentTime * 1_000);
}

async function cutKaraokeSegmentAtPlayhead(): Promise<void> {
  if (!activeDetail || workspaceActionBusy) return;
  if (karaokeTextDraftDirty) {
    setSubtitleEditAction("Cut 前请先保存或放弃当前文字修改。", true);
    return;
  }
  const boundary = Math.round((activeMedia?.currentTime ?? 0) * 1_000 / 10) * 10;
  const segment = activeDetail.segments.find((item) => boundary > item.start_ms && boundary < item.end_ms);
  if (!segment) {
    setSubtitleEditAction("播放头必须位于一个字幕块内部才能切开。", true);
    return;
  }
  const before = cloneSubtitleSegments(activeDetail.segments);
  workspaceActionBusy = true;
  setSubtitleEditAction(`正在播放头 ${formatPreciseTime(boundary)} 切开第 ${segment.segment_index + 1} 段…`);
  try {
    const after = await invoke<SubtitleSegment[]>("split_subtitle", {
      request: {
        jobId: activeDetail.job.job_id,
        segmentId: segment.id,
        boundaryMs: boundary,
        leftSourceText: segment.source_text,
        rightSourceText: segment.source_text,
        leftTranslatedText: segment.translated_text,
        rightTranslatedText: segment.translated_text,
      },
    });
    rememberStructureEdit(before, after, `第 ${segment.segment_index + 1} 段 Cut`);
    applySubtitleStructure(after, segment.id);
    setSubtitleEditAction("已在播放头切开；为保证连续播放观感，文字暂时复制到两块，可回到文字粗修继续整理。 ");
  } catch (error) {
    setSubtitleEditAction(`切开失败：${String(error)}`, true);
  } finally {
    workspaceActionBusy = false;
  }
}

async function joinKaraokeSegmentWithNext(): Promise<void> {
  if (!activeDetail || !activeSegmentId || workspaceActionBusy) return;
  if (karaokeTextDraftDirty) {
    setSubtitleEditAction("Join 前请先保存或放弃当前文字修改。", true);
    return;
  }
  const index = activeDetail.segments.findIndex((item) => item.id === activeSegmentId);
  const left = activeDetail.segments[index];
  const right = activeDetail.segments[index + 1];
  if (!left || !right) return;
  if (left.end_ms !== right.start_ms) {
    setSubtitleEditAction("两块之间存在空白；请保留为两个字幕块，或先独立修剪到同一边界。", true);
    return;
  }
  const before = cloneSubtitleSegments(activeDetail.segments);
  workspaceActionBusy = true;
  setSubtitleEditAction(`正在连接第 ${left.segment_index + 1}、${right.segment_index + 1} 段…`);
  try {
    const after = await invoke<SubtitleSegment[]>("merge_subtitles", {
      request: {
        jobId: activeDetail.job.job_id,
        leftSegmentId: left.id,
        rightSegmentId: right.id,
        sourceText: joinedSubtitleText(left.source_text, right.source_text, activeSourceLanguage()),
        translatedText: left.translated_text && right.translated_text
          ? joinedSubtitleText(left.translated_text, right.translated_text, activeTargetLanguage())
          : null,
      },
    });
    rememberStructureEdit(before, after, `第 ${left.segment_index + 1}、${right.segment_index + 1} 段 Join`);
    applySubtitleStructure(after, left.id);
    setSubtitleEditAction("已连接相接的字幕块；文字已顺序拼接。 ");
  } catch (error) {
    setSubtitleEditAction(`连接失败：${String(error)}`, true);
  } finally {
    workspaceActionBusy = false;
  }
}

function scheduleKaraokeAnimation(): void {
  if (karaokeAnimationFrame !== null || currentArea !== "karaoke" || !activeMedia || activeMedia.paused) return;
  const tick = () => {
    if (currentArea !== "karaoke" || !activeMedia || activeMedia.paused) {
      karaokeAnimationFrame = null;
      return;
    }
    updateActiveSubtitle(activeMedia.currentTime * 1_000);
    karaokeAnimationFrame = window.requestAnimationFrame(tick);
  };
  karaokeAnimationFrame = window.requestAnimationFrame(tick);
}

function showWorkspaceSection(section: WorkspaceSection): void {
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

async function openJob(jobId: string, initialSection: WorkspaceSection = "translation"): Promise<void> {
  if (!jobId || !workspaceMessage) return;
  const requestId = ++navigationRequestId;
  await stopActivePlayback();
  if (requestId !== navigationRequestId) return;
  showWorkspace(true);
  showWorkspaceSection(initialSection);
  window.scrollTo({ top: 0, behavior: "auto" });
  workspaceMessage.textContent = "正在读取 SQLite 字幕工作区…";
  setWorkspaceAction("");
  if (subtitleList) subtitleList.innerHTML = `<div class="empty-state">正在读取字幕…</div>`;
  try {
    activeDetail = await invoke<JobDetail>("get_job_detail", { jobId });
    if (requestId !== navigationRequestId || currentArea !== "workspace") return;
    renderWorkspace(activeDetail);
  } catch (error) {
    if (requestId !== navigationRequestId || currentArea !== "workspace") return;
    activeDetail = null;
    workspaceMessage.textContent = `无法打开任务：${String(error)}`;
  }
}

async function enterSubtitleEditor(jobId: string): Promise<void> {
  if (!jobId) return;
  karaokeSelectedJobId = jobId;
  await showTopLevelArea("karaoke");
}

function renderWorkspace(detail: JobDetail): void {
  lastExportedSubtitlePath = null;
  revealExportButton?.classList.add("hidden");
  if (workspaceTitle) workspaceTitle.textContent = displayName(detail.job);
  if (workspaceMessage) {
    workspaceMessage.textContent = `${statusLabel(detail.job.status)} · ${detail.job.message} · ${jobTimingLabel(detail.job)}`;
  }
  if (segmentCount) segmentCount.textContent = `${detail.segments.length} 段`;
  if (openSubtitleEditorButton) {
    openSubtitleEditorButton.disabled = detail.segments.length === 0 || !matchesTerminalStatus(detail.job.status);
    openSubtitleEditorButton.title = openSubtitleEditorButton.disabled ? "转写结束并产生字幕后才能进入字幕编辑" : "打开当前任务的高级字幕编辑";
  }
  renderGlossaryOptions();
  const sourceTrackOption = videoSubtitleTrack?.querySelector<HTMLOptionElement>('option[value="source"]');
  const translationTrackOption = videoSubtitleTrack?.querySelector<HTMLOptionElement>('option[value="translation"]');
  if (sourceTrackOption) sourceTrackOption.textContent = `仅${languageLabel(detail.job.source_language)}原文`;
  if (translationTrackOption) translationTrackOption.textContent = `仅${languageLabel(detail.job.target_language)}译文`;
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
  mountMedia(detail.playback_path, detail.audio_fallback_path);
  if (sourceMediaMissing && mediaMessage) {
    mediaMessage.textContent = "原媒体已移动或删除；当前使用任务内保存的音频。可重新定位原媒体以恢复画面和视频烧录。";
  }
  renderSubtitleList(detail.segments);
  updateActiveSubtitle(0);
  renderSubtitleOverlayButton();
  updateTranslationControls();
  void refreshVideoRenders();
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
    await openJob(activeDetail.job.job_id, currentWorkspaceSection);
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
  if (!host || !message) return;
  const sessionId = ++mediaSessionId;
  activeMedia = null;
  updatePlaybackControls();
  host.replaceChildren();
  const firstPath = primaryPath ?? fallbackPath;
  if (!firstPath) {
    message.textContent = "没有找到可播放的本地媒体；任务可能仍在处理或源文件已移动。";
    return;
  }

  const loadPath = (path: string, isFallback: boolean): void => {
    const element = document.createElement(isAudioPath(path) ? "audio" : "video");
    element.controls = true;
    element.preload = "metadata";
    const isCurrent = () => activeMedia === element && mediaSessionId === sessionId;
    element.addEventListener("timeupdate", () => {
      if (!isCurrent()) return;
      updateActiveSubtitle(element.currentTime * 1_000);
      schedulePlaybackPositionSave();
    });
    element.addEventListener("seeked", () => {
      if (isCurrent()) updateActiveSubtitle(element.currentTime * 1_000);
    });
    element.addEventListener("play", () => {
      if (isCurrent()) updatePlaybackControls();
    });
    element.addEventListener("pause", () => {
      if (!isCurrent()) return;
      updatePlaybackControls();
      void persistPlaybackPosition();
    });
    element.addEventListener("ended", () => {
      if (!isCurrent()) return;
      updatePlaybackControls();
      void persistPlaybackPosition();
    });
    element.addEventListener("ratechange", () => {
      if (isCurrent()) updatePlaybackControls();
    });
    element.addEventListener("loadedmetadata", () => {
      if (!isCurrent()) return;
      if (resumeMs > 0 && Number.isFinite(element.duration)) element.currentTime = Math.min(resumeMs / 1_000, Math.max(0, element.duration - 0.25));
      updatePlaybackControls();
      updateActiveSubtitle(element.currentTime * 1_000);
    });
    element.addEventListener(
      "error",
      () => {
        if (!isCurrent()) return;
        if (!isFallback && fallbackPath && fallbackPath !== path) {
          message.textContent = "原媒体编码无法由系统播放器读取，已切换到任务音频。";
          loadPath(fallbackPath, true);
        } else {
          message.textContent = "媒体加载失败。可能是文件已移动，或系统 WebView 不支持这种编码。";
        }
      },
      { once: true },
    );
    activeMedia = element;
    element.playbackRate = Number(activePlaybackRateSelect()?.value || "1");
    host.replaceChildren(element);
    element.src = convertFileSrc(path);
    updatePlaybackControls();
    if (!isFallback) message.textContent = resumeMs > 0 ? `${path} · 已恢复到 ${formatTime(resumeMs)}` : path;
  };

  loadPath(firstPath, firstPath === fallbackPath && primaryPath === null);
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

function rememberStructureEdit(before: SubtitleSegment[], after: SubtitleSegment[], label: string): void {
  if (!activeDetail) return;
  const jobId = activeDetail.job.job_id;
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
  if (segmentCount) segmentCount.textContent = `${segments.length} 段`;
  if (currentArea === "workspace") renderSubtitleListPreservingView(segments, expandedSegmentId);
  else {
    syncKaraokeWaveformStatus();
    renderKaraokeTimeline();
  }
  updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
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
  if (!activeDetail || !undoSubtitleStructureButton) return;
  if (karaokeTextDraftDirty) {
    setSubtitleEditAction("撤销打轴前请先保存或放弃当前文字修改。", true);
    return;
  }
  const history = subtitleStructureUndoHistory.get(activeDetail.job.job_id);
  const entry = history?.at(-1);
  if (!entry) return;
  undoSubtitleStructureButton.disabled = true;
  try {
    const restored = await invoke<SubtitleSegment[]>("restore_subtitle_structure", {
      request: {
        jobId: activeDetail.job.job_id,
        beforeSegments: entry.before,
        afterSegments: entry.after,
      },
    });
    history?.pop();
    if (history?.length === 0) subtitleStructureUndoHistory.delete(activeDetail.job.job_id);
    applySubtitleStructure(restored, restored[0]?.id);
    setSubtitleEditAction(`已撤销${entry.label}。撤销历史只在当前 App 会话内保留。`);
  } catch (error) {
    setSubtitleEditAction(`无法撤销结构修改：${String(error)}`, true);
  } finally {
    updateStructureUndoButton();
  }
}

function renderSubtitleList(segments: SubtitleSegment[], expandedSegmentId?: string): void {
  if (!subtitleList) return;
  subtitleList.replaceChildren();
  if (segments.length === 0) {
    subtitleList.innerHTML = `<div class="empty-state"><strong>还没有字幕。</strong><span>任务完成后重新读取；正在执行的任务会继续保留在后台。</span></div>`;
    return;
  }

  for (const segment of segments) {
    const card = document.createElement("article");
    card.className = "subtitle-card";
    card.dataset.segmentId = segment.id;
    card.dataset.dirty = "false";

    const meta = document.createElement("div");
    meta.className = "subtitle-meta";
    const time = document.createElement("button");
    time.className = "timecode";
    time.type = "button";
    time.textContent = `${formatTime(segment.start_ms)} → ${formatTime(segment.end_ms)}`;
    time.addEventListener("click", () => seekTo(segment.start_ms));
    const flags = document.createElement("span");
    flags.className = "edit-flags";
    flags.textContent = editFlags(segment);
    meta.append(time, flags);

    const sourceLabel = document.createElement("label");
    sourceLabel.textContent = `${languageLabel(activeSourceLanguage())}原文`;
    const sourceInput = document.createElement("textarea");
    sourceInput.className = "subtitle-input source-input";
    sourceInput.rows = 2;
    sourceInput.value = segment.source_text;
    sourceLabel.append(sourceInput);

    const translationLabel = document.createElement("label");
    translationLabel.textContent = languageLabel(activeTargetLanguage());
    const translationInput = document.createElement("textarea");
    translationInput.className = "subtitle-input translation-input";
    translationInput.rows = 2;
    translationInput.placeholder = "尚无翻译";
    translationInput.value = segment.translated_text ?? "";
    translationLabel.append(translationInput);

    const footer = document.createElement("div");
    footer.className = "subtitle-footer";
    const state = document.createElement("span");
    state.textContent = savedSegmentState(segment);
    if (segment.translation_stale) state.classList.add("warning");
    const save = document.createElement("button");
    save.type = "button";
    save.textContent = "保存本段";
    save.disabled = true;
    const discard = document.createElement("button");
    discard.type = "button";
    discard.className = "secondary";
    discard.textContent = "放弃修改";
    discard.disabled = true;
    const undo = document.createElement("button");
    undo.type = "button";
    undo.className = "secondary";
    undo.textContent = "撤销上次保存";
    undo.disabled = !canUndoSegment(segment);
    const translate = document.createElement("button");
    translate.type = "button";
    translate.className = "translate-segment secondary";
    translate.textContent = segment.translated_text ? "重译本段" : "翻译本段";
    const capture = document.createElement("button");
    capture.type = "button";
    capture.className = "capture-glossary-term secondary";
    capture.textContent = "修正加入词表";
    const markDirty = (): void => {
      card.dataset.dirty = "true";
      save.disabled = false;
      discard.disabled = false;
      state.textContent = "有未保存的修改";
      state.classList.remove("warning");
    };
    sourceInput.addEventListener("input", markDirty);
    translationInput.addEventListener("input", markDirty);

    discard.addEventListener("click", () => {
      const saved = activeDetail?.segments.find((item) => item.id === segment.id) ?? segment;
      sourceInput.value = saved.source_text;
      translationInput.value = saved.translated_text ?? "";
      card.dataset.dirty = "false";
      save.disabled = true;
      discard.disabled = true;
      state.textContent = savedSegmentState(saved);
      state.classList.toggle("warning", saved.translation_stale);
    });

    save.addEventListener("click", () => void saveSegment(segment, sourceInput, translationInput, save, state));
    undo.addEventListener("click", () => void undoSegment(segment, undo, state));
    translate.addEventListener("click", () => void translateSegment(segment, sourceInput, translationInput, state));
    capture.addEventListener("click", () => void captureGlossaryCorrection(segment, sourceInput, state));
    const actions = document.createElement("div");
    actions.className = "subtitle-actions";
    actions.append(capture, discard, undo, translate, save);
    footer.append(state, actions);

    card.append(meta, sourceLabel, translationLabel, footer);
    subtitleList.append(card);
  }
  highlightSegment(activeSegmentId);
  updateTranslationControls();
  updateStructureUndoButton();
}

function savedSegmentState(segment: SubtitleSegment): string {
  return segment.translation_stale ? "原文已改变，当前译文需要重译" : "已保存到本地 SQLite";
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

async function saveSegment(
  segment: SubtitleSegment,
  sourceInput: HTMLTextAreaElement,
  translationInput: HTMLTextAreaElement,
  button: HTMLButtonElement,
  state: HTMLSpanElement,
): Promise<void> {
  if (!activeDetail) return;
  button.disabled = true;
  state.textContent = "正在保存…";
  try {
    const before = activeDetail.segments.find((item) => item.id === segment.id) ?? segment;
    const updated = await persistSegment(segment.id, sourceInput.value, translationInput.value);
    rememberSubtitleSave(before, updated);
    replaceActiveSegment(updated);
    renderSubtitleListPreservingView(activeDetail.segments, segment.id);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
  } catch (error) {
    state.textContent = `保存失败：${String(error)}`;
    state.classList.add("warning");
    button.disabled = false;
  }
}

async function undoSegment(
  segment: SubtitleSegment,
  button: HTMLButtonElement,
  state: HTMLSpanElement,
): Promise<void> {
  if (!activeDetail) return;
  const key = subtitleUndoKey(segment);
  const history = subtitleUndoHistory.get(key);
  const entry = history?.at(-1);
  const current = activeDetail.segments.find((item) => item.id === segment.id);
  if (!entry || !current || entry.afterFingerprint !== subtitleValueFingerprint(current)) {
    state.textContent = "无法撤销：字幕在保存后已重新读取或再次改变。";
    state.classList.add("warning");
    button.disabled = true;
    return;
  }
  button.disabled = true;
  state.textContent = "正在撤销上次保存…";
  state.classList.remove("warning");
  try {
    const restored = await restoreSegment(entry.before);
    history?.pop();
    if (history?.length === 0) subtitleUndoHistory.delete(key);
    replaceActiveSegment(restored);
    renderSubtitleListPreservingView(activeDetail.segments, segment.id);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
    setWorkspaceAction("已撤销该段上一次手动保存；此撤销历史仅在当前 App 会话内保留。");
  } catch (error) {
    state.textContent = `撤销失败：${String(error)}`;
    state.classList.add("warning");
    button.disabled = false;
  }
}

async function restoreSegment(snapshot: SubtitleSegment): Promise<SubtitleSegment> {
  return invoke<SubtitleSegment>("restore_subtitle", { snapshot });
}

async function persistSegment(
  segmentId: string,
  sourceText: string,
  translatedText: string,
  startMs?: number,
  endMs?: number,
): Promise<SubtitleSegment> {
  if (!activeDetail) throw new Error("没有打开的字幕任务");
  const current = activeDetail.segments.find((segment) => segment.id === segmentId);
  if (!current) throw new Error("字幕段已经不存在");
  return invoke<SubtitleSegment>("update_subtitle", {
    request: {
      jobId: activeDetail.job.job_id,
      segmentId,
      sourceText,
      translatedText: translatedText.trim() || null,
      startMs: startMs ?? current.start_ms,
      endMs: endMs ?? current.end_ms,
    },
  });
}

function replaceActiveSegment(updated: SubtitleSegment): void {
  if (!activeDetail) return;
  activeDetail.segments = activeDetail.segments.map((item) => (item.id === updated.id ? updated : item));
}

async function reloadTranslatedWorkspace(jobId: string): Promise<void> {
  const detail = await invoke<JobDetail>("get_job_detail", { jobId });
  if (!activeDetail || activeDetail.job.job_id !== jobId) return;
  activeDetail.job = detail.job;
  activeDetail.segments = detail.segments;
  activeDetail.translation_runs = detail.translation_runs;
  renderSubtitleListPreservingView(activeDetail.segments);
  updateTranslationControls();
  updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
}

async function translateSegment(
  segment: SubtitleSegment,
  sourceInput: HTMLTextAreaElement,
  translationInput: HTMLTextAreaElement,
  state: HTMLSpanElement,
): Promise<void> {
  if (!activeDetail || workspaceActionBusy || !translationStatus.configured) return;
  setWorkspaceBusy(true);
  const stopElapsed = startWorkspaceElapsed(`正在保存并发送本段到 ${translationStatus.provider}`);
  state.textContent = `正在保存并发送本段到 ${translationStatus.provider}…`;
  state.classList.remove("warning");
  try {
    const saved = await persistSegment(segment.id, sourceInput.value, translationInput.value);
    replaceActiveSegment(saved);
    const translated = await invoke<SubtitleSegment>("translate_subtitle", {
      jobId: activeDetail.job.job_id,
      segmentId: segment.id,
    });
    replaceActiveSegment(translated);
    renderSubtitleListPreservingView(activeDetail.segments, segment.id);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
    try {
      await reloadTranslatedWorkspace(activeDetail.job.job_id);
      setWorkspaceAction(`本段${languageLabel(activeTargetLanguage())}译文已由 ${translationStatus.provider} 更新并从 SQLite 重新读取。`);
    } catch (reloadError) {
      setWorkspaceAction(`翻译已经写入 SQLite，但重新读取失败：${String(reloadError)}`, true);
    }
  } catch (error) {
    state.textContent = `翻译失败：${String(error)}`;
    state.classList.add("warning");
    setWorkspaceAction(`翻译失败：${String(error)}`, true);
  } finally {
    stopElapsed();
    setWorkspaceBusy(false);
  }
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
  if (!activeMedia) return;
  const durationMs = Number.isFinite(activeMedia.duration) ? activeMedia.duration * 1_000 : milliseconds;
  activeMedia.currentTime = Math.max(0, Math.min(milliseconds, durationMs)) / 1_000;
  if (autoplay) void activeMedia.play().catch(() => undefined);
}

function activePlaybackRateSelect(): HTMLSelectElement | null {
  if (currentArea === "listening") return listeningPlaybackRateSelect;
  if (currentArea === "karaoke") return karaokePlaybackRateSelect;
  return playbackRateSelect;
}

function schedulePlaybackPositionSave(): void {
  if (currentArea !== "listening" || !activeMedia || Date.now() - lastPlaybackPositionSavedAt < 5_000) return;
  if (playbackPositionSaveTimer !== null) return;
  playbackPositionSaveTimer = window.setTimeout(() => {
    playbackPositionSaveTimer = null;
    void persistPlaybackPosition();
  }, 300);
}

async function persistPlaybackPosition(): Promise<void> {
  if (currentArea !== "listening" || !activeDetail || !activeMedia) return;
  await savePlaybackPosition(activeDetail.job.job_id, activeMedia);
}

async function savePlaybackPosition(jobId: string, media: HTMLMediaElement): Promise<void> {
  const positionMs = Math.max(0, Math.round(media.currentTime * 1_000));
  lastPlaybackPositionSavedAt = Date.now();
  await invoke("save_playback_position", {
    request: { jobId, positionMs },
  }).catch(() => undefined);
}

function updatePlaybackControls(): void {
  const disabled = !activeMedia;
  for (const button of [previousSubtitleButton, rewindMediaButton, togglePlaybackButton, forwardMediaButton, nextSubtitleButton, listeningPreviousSubtitleButton, listeningRewindMediaButton, listeningTogglePlaybackButton, listeningForwardMediaButton, listeningNextSubtitleButton, karaokeTogglePlaybackButton]) {
    if (button) button.disabled = disabled;
  }
  for (const select of [playbackRateSelect, listeningPlaybackRateSelect, karaokePlaybackRateSelect]) if (select) select.disabled = disabled;
  if (togglePlaybackButton) togglePlaybackButton.textContent = activeMedia && !activeMedia.paused ? "暂停" : "播放";
  if (listeningTogglePlaybackButton) listeningTogglePlaybackButton.textContent = activeMedia && !activeMedia.paused ? "暂停" : "播放";
  if (karaokeTogglePlaybackButton) karaokeTogglePlaybackButton.textContent = activeMedia && !activeMedia.paused ? "暂停" : "播放";
  if (activeMedia) for (const select of [playbackRateSelect, listeningPlaybackRateSelect, karaokePlaybackRateSelect]) if (select) select.value = String(activeMedia.playbackRate);
  scheduleKaraokeAnimation();
  if (subtitleOverlayVisible) syncSubtitleOverlay(subtitleAt((activeMedia?.currentTime ?? 0) * 1_000));
}

function togglePlayback(): void {
  if (!activeMedia) return;
  if (activeMedia.paused) void activeMedia.play().catch(() => undefined);
  else activeMedia.pause();
}

function seekRelative(seconds: number): void {
  if (!activeMedia) return;
  seekTo((activeMedia.currentTime + seconds) * 1_000, false);
}

function seekAdjacentSubtitle(direction: -1 | 1): void {
  if (!activeMedia || !activeDetail || activeDetail.segments.length === 0) return;
  const currentMs = activeMedia.currentTime * 1_000;
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
  if (!activeMedia || !rateSelect) return;
  const rates = Array.from(rateSelect.options).map((option) => Number(option.value));
  const currentIndex = rates.findIndex((rate) => rate === activeMedia?.playbackRate);
  const nextIndex = Math.max(0, Math.min(rates.length - 1, (currentIndex < 0 ? rates.indexOf(1) : currentIndex) + direction));
  activeMedia.playbackRate = rates[nextIndex];
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

function subtitleAt(milliseconds: number): SubtitleSegment | undefined {
  return activeDetail?.segments.find(
    (item) => milliseconds >= item.start_ms && milliseconds < item.end_ms,
  );
}

function subtitleOverlayPayload(segment: SubtitleSegment | undefined): SubtitleOverlayPayload {
  return {
    sourceText: segment?.source_text ?? "当前时间没有字幕。",
    translatedText: segment?.translated_text ?? null,
    sourceLanguage: activeSourceLanguage(),
    targetLanguage: activeTargetLanguage(),
    playing: Boolean(activeMedia && !activeMedia.paused),
    playbackRate: activeMedia?.playbackRate ?? 1,
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
  const segment = subtitleAt((activeMedia?.currentTime ?? 0) * 1_000);
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
  const segment = subtitleAt(milliseconds);
  const nextId = segment?.id ?? null;
  if (currentSource) currentSource.textContent = segment?.source_text ?? "当前时间没有字幕。";
  if (currentTranslation) {
    currentTranslation.textContent = segment?.translated_text || `尚无${languageLabel(activeTargetLanguage())}翻译`;
    currentTranslation.classList.toggle("stale", segment?.translation_stale ?? false);
  }
  if (listeningCurrentSource) listeningCurrentSource.textContent = segment?.source_text ?? "当前时间没有字幕。";
  if (listeningCurrentTranslation) listeningCurrentTranslation.textContent = segment?.translated_text || `尚无${languageLabel(activeTargetLanguage())}翻译`;
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
    if (Date.now() < state.userScrollingUntil || !activeMedia || activeMedia.paused) return;
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
  if (!activeMedia || activeMedia.paused || Date.now() < state.userScrollingUntil) return;
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
  subtitleList?.querySelectorAll<HTMLElement>(".subtitle-card").forEach((card) => {
    const active = card.dataset.segmentId === segmentId;
    card.classList.toggle("active", active);
    if (active) followActiveSubtitle(subtitleList, card);
  });
  listeningSubtitleList?.querySelectorAll<HTMLElement>(".listening-subtitle").forEach((card) => {
    const active = card.dataset.segmentId === segmentId;
    card.classList.toggle("active", active);
    if (active) followActiveSubtitle(listeningSubtitleList, card);
  });
}

async function translateAllSubtitles(): Promise<void> {
  if (!activeDetail || workspaceActionBusy || !translationStatus.configured) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("请先保存各段尚未保存的修改，再执行全部重译。", true);
    return;
  }
  const confirmed = await confirmAction({
    title: "全部翻译／重译？",
    message: `将把 ${activeDetail.segments.length} 段${languageLabel(activeSourceLanguage())}原文发送到 ${translationStatus.provider}，并覆盖现有${languageLabel(activeTargetLanguage())}译文，包括人工修改。`,
    confirmLabel: "发送并覆盖",
    danger: true,
  });
  if (!confirmed) return;

  setWorkspaceBusy(true);
  const batchCount = Math.ceil(activeDetail.segments.length / 12);
  const stopElapsed = startWorkspaceElapsed(
    `正在通过 ${translationStatus.provider} 翻译 ${activeDetail.segments.length} 段字幕（${batchCount} 批）`,
  );
  try {
    const jobId = activeDetail.job.job_id;
    activeDetail.segments = await invoke<SubtitleSegment[]>("translate_all_subtitles", {
      jobId,
    });
    renderSubtitleListPreservingView(activeDetail.segments);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
    try {
      await reloadTranslatedWorkspace(jobId);
      setWorkspaceAction(`已翻译 ${activeDetail.segments.length} 段，原子写入并从 SQLite 重新读取。`);
    } catch (reloadError) {
      setWorkspaceAction(`全部翻译已经原子写入 SQLite，但重新读取失败：${String(reloadError)}`, true);
    }
  } catch (error) {
    setWorkspaceAction(`全部翻译失败：${String(error)}`, true);
  } finally {
    stopElapsed();
    setWorkspaceBusy(false);
  }
}

async function exportSubtitles(): Promise<void> {
  if (!activeDetail || workspaceActionBusy) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("请先保存各段尚未保存的修改，再导出字幕。", true);
    return;
  }
  const artifacts = await chooseSubtitleExportArtifacts();
  if (!artifacts) return;
  const needsTranslation = artifacts.some((artifact) => artifact !== "source_srt");
  const staleCount = activeDetail.segments.filter((segment) => segment.translation_stale).length;
  const missingCount = activeDetail.segments.filter((segment) => !segment.translated_text?.trim()).length;
  if (needsTranslation && (staleCount > 0 || missingCount > 0)) {
    setWorkspaceAction(`含译文的导出需要完整且最新的译文；当前缺失 ${missingCount} 段、待重译 ${staleCount} 段。也可以只导出原文 SRT。`, true);
    return;
  }

  setWorkspaceBusy(true);
  setWorkspaceAction("正在选择字幕导出目录…");
  try {
    const inputPath = activeDetail.job.input_path;
    const separator = inputPath ? Math.max(inputPath.lastIndexOf("/"), inputPath.lastIndexOf("\\")) : -1;
    const initialDirectory = inputPath && separator > 0 ? inputPath.slice(0, separator) : null;
    const outputDirectory = await invoke<string | null>("pick_subtitle_export_directory", {
      initialDirectory,
    });
    if (!outputDirectory) {
      setWorkspaceAction("已取消字幕导出。");
      return;
    }

    const plan = await invoke<SubtitleExportPlan>("preview_workspace_subtitle_export", {
      request: {
        jobId: activeDetail.job.job_id,
        outputDirectory,
        artifacts,
      },
    });
    let overwriteExisting = false;
    if (plan.existing_files.length > 0) {
      const names = plan.existing_files
        .map((path) => path.split(/[\\/]/).pop() ?? path)
        .join("\n");
      overwriteExisting = await confirmAction({
        title: "覆盖已有字幕？",
        message: `以下 ${plan.existing_files.length} 个字幕文件已存在：\n\n${names}`,
        confirmLabel: "覆盖字幕",
        danger: true,
      });
      if (!overwriteExisting) {
        setWorkspaceAction("已取消导出，现有字幕文件没有被修改。");
        return;
      }
    }

    setWorkspaceAction("正在从 SQLite 当前内容生成原文、译文和双语字幕…");
    const exported = await invoke<SubtitleExport>("export_workspace_subtitles", {
      request: {
        jobId: activeDetail.job.job_id,
        outputDirectory,
        overwriteExisting,
        artifacts,
      },
    });
    const exportedPaths: Record<SubtitleExportArtifact, string> = {
      source_srt: exported.source_srt,
      translated_srt: exported.translated_srt,
      bilingual_srt: exported.bilingual_srt,
      bilingual_ass: exported.bilingual_ass,
    };
    lastExportedSubtitlePath = exportedPaths[artifacts[0]];
    revealExportButton?.classList.remove("hidden");
    setWorkspaceAction(`已导出 ${artifacts.length} 个字幕文件到：${outputDirectory}`);
  } catch (error) {
    setWorkspaceAction(`导出失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

function chooseSubtitleExportArtifacts(): Promise<SubtitleExportArtifact[] | null> {
  if (!subtitleExportDialog || !subtitleExportMessage) return Promise.resolve(null);
  subtitleExportMessage.textContent = "";
  subtitleExportDialog.showModal();
  return new Promise((resolve) => {
    subtitleExportResolver = resolve;
  });
}

async function revealExportedSubtitle(): Promise<void> {
  if (!lastExportedSubtitlePath || workspaceActionBusy) return;
  setWorkspaceBusy(true);
  try {
    await invoke("reveal_exported_subtitle", { path: lastExportedSubtitlePath });
  } catch (error) {
    setWorkspaceAction(`${fileManagerLabel} 定位失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

function videoTrackLabel(track: VideoRender["subtitle_track"]): string {
  if (track === "source") return `仅${languageLabel(activeSourceLanguage())}原文`;
  if (track === "translation") return `仅${languageLabel(activeTargetLanguage())}译文`;
  return "原文＋译文";
}

function videoRenderStatusLabel(render: VideoRender): string {
  if (render.status === "queued") return "等待烧录";
  if (render.status === "running") return `烧录中 ${Math.round(render.progress * 100)}%`;
  if (render.status === "done") return "已完成";
  if (render.status === "cancelled") return "已取消";
  return "失败";
}

function suggestedVideoName(): string {
  const raw = activeDetail ? displayName(activeDetail.job) : "Atogaki";
  const safe = raw
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, "_")
    .replace(/[. ]+$/g, "")
    .slice(0, 100) || "Atogaki";
  const track = videoSubtitleTrack?.value ?? "bilingual";
  return `${safe}.${track}.mp4`;
}

function renderVideoRenderHistory(): void {
  if (!videoRenderList || !videoRenderCount) return;
  videoRenderCount.textContent = `${videoRenders.length} 条`;
  if (videoRenders.length === 0) {
    videoRenderList.innerHTML = `<div class="empty-state">还没有烧录记录。</div>`;
    return;
  }
  videoRenderList.innerHTML = videoRenders
    .map((render) => {
      const details = render.status === "done"
        ? `${render.encoder === "videotoolbox" ? "VideoToolbox" : render.encoder === "mpeg4" ? "MPEG-4 软件编码" : render.encoder ?? "未知编码器"} · 音频 ${render.audio_encoder ?? "未知"}${render.fallback_reason ? ` · ${render.fallback_reason}` : ""}`
        : render.error_message ?? videoTrackLabel(render.subtitle_track);
      const actions = render.status === "queued" || render.status === "running"
        ? `<button type="button" class="danger" data-cancel-render="${escapeHtml(render.id)}">取消</button>`
        : render.status === "done"
          ? `<button type="button" class="secondary" data-reveal-render="${escapeHtml(render.output_path)}">在 ${fileManagerLabel} 中显示</button>`
          : "";
      return `<article class="video-render-row">
        <div class="video-render-row-main">
          <strong>${escapeHtml(videoRenderStatusLabel(render))}</strong>
          <span>${escapeHtml(details)}</span>
          <code>${escapeHtml(render.output_path)}</code>
          ${render.status === "running" ? `<progress max="1" value="${render.progress}"></progress>` : ""}
        </div>
        ${actions}
      </article>`;
    })
    .join("");
  videoRenderList.querySelectorAll<HTMLButtonElement>("[data-cancel-render]").forEach((button) => {
    button.addEventListener("click", () => void cancelVideoRender(button.dataset.cancelRender ?? ""));
  });
  videoRenderList.querySelectorAll<HTMLButtonElement>("[data-reveal-render]").forEach((button) => {
    button.addEventListener("click", () => void revealRenderedVideo(button.dataset.revealRender ?? ""));
  });
}

async function refreshVideoRenders(): Promise<void> {
  const jobId = activeDetail?.job.job_id;
  if (!jobId) return;
  try {
    const renders = await invoke<VideoRender[]>("list_video_renders", { sourceJobId: jobId });
    if (activeDetail?.job.job_id !== jobId) return;
    videoRenders = renders;
    renderVideoRenderHistory();
  } catch (error) {
    if (videoRenderDialog?.open && videoRenderMessage) {
      videoRenderMessage.textContent = `无法读取烧录记录：${String(error)}`;
    }
  }
}

function renderMediaCapabilities(): void {
  if (!mediaCapabilitiesHost) return;
  if (!mediaCapabilities) {
    mediaCapabilitiesHost.textContent = "正在检查内置 FFmpeg sidecar…";
    mediaCapabilitiesHost.classList.remove("warning");
    return;
  }
  if (submitVideoRenderButton) {
    submitVideoRenderButton.disabled = videoRenderSubmitting || !mediaCapabilities.ready_for_hard_subtitles;
  }
  const encoder = [
    ...(desktopPlatform === "macos"
      ? [mediaCapabilities.videotoolbox_encoder ? "VideoToolbox 可用" : "VideoToolbox 不可用"]
      : []),
    mediaCapabilities.mpeg4_encoder
      ? desktopPlatform === "macos" ? "MPEG-4 回退可用" : "MPEG-4 软件编码可用"
      : desktopPlatform === "macos" ? "MPEG-4 回退缺失" : "MPEG-4 软件编码缺失",
  ].join(" · ");
  mediaCapabilitiesHost.innerHTML = `<strong>${mediaCapabilities.ready_for_hard_subtitles ? "烧录环境就绪" : "烧录环境不可用"}</strong><span>${escapeHtml(encoder)} · libass ${mediaCapabilities.ass_filter ? "可用" : "缺失"}</span><code>${escapeHtml(mediaCapabilities.binary_path)}</code><small>${escapeHtml(mediaCapabilities.version)}</small>`;
  mediaCapabilitiesHost.classList.toggle("warning", !mediaCapabilities.ready_for_hard_subtitles);
}

async function openVideoRenderDialog(): Promise<void> {
  if (!activeDetail || workspaceActionBusy) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("请先保存各段尚未保存的修改，再烧录视频。", true);
    return;
  }
  const staleCount = activeDetail.segments.filter((segment) => segment.translation_stale).length;
  if (staleCount > 0) {
    setWorkspaceAction(`有 ${staleCount} 段译文已过期，请重译或修正后再烧录。`, true);
    return;
  }
  if (!activeDetail.job.input_path || !activeDetail.playback_path || isAudioPath(activeDetail.job.input_path)) {
    setWorkspaceAction("当前任务没有可烧录的视频源；如果原文件已移动，请先在字幕校对中重新定位。", true);
    return;
  }
  if (videoRenderMessage) videoRenderMessage.textContent = "";
  if (videoOutputPath && !videoOutputPath.value) videoOutputPath.placeholder = suggestedVideoName();
  selectedVideoOutputAlreadyExists = false;
  if (submitVideoRenderButton) submitVideoRenderButton.disabled = true;
  videoRenderDialog?.showModal();
  renderMediaCapabilities();
  await Promise.all([
    invoke<MediaCapabilities>("media_capabilities")
      .then((capabilities) => {
        mediaCapabilities = capabilities;
        renderMediaCapabilities();
      })
      .catch((error) => {
        mediaCapabilities = null;
        if (mediaCapabilitiesHost) {
          mediaCapabilitiesHost.textContent = `ffmpeg 检查失败：${String(error)}`;
          mediaCapabilitiesHost.classList.add("warning");
        }
        if (submitVideoRenderButton) submitVideoRenderButton.disabled = true;
      }),
    refreshVideoRenders(),
  ]);
}

async function chooseVideoOutput(): Promise<void> {
  if (!activeDetail || videoRenderSubmitting) return;
  const inputPath = activeDetail.job.input_path;
  const separator = inputPath ? Math.max(inputPath.lastIndexOf("/"), inputPath.lastIndexOf("\\")) : -1;
  const initialDirectory = inputPath && separator > 0 ? inputPath.slice(0, separator) : null;
  if (videoRenderMessage) videoRenderMessage.textContent = "正在打开视频保存面板…";
  try {
    const selection = await invoke<VideoOutputSelection | null>("pick_video_output_file", {
      initialDirectory,
      suggestedName: suggestedVideoName(),
    });
    if (!selection) {
      if (videoRenderMessage) videoRenderMessage.textContent = "已取消选择。";
      return;
    }
    if (videoOutputPath) videoOutputPath.value = selection.path;
    selectedVideoOutputAlreadyExists = selection.alreadyExists;
    if (videoRenderMessage) {
      videoRenderMessage.textContent = selection.alreadyExists
        ? "目标文件已经存在，开始前会再次确认覆盖。"
        : "已选择输出位置。";
    }
  } catch (error) {
    if (videoRenderMessage) videoRenderMessage.textContent = `无法选择输出位置：${String(error)}`;
  }
}

async function submitVideoRender(): Promise<void> {
  if (!activeDetail || !videoOutputPath || !videoSubtitleTrack || videoRenderSubmitting) return;
  const outputPath = videoOutputPath.value.trim();
  if (!outputPath.toLowerCase().endsWith(".mp4")) {
    if (videoRenderMessage) videoRenderMessage.textContent = "输出路径必须以 .mp4 结尾。";
    return;
  }
  let overwriteExisting = false;
  if (selectedVideoOutputAlreadyExists) {
    overwriteExisting = await confirmAction({
      title: "覆盖已有视频？",
      message: "目标视频已经存在。只有新视频烧录成功后，Atogaki 才会安全替换它。",
      confirmLabel: "烧录并替换",
      danger: true,
    });
    if (!overwriteExisting) {
      if (videoRenderMessage) videoRenderMessage.textContent = "已取消，现有视频不会被修改。";
      return;
    }
  }
  videoRenderSubmitting = true;
  if (submitVideoRenderButton) submitVideoRenderButton.disabled = true;
  if (videoRenderMessage) videoRenderMessage.textContent = "正在冻结 SQLite 字幕并创建烧录任务…";
  try {
    const render = await invoke<VideoRender>("submit_video_render", {
      request: {
        sourceJobId: activeDetail.job.job_id,
        outputPath,
        subtitleTrack: videoSubtitleTrack.value,
        overwriteExisting,
      },
    });
    selectedVideoOutputAlreadyExists = false;
    if (videoRenderMessage) {
      videoRenderMessage.textContent = `已提交：${videoTrackLabel(render.subtitle_track)}，可关闭窗口继续校对。`;
    }
    await refreshVideoRenders();
  } catch (error) {
    if (videoRenderMessage) videoRenderMessage.textContent = `提交失败：${String(error)}`;
  } finally {
    videoRenderSubmitting = false;
    if (submitVideoRenderButton) {
      submitVideoRenderButton.disabled = mediaCapabilities?.ready_for_hard_subtitles === false;
    }
  }
}

async function cancelVideoRender(renderId: string): Promise<void> {
  if (!renderId) return;
  const confirmed = await confirmAction({
    title: "取消视频烧录？",
    message: "FFmpeg 会停止，临时输出会被清理；已完成的源任务和字幕不会改变。",
    confirmLabel: "取消烧录",
    danger: true,
  });
  if (!confirmed) return;
  try {
    await invoke<VideoRender>("cancel_video_render", { renderId });
    if (videoRenderMessage) videoRenderMessage.textContent = "已请求取消，正在停止 FFmpeg 并清理临时文件。";
    await refreshVideoRenders();
  } catch (error) {
    if (videoRenderMessage) videoRenderMessage.textContent = `取消失败：${String(error)}`;
  }
}

async function revealRenderedVideo(path: string): Promise<void> {
  try {
    await invoke("reveal_rendered_video", { path });
  } catch (error) {
    if (videoRenderMessage) videoRenderMessage.textContent = `${fileManagerLabel} 定位失败：${String(error)}`;
  }
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
  state: HTMLSpanElement,
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
  setWorkspaceAction("正在比较词表与 SQLite 原文字幕…");
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
  setWorkspaceAction("正在把词表修正写入 SQLite…");
  try {
    const applied = await invoke<GlossaryApplyResult>("apply_glossary_to_workspace", {
      jobId: activeDetail.job.job_id,
      glossaryId: pendingGlossaryPreview.glossary_id,
    });
    activeDetail.segments = applied.segments;
    clearGlossaryPreview();
    renderSubtitleListPreservingView(activeDetail.segments);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
    setWorkspaceAction(
      `已修正 ${applied.changed_segments} 段并保存到 SQLite；${applied.stale_translations} 段译文需要重译。`,
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
settingsProvider?.addEventListener("change", () => {
  if (settingsProvider.value === "deepseek") {
    if (settingsProviderBaseUrl) settingsProviderBaseUrl.value = "https://api.deepseek.com";
    if (settingsProviderModel) settingsProviderModel.value = "deepseek-v4-flash";
  } else if (settingsProvider.value === "openai-compatible") {
    if (settingsProviderBaseUrl) settingsProviderBaseUrl.value = "https://api.openai.com/v1";
    if (settingsProviderModel) settingsProviderModel.value = "";
  }
  if ((settingsProvider.value === "deepseek" || settingsProvider.value === "openai-compatible") && settingsTranslationStyle && !settingsTranslationStyle.value.trim()) {
    settingsTranslationStyle.value = "准确、自然的简体中文口语字幕；保留说话语气，不补充原文没有的信息。";
  }
  syncProviderSettings();
  if (apiKeyStatus && settingsProvider.value !== "none") {
    apiKeyStatus.textContent = "尚未检查当前 provider；可点击“检查所选 Key”确认系统凭据是否存在。";
    apiKeyStatus.classList.remove("warning");
  }
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
document.querySelector<HTMLButtonElement>("#back-to-jobs")?.addEventListener("click", () => {
  void showTopLevelArea("workbench").then(() => refresh());
});
document.querySelector<HTMLButtonElement>("#reload-detail")?.addEventListener("click", () => {
  if (activeDetail) void openJob(activeDetail.job.job_id, currentWorkspaceSection);
});
relinkJobMediaButton?.addEventListener("click", () => void relinkActiveJobMedia());
openSubtitleEditorButton?.addEventListener("click", () => {
  if (!activeDetail) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("进入字幕编辑前，请先保存或放弃当前字幕校对中的文字草稿。", true);
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
translateAllButton?.addEventListener("click", () => void translateAllSubtitles());
openSubtitleOverlayButton?.addEventListener("click", () => void openSubtitleOverlay());
previousSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(-1));
rewindMediaButton?.addEventListener("click", () => seekRelative(-5));
togglePlaybackButton?.addEventListener("click", togglePlayback);
forwardMediaButton?.addEventListener("click", () => seekRelative(5));
nextSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(1));
playbackRateSelect?.addEventListener("change", () => {
  if (activeMedia && playbackRateSelect) activeMedia.playbackRate = Number(playbackRateSelect.value);
});
listeningPreviousSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(-1));
listeningRewindMediaButton?.addEventListener("click", () => seekRelative(-5));
listeningTogglePlaybackButton?.addEventListener("click", togglePlayback);
listeningForwardMediaButton?.addEventListener("click", () => seekRelative(5));
listeningNextSubtitleButton?.addEventListener("click", () => seekAdjacentSubtitle(1));
listeningPlaybackRateSelect?.addEventListener("change", () => {
  if (activeMedia && listeningPlaybackRateSelect) activeMedia.playbackRate = Number(listeningPlaybackRateSelect.value);
});
syncKaraokeZoomLabel();
karaokeTogglePlaybackButton?.addEventListener("click", togglePlayback);
karaokePlaybackRateSelect?.addEventListener("change", () => {
  if (activeMedia && karaokePlaybackRateSelect) activeMedia.playbackRate = Number(karaokePlaybackRateSelect.value);
});
document.querySelector<HTMLButtonElement>("#karaoke-step-back-small")?.addEventListener("click", () => seekKaraokeBy(-10));
document.querySelector<HTMLButtonElement>("#karaoke-step-back")?.addEventListener("click", () => seekKaraokeBy(-100));
document.querySelector<HTMLButtonElement>("#karaoke-step-forward")?.addEventListener("click", () => seekKaraokeBy(100));
document.querySelector<HTMLButtonElement>("#karaoke-step-forward-small")?.addEventListener("click", () => seekKaraokeBy(10));
document.querySelector<HTMLButtonElement>("#karaoke-window-back")?.addEventListener("click", () => moveKaraokeWindow(-1));
document.querySelector<HTMLButtonElement>("#karaoke-window-forward")?.addEventListener("click", () => moveKaraokeWindow(1));
karaokeFollowPlayheadButton?.addEventListener("click", () => {
  karaokeFollowPlayhead = !karaokeFollowPlayhead;
  syncKaraokeFollowButton();
  if (karaokeFollowPlayhead) void loadKaraokeWaveform((activeMedia?.currentTime ?? 0) * 1_000);
});
karaokeZoomInput?.addEventListener("input", () => {
  const oldWindow = karaokeWaveformWindow ? karaokeWaveformWindow.end_ms - karaokeWaveformWindow.start_ms : 30_000;
  const center = karaokeViewStartMs + oldWindow / 2;
  syncKaraokeZoomLabel();
  karaokeFollowPlayhead = false;
  syncKaraokeFollowButton();
  scheduleKaraokeWaveformReload(Math.max(0, center - karaokeWindowDurationMs() / 2));
});
karaokeWaveformGainInput?.addEventListener("input", () => {
  karaokeWaveformGain = Number(karaokeWaveformGainInput.value);
  if (karaokeWaveformGainLabel) karaokeWaveformGainLabel.value = `${karaokeWaveformGain.toFixed(karaokeWaveformGain % 1 ? 2 : 0)}×`;
  renderKaraokeTimeline();
});
karaokeWaveform?.addEventListener("pointerdown", (event) => {
  if (!activeDetail || !karaokeWaveformWindow) return;
  const hit = karaokeTimelineHitAtPoint(event.clientX, event.clientY);
  if (hit) {
    if (karaokeTextDraftDirty) {
      setSubtitleEditAction("调整其他字幕块前，请先保存或放弃当前文字修改。", true);
      return;
    }
    activeMedia?.pause();
    activeSegmentId = activeDetail.segments[hit.segmentIndex]?.id ?? activeSegmentId;
    updateKaraokePosition((activeMedia?.currentTime ?? 0) * 1_000);
    karaokeTimingDrag = {
      pointerId: event.pointerId,
      segmentIndex: hit.segmentIndex,
      mode: hit.mode,
      pointerStartX: event.clientX,
      before: cloneSubtitleSegments(activeDetail.segments),
      draft: cloneSubtitleSegments(activeDetail.segments),
      moved: false,
    };
    karaokeSuppressClick = true;
  } else {
    karaokePanDrag = {
      pointerId: event.pointerId,
      pointerStartX: event.clientX,
      viewStartMs: karaokeViewStartMs,
      moved: false,
    };
  }
  karaokeFollowPlayhead = false;
  syncKaraokeFollowButton();
  karaokeWaveform.setPointerCapture(event.pointerId);
  event.preventDefault();
});
karaokeWaveform?.addEventListener("pointermove", (event) => {
  if (!karaokeTimingDrag && !karaokePanDrag) {
    const hit = karaokeTimelineHitAtPoint(event.clientX, event.clientY);
    karaokeWaveform.style.cursor = hit?.mode === "move" ? "grab" : hit ? "ew-resize" : "crosshair";
    return;
  }
  if (karaokeTimingDrag?.pointerId === event.pointerId) updateKaraokeTimingDrag(event.clientX);
  if (karaokePanDrag?.pointerId === event.pointerId && karaokeWaveformWindow) {
    const bounds = karaokeWaveform.getBoundingClientRect();
    const deltaPixels = event.clientX - karaokePanDrag.pointerStartX;
    karaokePanDrag.moved ||= Math.abs(deltaPixels) >= 3;
    if (karaokePanDrag.moved) {
      const duration = karaokeWindowDurationMs();
      const maximum = Math.max(0, karaokeWaveformWindow.duration_ms - duration);
      karaokeViewStartMs = Math.max(0, Math.min(maximum, karaokePanDrag.viewStartMs - deltaPixels / bounds.width * duration));
      karaokeWaveform.style.cursor = "grabbing";
      scheduleKaraokeWaveformReload(karaokeViewStartMs);
    }
  }
  event.preventDefault();
});
karaokeWaveform?.addEventListener("pointerup", (event) => {
  if (karaokeTimingDrag?.pointerId === event.pointerId) void commitKaraokeTimingDrag();
  if (karaokePanDrag?.pointerId === event.pointerId) {
    karaokeSuppressClick = karaokePanDrag.moved;
    karaokePanDrag = null;
  }
  karaokeWaveform.releasePointerCapture(event.pointerId);
  event.preventDefault();
});
karaokeWaveform?.addEventListener("pointercancel", () => {
  karaokePanDrag = null;
  cancelKaraokeTimingDrag();
});
karaokeWaveform?.addEventListener("wheel", (event) => {
  if (!karaokeWaveformWindow || !karaokeZoomInput) return;
  const bounds = karaokeWaveform.getBoundingClientRect();
  if (event.ctrlKey || event.metaKey) {
    if (karaokeGestureStart) {
      event.preventDefault();
      return;
    }
    const ratio = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const oldDuration = karaokeWindowDurationMs();
    const anchor = karaokeViewStartMs + ratio * oldDuration;
    karaokeZoomInput.value = String(Math.max(0, Math.min(100, Number(karaokeZoomInput.value) + event.deltaY * 0.08)));
    syncKaraokeZoomLabel();
    const newStart = anchor - ratio * karaokeWindowDurationMs();
    karaokeFollowPlayhead = false;
    syncKaraokeFollowButton();
    scheduleKaraokeWaveformReload(Math.max(0, newStart));
    event.preventDefault();
    return;
  }
  const horizontalDelta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.shiftKey ? event.deltaY : 0;
  if (horizontalDelta !== 0) {
    const duration = karaokeWindowDurationMs();
    const maximum = Math.max(0, karaokeWaveformWindow.duration_ms - duration);
    karaokeViewStartMs = Math.max(0, Math.min(maximum, karaokeViewStartMs + horizontalDelta / bounds.width * duration));
    karaokeFollowPlayhead = false;
    syncKaraokeFollowButton();
    scheduleKaraokeWaveformReload(karaokeViewStartMs);
    event.preventDefault();
  }
}, { passive: false });
karaokeWaveform?.addEventListener("gesturestart", ((event: KaraokeGestureEvent) => {
  if (!karaokeWaveformWindow || !karaokeZoomInput) return;
  const bounds = karaokeWaveform!.getBoundingClientRect();
  const ratio = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
  const durationMs = karaokeWindowDurationMs();
  karaokeGestureStart = {
    durationMs,
    anchorMs: karaokeViewStartMs + ratio * durationMs,
    ratio,
  };
  karaokeFollowPlayhead = false;
  syncKaraokeFollowButton();
  event.preventDefault();
}) as EventListener, { passive: false });
karaokeWaveform?.addEventListener("gesturechange", ((event: KaraokeGestureEvent) => {
  if (!karaokeGestureStart || !karaokeZoomInput) return;
  const durationMs = Math.max(2_000, Math.min(120_000, karaokeGestureStart.durationMs / Math.max(0.05, event.scale)));
  karaokeZoomInput.value = String(karaokeZoomValueForDuration(durationMs));
  syncKaraokeZoomLabel();
  scheduleKaraokeWaveformReload(Math.max(0, karaokeGestureStart.anchorMs - karaokeGestureStart.ratio * karaokeWindowDurationMs()));
  event.preventDefault();
}) as EventListener, { passive: false });
karaokeWaveform?.addEventListener("gestureend", ((event: Event) => {
  karaokeGestureStart = null;
  event.preventDefault();
}) as EventListener, { passive: false });
karaokeWaveform?.addEventListener("click", (event) => {
  if (karaokeSuppressClick) {
    karaokeSuppressClick = false;
    return;
  }
  if (!karaokeWaveformWindow) return;
  const bounds = karaokeWaveform.getBoundingClientRect();
  const ratio = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
  const milliseconds = karaokeWaveformWindow.start_ms + ratio * (karaokeWaveformWindow.end_ms - karaokeWaveformWindow.start_ms);
  karaokeFollowPlayhead = false;
  syncKaraokeFollowButton();
  seekTo(milliseconds, false);
  updateActiveSubtitle(milliseconds);
});
karaokeCutSegmentButton?.addEventListener("click", () => void cutKaraokeSegmentAtPlayhead());
karaokeJoinSegmentButton?.addEventListener("click", () => void joinKaraokeSegmentWithNext());
karaokeCurrentSource?.addEventListener("input", markKaraokeTextDirty);
karaokeCurrentTranslation?.addEventListener("input", markKaraokeTextDirty);
karaokeSaveTextButton?.addEventListener("click", () => void saveKaraokeTextDraft());
karaokeDiscardTextButton?.addEventListener("click", discardKaraokeTextDraft);
karaokeUndoTextButton?.addEventListener("click", () => void undoKaraokeTextSave());
karaokeOpenWorkspaceButton?.addEventListener("click", () => {
  if (!activeDetail) return;
  if (karaokeTextDraftDirty) {
    setSubtitleEditAction("返回字幕校对前，请先保存或放弃当前文字修改。", true);
    return;
  }
  void openJob(activeDetail.job.job_id, "review");
});
exportButton?.addEventListener("click", () => void exportSubtitles());
renderVideoButton?.addEventListener("click", () => void openVideoRenderDialog());
revealExportButton?.addEventListener("click", () => void revealExportedSubtitle());
document.querySelector<HTMLButtonElement>("#close-video-render")?.addEventListener("click", () => videoRenderDialog?.close());
document.querySelector<HTMLButtonElement>("#choose-video-output")?.addEventListener("click", () => void chooseVideoOutput());
submitVideoRenderButton?.addEventListener("click", () => void submitVideoRender());
videoOutputPath?.addEventListener("input", () => {
  selectedVideoOutputAlreadyExists = false;
});
videoSubtitleTrack?.addEventListener("change", () => {
  if (videoOutputPath && !videoOutputPath.value) videoOutputPath.placeholder = suggestedVideoName();
});
subtitleExportForm?.addEventListener("submit", (event) => {
  event.preventDefault();
  const artifacts = Array.from(subtitleExportForm.querySelectorAll<HTMLInputElement>('input[name="subtitle-artifact"]:checked'))
    .map((input) => input.value as SubtitleExportArtifact);
  if (artifacts.length === 0) {
    if (subtitleExportMessage) subtitleExportMessage.textContent = "请至少选择一种字幕文件。";
    return;
  }
  subtitleExportDialog?.close();
  subtitleExportResolver?.(artifacts);
  subtitleExportResolver = null;
});
document.querySelector<HTMLButtonElement>("#cancel-subtitle-export")?.addEventListener("click", () => {
  subtitleExportDialog?.close();
  subtitleExportResolver?.(null);
  subtitleExportResolver = null;
});
subtitleExportDialog?.addEventListener("cancel", (event) => {
  event.preventDefault();
  subtitleExportDialog.close();
  subtitleExportResolver?.(null);
  subtitleExportResolver = null;
});
document.querySelector<HTMLButtonElement>("#manage-glossaries")?.addEventListener("click", () => void openGlossaryManager());
document.querySelector<HTMLButtonElement>("#manage-workspace-glossary")?.addEventListener("click", () => void openGlossaryManager(workspaceGlossary?.value));
document.querySelector<HTMLButtonElement>("#close-glossaries")?.addEventListener("click", () => glossaryDialog?.close());
document.querySelector<HTMLButtonElement>("#new-glossary")?.addEventListener("click", () => void editGlossary(null));
document.querySelector<HTMLButtonElement>("#add-glossary-term")?.addEventListener("click", () => addGlossaryTermRow());
document.querySelector<HTMLButtonElement>("#save-glossary")?.addEventListener("click", () => void saveGlossaryEditor());
deleteGlossaryButton?.addEventListener("click", () => void deleteGlossaryEditor());
document.querySelector<HTMLButtonElement>("#preview-glossary")?.addEventListener("click", () => void previewGlossaryApplication());
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
    || (target instanceof HTMLElement && target.isContentEditable);
}

document.addEventListener("keydown", (event) => {
  if (karaokeTimingDrag && event.key === "Escape") {
    cancelKaraokeTimingDrag();
    event.preventDefault();
    return;
  }
  if (!activeDetail || !activeMedia || event.altKey || isTextEntryTarget(event.target)) return;
  if (document.querySelector("dialog[open]")) return;
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
    const path = await invoke<string | null>(command);
    if (typeof path === "string") {
      const input = kind === "media" ? mediaPath : kind === "model" ? modelPath : vadModelPath;
      if (input) input.value = path;
      if (taskMessage) taskMessage.textContent = `已选择${label}。`;
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
  void invoke<string>("submit_transcription", {
    request: {
      inputPath: mediaPath.value,
      modelPath: modelPath.value,
      sourceLanguage: selectedSourceLanguage(),
      vadModelPath: vadEnabled?.checked ? vadModelPath?.value.trim() || null : null,
      glossaryId: taskGlossary?.value || null,
      selectedContentGroups: taskGlossary?.value ? Array.from(selectedTaskContentGroups) : [],
    },
  })
    .then((jobId) => {
      taskMessage.textContent = `已排队：${jobId}`;
      void refresh();
    })
    .catch((error) => {
      taskMessage.textContent = `创建失败：${String(error)}`;
    })
    .finally(() => {
      submitButton.disabled = false;
    });
});

registerSubtitleFollowHost(subtitleList);
registerSubtitleFollowHost(listeningSubtitleList);
if (karaokeWaveform) {
  new ResizeObserver(() => {
    renderKaraokeTimeline();
    if (activeDetail && currentArea === "karaoke") void loadKaraokeWaveform(karaokeViewStartMs, true);
  }).observe(karaokeWaveform);
}
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
  if (!activeDetail || !activeMedia) return;
  executePlaybackAction(event.payload.action);
});
window.setInterval(() => void refresh(), 2_000);
window.setInterval(() => void refreshActiveJob(), 2_000);
window.setInterval(updateVisibleJobTimings, 1_000);
window.setInterval(() => {
  if (activeDetail && videoRenders.some((render) => render.status === "queued" || render.status === "running")) {
    void refreshVideoRenders();
  }
}, 1_000);
