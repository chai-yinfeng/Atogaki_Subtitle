import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import "./styles.css";

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
  updated_at_unix: number;
};

type Glossary = {
  id: string;
  name: string;
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
  ja_text: string;
  zh_text: string | null;
  source_edited: boolean;
  translation_edited: boolean;
  translation_stale: boolean;
};

type JobDetail = {
  job: LocalJob;
  segments: SubtitleSegment[];
  playback_path: string | null;
  audio_fallback_path: string | null;
};

type TranslationStatus = {
  provider: string;
  configured: boolean;
  source_language: string;
  target_language: string;
};

type RecognitionDefaults = {
  whisperModelPath: string | null;
  vadModelPath: string | null;
};

type SubtitleExport = {
  ja_srt: string;
  zh_srt: string;
  bilingual_srt: string;
  bilingual_ass: string;
  missing_translation_count: number;
};

type SubtitleExportPlan = {
  output_directory: string;
  base_name: string;
  ja_srt: string;
  zh_srt: string;
  bilingual_srt: string;
  bilingual_ass: string;
  existing_files: string[];
};

type MediaCapabilities = {
  binary_path: string;
  version: string;
  ass_filter: boolean;
  videotoolbox_encoder: boolean;
  libx264_encoder: boolean;
  ready_for_hard_subtitles: boolean;
};

type VideoRender = {
  id: string;
  source_job_id: string;
  input_path: string;
  subtitle_path: string;
  output_path: string;
  subtitle_track: "japanese" | "chinese" | "bilingual";
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

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing app root");

app.innerHTML = `
  <main class="shell">
    <header>
      <div>
        <p class="eyebrow">LOCAL JAPANESE MEDIA WORKSPACE</p>
        <h1>Atogaki</h1>
      </div>
      <button id="refresh" type="button">刷新任务</button>
    </header>
    <div id="home-view">
      <section class="intro">
        <h2>离线理解，保留在本机。</h2>
        <p>选择本地媒体与 Whisper 模型后，任务会先写入本地 SQLite，再在后台开始日语转写。</p>
        <p class="data-path" id="data-path">正在读取本地数据目录…</p>
      </section>
      <section class="create-task" aria-labelledby="create-heading">
        <div class="section-heading"><h2 id="create-heading">新建日语转写</h2><span>本地执行</span></div>
        <form id="task-form">
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
    <section id="workspace-view" class="workspace hidden" aria-labelledby="workspace-title">
      <div class="workspace-heading">
        <button id="back-to-jobs" class="secondary" type="button">← 返回任务</button>
        <div>
          <p class="eyebrow">PLAYBACK WORKSPACE</p>
          <h2 id="workspace-title">任务详情</h2>
          <p id="workspace-message" class="workspace-message"></p>
        </div>
        <button id="reload-detail" type="button" class="secondary">重新读取</button>
      </div>
      <section class="workspace-toolbar" aria-label="翻译与导出">
        <div>
          <strong>简体中文翻译</strong>
          <span id="translation-status">正在读取翻译配置…</span>
        </div>
        <div class="workspace-action-buttons">
          <button id="translate-all" type="button">全部翻译／重译</button>
          <button id="export-subtitles" type="button" class="secondary">导出字幕…</button>
          <button id="render-video" type="button" class="secondary">导出带字幕视频…</button>
          <button id="reveal-export" type="button" class="secondary hidden">在 Finder 中显示</button>
        </div>
        <div class="workspace-glossary-row">
          <div>
            <strong>识别词表修正</strong>
            <span id="job-glossary-status">当前任务未记录识别词表</span>
          </div>
          <select id="workspace-glossary"><option value="">选择词表…</option></select>
          <button id="preview-glossary" type="button" class="secondary">预览应用</button>
        </div>
        <div id="glossary-preview" class="glossary-preview hidden"></div>
        <p id="workspace-action-message" role="status"></p>
      </section>
      <div class="review-grid">
        <section class="media-panel" aria-label="媒体播放器">
          <div id="media-host" class="media-host"></div>
          <p id="media-message" class="media-message"></p>
          <div class="current-caption" aria-live="polite">
            <p id="current-ja">播放时将在这里显示当前日文。</p>
            <p id="current-zh">中文翻译</p>
          </div>
        </section>
        <section class="timeline" aria-labelledby="timeline-title">
          <div class="section-heading">
            <h2 id="timeline-title">字幕时间轴</h2>
            <span id="segment-count"></span>
          </div>
          <div id="subtitle-list" class="subtitle-list"></div>
        </section>
      </div>
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
          <label>词表名称<input id="glossary-name" maxlength="80" placeholder="例如：日语电台常用词" /></label>
          <div class="term-heading"><strong>提示词与识别修正规则</strong><button id="add-glossary-term" type="button" class="secondary">＋ 添加词条</button></div>
          <div id="glossary-terms" class="glossary-terms"></div>
          <div class="glossary-editor-footer">
            <span id="glossary-message" role="status"></span>
            <div><button id="delete-glossary" type="button" class="danger">删除</button><button id="save-glossary" type="button">保存词表</button></div>
          </div>
        </section>
      </div>
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
      <div id="media-capabilities" class="media-capabilities">正在检查 ffmpeg-full…</div>
      <div class="video-render-form">
        <label>字幕内容
          <select id="video-subtitle-track">
            <option value="bilingual">日中双语（推荐）</option>
            <option value="chinese">仅中文</option>
            <option value="japanese">仅日文</option>
          </select>
        </label>
        <label>输出视频
          <input id="video-output-path" placeholder="选择 MP4 保存位置，或粘贴完整路径" />
        </label>
        <button id="choose-video-output" type="button" class="secondary">选择位置</button>
        <p>提交时会把 SQLite 当前字幕冻结为本次 ASS 快照；默认优先 VideoToolbox，失败时明确回退到 libx264。</p>
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
  </main>
`;

const homeView = document.querySelector<HTMLDivElement>("#home-view");
const workspaceView = document.querySelector<HTMLElement>("#workspace-view");
const jobList = document.querySelector<HTMLDivElement>("#job-list");
const jobCount = document.querySelector<HTMLSpanElement>("#job-count");
const jobManagementMessage = document.querySelector<HTMLParagraphElement>("#job-management-message");
const dataPath = document.querySelector<HTMLParagraphElement>("#data-path");
const mediaPath = document.querySelector<HTMLInputElement>("#media-path");
const modelPath = document.querySelector<HTMLInputElement>("#model-path");
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
const subtitleList = document.querySelector<HTMLDivElement>("#subtitle-list");
const segmentCount = document.querySelector<HTMLSpanElement>("#segment-count");
const currentJa = document.querySelector<HTMLParagraphElement>("#current-ja");
const currentZh = document.querySelector<HTMLParagraphElement>("#current-zh");
const translationStatusText = document.querySelector<HTMLSpanElement>("#translation-status");
const translateAllButton = document.querySelector<HTMLButtonElement>("#translate-all");
const exportButton = document.querySelector<HTMLButtonElement>("#export-subtitles");
const renderVideoButton = document.querySelector<HTMLButtonElement>("#render-video");
const revealExportButton = document.querySelector<HTMLButtonElement>("#reveal-export");
const workspaceActionMessage = document.querySelector<HTMLParagraphElement>("#workspace-action-message");
const workspaceGlossary = document.querySelector<HTMLSelectElement>("#workspace-glossary");
const jobGlossaryStatus = document.querySelector<HTMLSpanElement>("#job-glossary-status");
const glossaryPreviewHost = document.querySelector<HTMLDivElement>("#glossary-preview");
const glossaryDialog = document.querySelector<HTMLDialogElement>("#glossary-dialog");
const glossaryListHost = document.querySelector<HTMLDivElement>("#glossary-list");
const glossaryName = document.querySelector<HTMLInputElement>("#glossary-name");
const glossaryTerms = document.querySelector<HTMLDivElement>("#glossary-terms");
const glossaryMessage = document.querySelector<HTMLSpanElement>("#glossary-message");
const deleteGlossaryButton = document.querySelector<HTMLButtonElement>("#delete-glossary");
const renameJobDialog = document.querySelector<HTMLDialogElement>("#rename-job-dialog");
const renameJobForm = document.querySelector<HTMLFormElement>("#rename-job-form");
const renameJobInput = document.querySelector<HTMLInputElement>("#rename-job-input");
const renameJobMessage = document.querySelector<HTMLSpanElement>("#rename-job-message");
const confirmationDialog = document.querySelector<HTMLDialogElement>("#confirmation-dialog");
const confirmationForm = document.querySelector<HTMLFormElement>("#confirmation-form");
const confirmationTitle = document.querySelector<HTMLHeadingElement>("#confirmation-title");
const confirmationMessage = document.querySelector<HTMLParagraphElement>("#confirmation-message");
const acceptConfirmationButton = document.querySelector<HTMLButtonElement>("#accept-confirmation");
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

let refreshing = false;
let activeDetail: JobDetail | null = null;
let activeMedia: HTMLMediaElement | null = null;
let activeSegmentId: string | null = null;
let workspaceActionBusy = false;
let lastExportedSubtitlePath: string | null = null;
let glossaries: Glossary[] = [];
let editingGlossaryId: string | null = null;
let pendingGlossaryPreview: GlossaryPreview | null = null;
let renamingJob: LocalJob | null = null;
let confirmationResolver: ((confirmed: boolean) => void) | null = null;
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
let translationStatus: TranslationStatus = {
  provider: "DeepL",
  configured: false,
  source_language: "ja",
  target_language: "zh-hans",
};

function displayName(job: LocalJob): string {
  if (job.display_name?.trim()) return job.display_name;
  const source = job.input_path?.split("/").pop();
  return source || job.job_id;
}

function statusLabel(status: string): string {
  return status.split("_").join(" ");
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

function renderGlossaryOptions(): void {
  const taskSelection = taskGlossary?.value ?? "";
  const workspaceSelection = workspaceGlossary?.value ?? "";
  const options = glossaries
    .map((glossary) => `<option value="${escapeHtml(glossary.id)}">${escapeHtml(glossary.name)}（核心 ${glossary.core_term_count}／内容包 ${glossary.content_group_count}／仅修正 ${glossary.correction_only_count}）</option>`)
    .join("");
  if (taskGlossary) {
    taskGlossary.innerHTML = `<option value="">不使用词表</option>${options}`;
    if (glossaries.some((glossary) => glossary.id === taskSelection)) taskGlossary.value = taskSelection;
  }
  if (workspaceGlossary) {
    workspaceGlossary.innerHTML = `<option value="">选择词表…</option>${options}`;
    const preferred = glossaries.some((glossary) => glossary.id === workspaceSelection)
      ? workspaceSelection
      : activeDetail?.job.glossary_id ?? "";
    workspaceGlossary.value = glossaries.some((glossary) => glossary.id === preferred) ? preferred : "";
  }
  renderGlossaryList();
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
      (glossary) => `<button type="button" class="glossary-list-item${glossary.id === editingGlossaryId ? " active" : ""}" data-glossary-id="${escapeHtml(glossary.id)}"><strong>${escapeHtml(glossary.name)}</strong><span>核心 ${glossary.core_term_count} · 内容 ${glossary.content_group_count} 包 · 仅修正 ${glossary.correction_only_count}</span></button>`,
    )
    .join("");
  glossaryListHost.querySelectorAll<HTMLButtonElement>("[data-glossary-id]").forEach((button) => {
    button.addEventListener("click", () => void editGlossary(button.dataset.glossaryId ?? null));
  });
}

function updateTranslationControls(): void {
  const hasSegments = (activeDetail?.segments.length ?? 0) > 0;
  if (translationStatusText) {
    translationStatusText.textContent = translationStatus.configured
      ? `${translationStatus.provider} 已配置 · 日文会发送到云端翻译为简体中文`
      : `未配置 ${translationStatus.provider}；请设置 DEEPL_AUTH_KEY 后重启应用`;
    translationStatusText.classList.toggle("warning", !translationStatus.configured);
  }
  if (translateAllButton) {
    translateAllButton.disabled = workspaceActionBusy || !hasSegments || !translationStatus.configured;
  }
  if (exportButton) exportButton.disabled = workspaceActionBusy || !hasSegments;
  if (renderVideoButton) {
    const inputPath = activeDetail?.job.input_path;
    renderVideoButton.disabled =
      workspaceActionBusy || !hasSegments || !inputPath || isAudioPath(inputPath);
    renderVideoButton.title = inputPath && isAudioPath(inputPath) ? "音频任务不能烧录视频" : "";
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

function setWorkspaceBusy(busy: boolean): void {
  workspaceActionBusy = busy;
  updateTranslationControls();
}

function hasUnsavedSubtitleEdits(): boolean {
  return Boolean(subtitleList?.querySelector<HTMLElement>('[data-dirty="true"]'));
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
            <div><h3>${escapeHtml(displayName(job))}</h3><p>${escapeHtml(job.message)}</p></div>
            <span class="status status-${escapeHtml(job.status)}">${escapeHtml(statusLabel(job.status))}</span>
          </button>
          <div class="job-actions">
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
  jobList.querySelectorAll<HTMLButtonElement>("[data-delete-job]").forEach((button) => {
    button.addEventListener("click", () => {
      const job = jobs.find((item) => item.job_id === button.dataset.deleteJob);
      if (job) void deleteJob(job);
    });
  });
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
  if (!activeDetail) jobList.innerHTML = `<div class="empty-state">正在读取任务…</div>`;
  try {
    const jobs = await invoke<LocalJob[]>("list_jobs");
    renderJobs(jobs);
  } catch (error) {
    jobList.innerHTML = `<div class="empty-state error">无法读取本地任务：${escapeHtml(String(error))}</div>`;
  } finally {
    refreshing = false;
  }
}

function showWorkspace(show: boolean): void {
  homeView?.classList.toggle("hidden", show);
  workspaceView?.classList.toggle("hidden", !show);
  document.querySelector<HTMLButtonElement>("#refresh")?.classList.toggle("hidden", show);
}

async function openJob(jobId: string): Promise<void> {
  if (!jobId || !workspaceMessage) return;
  showWorkspace(true);
  window.scrollTo({ top: 0, behavior: "auto" });
  workspaceMessage.textContent = "正在读取 SQLite 字幕工作区…";
  setWorkspaceAction("");
  if (subtitleList) subtitleList.innerHTML = `<div class="empty-state">正在读取字幕…</div>`;
  try {
    activeDetail = await invoke<JobDetail>("get_job_detail", { jobId });
    renderWorkspace(activeDetail);
  } catch (error) {
    activeDetail = null;
    workspaceMessage.textContent = `无法打开任务：${String(error)}`;
  }
}

function renderWorkspace(detail: JobDetail): void {
  lastExportedSubtitlePath = null;
  revealExportButton?.classList.add("hidden");
  if (workspaceTitle) workspaceTitle.textContent = displayName(detail.job);
  if (workspaceMessage) {
    workspaceMessage.textContent = `${statusLabel(detail.job.status)} · ${detail.job.message}`;
  }
  if (segmentCount) segmentCount.textContent = `${detail.segments.length} 段`;
  if (jobGlossaryStatus) {
    jobGlossaryStatus.textContent = detail.job.glossary_name
      ? `转写时使用：${detail.job.glossary_name}（已保存任务快照）`
      : "转写时未使用识别词表";
  }
  if (workspaceGlossary && detail.job.glossary_id && glossaries.some((item) => item.id === detail.job.glossary_id)) {
    workspaceGlossary.value = detail.job.glossary_id;
  }
  clearGlossaryPreview();
  mountMedia(detail.playback_path, detail.audio_fallback_path);
  renderSubtitleList(detail.segments);
  updateActiveSubtitle(0);
  updateTranslationControls();
  void refreshVideoRenders();
}

function isAudioPath(path: string): boolean {
  return /\.(mp3|m4a|aac|wav|flac|ogg)$/i.test(path);
}

function mountMedia(primaryPath: string | null, fallbackPath: string | null): void {
  if (!mediaHost || !mediaMessage) return;
  activeMedia = null;
  mediaHost.replaceChildren();
  const firstPath = primaryPath ?? fallbackPath;
  if (!firstPath) {
    mediaMessage.textContent = "没有找到可播放的本地媒体；任务可能仍在处理或源文件已移动。";
    return;
  }

  const loadPath = (path: string, isFallback: boolean): void => {
    const element = document.createElement(isAudioPath(path) ? "audio" : "video");
    element.controls = true;
    element.preload = "metadata";
    element.src = convertFileSrc(path);
    element.addEventListener("timeupdate", () => updateActiveSubtitle(element.currentTime * 1_000));
    element.addEventListener("seeked", () => updateActiveSubtitle(element.currentTime * 1_000));
    element.addEventListener(
      "error",
      () => {
        if (!isFallback && fallbackPath && fallbackPath !== path) {
          mediaMessage.textContent = "原媒体编码无法由系统播放器读取，已切换到任务音频。";
          loadPath(fallbackPath, true);
        } else {
          mediaMessage.textContent = "媒体加载失败。可能是文件已移动，或系统 WebView 不支持这种编码。";
        }
      },
      { once: true },
    );
    activeMedia = element;
    mediaHost.replaceChildren(element);
    if (!isFallback) mediaMessage.textContent = path;
  };

  loadPath(firstPath, firstPath === fallbackPath && primaryPath === null);
}

function renderSubtitleList(segments: SubtitleSegment[]): void {
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

    const jaLabel = document.createElement("label");
    jaLabel.textContent = "日本語";
    const ja = document.createElement("textarea");
    ja.className = "subtitle-input ja-input";
    ja.rows = 2;
    ja.value = segment.ja_text;
    jaLabel.append(ja);

    const zhLabel = document.createElement("label");
    zhLabel.textContent = "简体中文";
    const zh = document.createElement("textarea");
    zh.className = "subtitle-input zh-input";
    zh.rows = 2;
    zh.placeholder = "尚无翻译";
    zh.value = segment.zh_text ?? "";
    zhLabel.append(zh);

    const footer = document.createElement("div");
    footer.className = "subtitle-footer";
    const state = document.createElement("span");
    state.textContent = segment.translation_stale ? "日文已改变，当前译文需要重译" : "已保存到本地 SQLite";
    if (segment.translation_stale) state.classList.add("warning");
    const save = document.createElement("button");
    save.type = "button";
    save.textContent = "保存本段";
    save.disabled = true;
    const translate = document.createElement("button");
    translate.type = "button";
    translate.className = "translate-segment secondary";
    translate.textContent = segment.zh_text ? "重译本段" : "翻译本段";
    const capture = document.createElement("button");
    capture.type = "button";
    capture.className = "capture-glossary-term secondary";
    capture.textContent = "修正加入词表";
    const markDirty = (): void => {
      card.dataset.dirty = "true";
      save.disabled = false;
      state.textContent = "有未保存的修改";
      state.classList.remove("warning");
    };
    ja.addEventListener("input", markDirty);
    zh.addEventListener("input", markDirty);
    save.addEventListener("click", () => void saveSegment(segment, ja, zh, save, state));
    translate.addEventListener("click", () => void translateSegment(segment, ja, zh, state));
    capture.addEventListener("click", () => void captureGlossaryCorrection(segment, ja, state));
    const actions = document.createElement("div");
    actions.className = "subtitle-actions";
    actions.append(capture, translate, save);
    footer.append(state, actions);

    card.append(meta, jaLabel, zhLabel, footer);
    subtitleList.append(card);
  }
  highlightSegment(activeSegmentId);
  updateTranslationControls();
}

async function saveSegment(
  segment: SubtitleSegment,
  ja: HTMLTextAreaElement,
  zh: HTMLTextAreaElement,
  button: HTMLButtonElement,
  state: HTMLSpanElement,
): Promise<void> {
  if (!activeDetail) return;
  button.disabled = true;
  state.textContent = "正在保存…";
  try {
    const updated = await persistSegment(segment.id, ja.value, zh.value);
    replaceActiveSegment(updated);
    renderSubtitleList(activeDetail.segments);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
  } catch (error) {
    state.textContent = `保存失败：${String(error)}`;
    state.classList.add("warning");
    button.disabled = false;
  }
}

async function persistSegment(segmentId: string, jaText: string, zhText: string): Promise<SubtitleSegment> {
  if (!activeDetail) throw new Error("没有打开的字幕任务");
  return invoke<SubtitleSegment>("update_subtitle", {
    request: {
      jobId: activeDetail.job.job_id,
      segmentId,
      jaText,
      zhText: zhText.trim() || null,
    },
  });
}

function replaceActiveSegment(updated: SubtitleSegment): void {
  if (!activeDetail) return;
  activeDetail.segments = activeDetail.segments.map((item) => (item.id === updated.id ? updated : item));
}

async function translateSegment(
  segment: SubtitleSegment,
  ja: HTMLTextAreaElement,
  zh: HTMLTextAreaElement,
  state: HTMLSpanElement,
): Promise<void> {
  if (!activeDetail || workspaceActionBusy || !translationStatus.configured) return;
  setWorkspaceBusy(true);
  state.textContent = "正在保存并发送本段到 DeepL…";
  state.classList.remove("warning");
  try {
    const saved = await persistSegment(segment.id, ja.value, zh.value);
    replaceActiveSegment(saved);
    const translated = await invoke<SubtitleSegment>("translate_subtitle", {
      jobId: activeDetail.job.job_id,
      segmentId: segment.id,
    });
    replaceActiveSegment(translated);
    setWorkspaceAction("本段中文已由 DeepL 更新并保存到 SQLite。");
    renderSubtitleList(activeDetail.segments);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
  } catch (error) {
    state.textContent = `翻译失败：${String(error)}`;
    state.classList.add("warning");
    setWorkspaceAction(`翻译失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

function editFlags(segment: SubtitleSegment): string {
  const flags = [];
  if (segment.source_edited) flags.push("日文已编辑");
  if (segment.translation_edited) flags.push("中文已编辑");
  if (segment.translation_stale) flags.push("待重译");
  return flags.join(" · ");
}

function formatTime(milliseconds: number): string {
  const totalSeconds = milliseconds / 1_000;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = Math.floor(totalSeconds % 60);
  const tenths = Math.floor((milliseconds % 1_000) / 100);
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${tenths}`;
}

function seekTo(milliseconds: number): void {
  if (!activeMedia) return;
  activeMedia.currentTime = milliseconds / 1_000;
  void activeMedia.play().catch(() => undefined);
}

function updateActiveSubtitle(milliseconds: number): void {
  const segment = activeDetail?.segments.find(
    (item) => milliseconds >= item.start_ms && milliseconds < item.end_ms,
  );
  const nextId = segment?.id ?? null;
  if (currentJa) currentJa.textContent = segment?.ja_text ?? "当前时间没有字幕。";
  if (currentZh) {
    currentZh.textContent = segment?.zh_text || "尚无中文翻译";
    currentZh.classList.toggle("stale", segment?.translation_stale ?? false);
  }
  if (activeSegmentId !== nextId) {
    activeSegmentId = nextId;
    highlightSegment(nextId);
  }
}

function highlightSegment(segmentId: string | null): void {
  subtitleList?.querySelectorAll<HTMLElement>(".subtitle-card").forEach((card) => {
    const active = card.dataset.segmentId === segmentId;
    card.classList.toggle("active", active);
    if (active && activeMedia && !activeMedia.paused) card.scrollIntoView({ block: "nearest", behavior: "smooth" });
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
    message: `将把 ${activeDetail.segments.length} 段日文发送到 DeepL，并覆盖现有中文，包括人工修改。`,
    confirmLabel: "发送并覆盖",
    danger: true,
  });
  if (!confirmed) return;

  setWorkspaceBusy(true);
  setWorkspaceAction(`正在通过 DeepL 翻译 ${activeDetail.segments.length} 段字幕…`);
  try {
    activeDetail.segments = await invoke<SubtitleSegment[]>("translate_all_subtitles", {
      jobId: activeDetail.job.job_id,
    });
    setWorkspaceAction(`已翻译 ${activeDetail.segments.length} 段并原子写入 SQLite。`);
    renderSubtitleList(activeDetail.segments);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
  } catch (error) {
    setWorkspaceAction(`全部翻译失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

async function exportSubtitles(): Promise<void> {
  if (!activeDetail || workspaceActionBusy) return;
  if (hasUnsavedSubtitleEdits()) {
    setWorkspaceAction("请先保存各段尚未保存的修改，再导出字幕。", true);
    return;
  }
  const staleCount = activeDetail.segments.filter((segment) => segment.translation_stale).length;
  if (staleCount > 0) {
    setWorkspaceAction(`有 ${staleCount} 段中文已过期，请重译或修正后再导出。`, true);
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

    setWorkspaceAction("正在从 SQLite 当前内容生成日文、中文和双语字幕…");
    const exported = await invoke<SubtitleExport>("export_workspace_subtitles", {
      request: {
        jobId: activeDetail.job.job_id,
        outputDirectory,
        overwriteExisting,
      },
    });
    const missing = exported.missing_translation_count
      ? `；${exported.missing_translation_count} 段尚无中文，双语字幕中保留日文`
      : "";
    lastExportedSubtitlePath = exported.bilingual_ass;
    revealExportButton?.classList.remove("hidden");
    setWorkspaceAction(`已导出 4 个字幕文件到：${outputDirectory}${missing}`);
  } catch (error) {
    setWorkspaceAction(`导出失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

async function revealExportedSubtitle(): Promise<void> {
  if (!lastExportedSubtitlePath || workspaceActionBusy) return;
  setWorkspaceBusy(true);
  try {
    await invoke("reveal_exported_subtitle", { path: lastExportedSubtitlePath });
  } catch (error) {
    setWorkspaceAction(`Finder 定位失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

function videoTrackLabel(track: VideoRender["subtitle_track"]): string {
  if (track === "japanese") return "仅日文";
  if (track === "chinese") return "仅中文";
  return "日中双语";
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
        ? `${render.encoder === "videotoolbox" ? "VideoToolbox" : "libx264"} · 音频 ${render.audio_encoder ?? "未知"}${render.fallback_reason ? ` · ${render.fallback_reason}` : ""}`
        : render.error_message ?? videoTrackLabel(render.subtitle_track);
      const actions = render.status === "queued" || render.status === "running"
        ? `<button type="button" class="danger" data-cancel-render="${escapeHtml(render.id)}">取消</button>`
        : render.status === "done"
          ? `<button type="button" class="secondary" data-reveal-render="${escapeHtml(render.output_path)}">在 Finder 中显示</button>`
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
    mediaCapabilitiesHost.textContent = "正在检查 ffmpeg-full…";
    mediaCapabilitiesHost.classList.remove("warning");
    return;
  }
  if (submitVideoRenderButton) {
    submitVideoRenderButton.disabled = videoRenderSubmitting || !mediaCapabilities.ready_for_hard_subtitles;
  }
  const encoder = mediaCapabilities.videotoolbox_encoder
    ? "VideoToolbox 可用"
    : mediaCapabilities.libx264_encoder
      ? "仅 libx264"
      : "没有可用 H.264 编码器";
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
    setWorkspaceAction(`有 ${staleCount} 段中文已过期，请重译或修正后再烧录。`, true);
    return;
  }
  if (!activeDetail.job.input_path || isAudioPath(activeDetail.job.input_path)) {
    setWorkspaceAction("当前任务没有可烧录的视频源。", true);
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
    if (videoRenderMessage) videoRenderMessage.textContent = `Finder 定位失败：${String(error)}`;
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
    source.placeholder = correction ? "日语读音或常见误识别" : "希望 Whisper 识别出的写法";
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
    if (deleteGlossaryButton) deleteGlossaryButton.disabled = true;
    addGlossaryTermRow();
    glossaryName?.focus();
    return;
  }

  if (glossaryMessage) glossaryMessage.textContent = "正在读取词表…";
  try {
    const detail = await invoke<GlossaryDetail>("get_glossary", { glossaryId });
    if (glossaryName) glossaryName.value = detail.glossary.name;
    if (deleteGlossaryButton) deleteGlossaryButton.disabled = false;
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
      glossaryMessage.textContent = `核心 ${detail.glossary.core_term_count} 条 · 内容 ${detail.glossary.content_term_count} 条／${detail.glossary.content_group_count} 包 · 仅修正 ${detail.glossary.correction_only_count} 条`;
    }
  } catch (error) {
    if (glossaryMessage) glossaryMessage.textContent = `读取失败：${String(error)}`;
  }
}

async function openGlossaryManager(): Promise<void> {
  await refreshGlossaries();
  glossaryDialog?.showModal();
  const preferred = taskGlossary?.value || glossaries[0]?.id || null;
  await editGlossary(preferred);
}

async function saveGlossaryEditor(): Promise<void> {
  if (!glossaryName || !glossaryTerms || !glossaryMessage) return;
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
    const detail = await invoke<GlossaryDetail>("save_glossary", {
      request: { glossaryId: editingGlossaryId, name, terms },
    });
    editingGlossaryId = detail.glossary.id;
    await refreshGlossaries();
    if (taskGlossary) taskGlossary.value = detail.glossary.id;
    await refreshTaskGlossaryConfiguration();
    glossaryMessage.textContent = `已保存 ${detail.terms.length} 条词条。`;
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
  ja: HTMLTextAreaElement,
  state: HTMLSpanElement,
): void {
  const glossaryId = workspaceGlossary?.value;
  if (!glossaryId) {
    setWorkspaceAction("请先在工作区选择一个词表。", true);
    return;
  }
  pendingGlossaryCorrection = { glossaryId, state };
  if (glossaryCorrectionSource) glossaryCorrectionSource.value = segment.ja_text;
  if (glossaryCorrectionTarget) glossaryCorrectionTarget.value = ja.value.trim();
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
    await invoke<GlossaryDetail>("save_glossary", {
      request: {
        glossaryId,
        name: detail.glossary.name,
        terms,
      },
    });
    await refreshGlossaries();
    if (workspaceGlossary) workspaceGlossary.value = glossaryId;
    setWorkspaceAction(`已把“${sourceText} → ${targetText}”加入 ${detail.glossary.name}。字幕修改仍需单独保存。`);
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
  setWorkspaceAction("正在比较词表与 SQLite 日文字幕…");
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
    ? `<div><strong>${escapeHtml(preview.glossary_name)} 将修改 ${preview.changes.length} 段</strong><span>${staleCount} 段已有中文，应用后会标记为待重译。</span></div><ul>${examples}</ul><button id="apply-previewed-glossary" type="button">确认应用</button>`
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
    renderSubtitleList(activeDetail.segments);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
    setWorkspaceAction(
      `已修正 ${applied.changed_segments} 段并保存到 SQLite；${applied.stale_translations} 段中文需要重译。`,
    );
  } catch (error) {
    setWorkspaceAction(`应用词表失败：${String(error)}`, true);
  } finally {
    setWorkspaceBusy(false);
  }
}

document.querySelector<HTMLButtonElement>("#refresh")?.addEventListener("click", () => void refresh());
document.querySelector<HTMLButtonElement>("#back-to-jobs")?.addEventListener("click", () => {
  activeMedia?.pause();
  activeDetail = null;
  activeMedia = null;
  activeSegmentId = null;
  showWorkspace(false);
  void refresh();
});
document.querySelector<HTMLButtonElement>("#reload-detail")?.addEventListener("click", () => {
  if (activeDetail) void openJob(activeDetail.job.job_id);
});
translateAllButton?.addEventListener("click", () => void translateAllSubtitles());
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
document.querySelector<HTMLButtonElement>("#manage-glossaries")?.addEventListener("click", () => void openGlossaryManager());
document.querySelector<HTMLButtonElement>("#close-glossaries")?.addEventListener("click", () => glossaryDialog?.close());
document.querySelector<HTMLButtonElement>("#new-glossary")?.addEventListener("click", () => void editGlossary(null));
document.querySelector<HTMLButtonElement>("#add-glossary-term")?.addEventListener("click", () => addGlossaryTermRow());
document.querySelector<HTMLButtonElement>("#save-glossary")?.addEventListener("click", () => void saveGlossaryEditor());
deleteGlossaryButton?.addEventListener("click", () => void deleteGlossaryEditor());
document.querySelector<HTMLButtonElement>("#preview-glossary")?.addEventListener("click", () => void previewGlossaryApplication());
workspaceGlossary?.addEventListener("change", () => {
  clearGlossaryPreview();
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

syncVadControls();
void invoke<RecognitionDefaults>("recognition_defaults")
  .then((defaults) => {
    if (modelPath && !modelPath.value && defaults.whisperModelPath) modelPath.value = defaults.whisperModelPath;
    if (vadModelPath && !vadModelPath.value && defaults.vadModelPath) vadModelPath.value = defaults.vadModelPath;
  })
  .catch(() => {
    // Manual path entry and the native pickers remain available.
  });

void invoke<string>("data_directory")
  .then((path) => {
    if (dataPath) dataPath.textContent = `本地数据：${path}`;
  })
  .catch(() => {
    if (dataPath) dataPath.textContent = "本地数据目录暂不可用。";
  });
void invoke<TranslationStatus>("translation_status")
  .then((status) => {
    translationStatus = status;
    updateTranslationControls();
  })
  .catch((error) => {
    setWorkspaceAction(`无法读取翻译配置：${String(error)}`, true);
    updateTranslationControls();
  });
void refreshGlossaries();
void refresh();
window.setInterval(() => void refresh(), 2_000);
window.setInterval(() => {
  if (activeDetail && videoRenders.some((render) => render.status === "queued" || render.status === "running")) {
    void refreshVideoRenders();
  }
}, 1_000);
