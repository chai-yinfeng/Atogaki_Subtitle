import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type LocalJob = {
  job_id: string;
  storage_dir: string;
  input_path: string | null;
  status: string;
  message: string;
  error_message: string | null;
  updated_at_unix: number;
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
          <label>媒体文件<input id="media-path" required readonly placeholder="选择音频或视频文件" /></label>
          <button id="choose-media" type="button" class="secondary">选择媒体</button>
          <label>Whisper 模型<input id="model-path" required readonly placeholder="选择 ggml 模型文件" /></label>
          <button id="choose-model" type="button" class="secondary">选择模型</button>
          <div class="form-footer"><span id="task-message" role="status"></span><button id="submit-task" type="submit">开始转写</button></div>
        </form>
      </section>
      <section class="jobs" aria-labelledby="jobs-heading">
        <div class="section-heading">
          <h2 id="jobs-heading">最近任务</h2>
          <span id="job-count"></span>
        </div>
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
        <button id="reload-detail" type="button">重新读取</button>
      </div>
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
  </main>
`;

const homeView = document.querySelector<HTMLDivElement>("#home-view");
const workspaceView = document.querySelector<HTMLElement>("#workspace-view");
const jobList = document.querySelector<HTMLDivElement>("#job-list");
const jobCount = document.querySelector<HTMLSpanElement>("#job-count");
const dataPath = document.querySelector<HTMLParagraphElement>("#data-path");
const mediaPath = document.querySelector<HTMLInputElement>("#media-path");
const modelPath = document.querySelector<HTMLInputElement>("#model-path");
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

let refreshing = false;
let activeDetail: JobDetail | null = null;
let activeMedia: HTMLMediaElement | null = null;
let activeSegmentId: string | null = null;

function displayName(job: LocalJob): string {
  const source = job.input_path?.split("/").pop();
  return source || job.job_id;
}

function statusLabel(status: string): string {
  return status.split("_").join(" ");
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
        <button class="job-card" data-job-id="${escapeHtml(job.job_id)}" type="button">
          <div><h3>${escapeHtml(displayName(job))}</h3><p>${escapeHtml(job.message)}</p></div>
          <span class="status status-${escapeHtml(job.status)}">${escapeHtml(statusLabel(job.status))}</span>
        </button>`,
    )
    .join("");
  jobList.querySelectorAll<HTMLButtonElement>("[data-job-id]").forEach((button) => {
    button.addEventListener("click", () => void openJob(button.dataset.jobId ?? ""));
  });
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
  workspaceMessage.textContent = "正在读取 SQLite 字幕工作区…";
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
  if (workspaceTitle) workspaceTitle.textContent = displayName(detail.job);
  if (workspaceMessage) {
    workspaceMessage.textContent = `${statusLabel(detail.job.status)} · ${detail.job.message}`;
  }
  if (segmentCount) segmentCount.textContent = `${detail.segments.length} 段`;
  mountMedia(detail.playback_path, detail.audio_fallback_path);
  renderSubtitleList(detail.segments);
  updateActiveSubtitle(0);
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
    const markDirty = (): void => {
      save.disabled = false;
      state.textContent = "有未保存的修改";
      state.classList.remove("warning");
    };
    ja.addEventListener("input", markDirty);
    zh.addEventListener("input", markDirty);
    save.addEventListener("click", () => void saveSegment(segment, ja, zh, save, state));
    footer.append(state, save);

    card.append(meta, jaLabel, zhLabel, footer);
    subtitleList.append(card);
  }
  highlightSegment(activeSegmentId);
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
    const updated = await invoke<SubtitleSegment>("update_subtitle", {
      request: {
        jobId: activeDetail.job.job_id,
        segmentId: segment.id,
        jaText: ja.value,
        zhText: zh.value.trim() || null,
      },
    });
    activeDetail.segments = activeDetail.segments.map((item) => (item.id === updated.id ? updated : item));
    renderSubtitleList(activeDetail.segments);
    updateActiveSubtitle((activeMedia?.currentTime ?? 0) * 1_000);
  } catch (error) {
    state.textContent = `保存失败：${String(error)}`;
    state.classList.add("warning");
    button.disabled = false;
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

async function chooseFile(kind: "media" | "model"): Promise<void> {
  const button = document.querySelector<HTMLButtonElement>(
    kind === "media" ? "#choose-media" : "#choose-model",
  );
  if (button) button.disabled = true;
  if (taskMessage) taskMessage.textContent = kind === "media" ? "正在打开媒体选择器…" : "正在打开模型选择器…";
  try {
    const path = await open({
      multiple: false,
      directory: false,
      filters:
        kind === "media"
          ? [{ name: "媒体", extensions: ["mp3", "m4a", "wav", "mp4", "mkv", "webm", "mov"] }]
          : [{ name: "Whisper 模型", extensions: ["bin"] }],
    });
    if (typeof path === "string") {
      (kind === "media" ? mediaPath : modelPath)!.value = path;
      if (taskMessage) taskMessage.textContent = kind === "media" ? "已选择媒体文件。" : "已选择 Whisper 模型。";
    } else if (taskMessage) {
      taskMessage.textContent = "已取消选择。";
    }
  } catch (error) {
    if (taskMessage) taskMessage.textContent = `无法打开文件选择器：${String(error)}`;
  } finally {
    if (button) button.disabled = false;
  }
}

document.querySelector<HTMLButtonElement>("#choose-media")?.addEventListener("click", () => void chooseFile("media"));
document.querySelector<HTMLButtonElement>("#choose-model")?.addEventListener("click", () => void chooseFile("model"));

document.querySelector<HTMLFormElement>("#task-form")?.addEventListener("submit", (event) => {
  event.preventDefault();
  if (!mediaPath?.value || !modelPath?.value || !taskMessage || !submitButton) return;
  submitButton.disabled = true;
  taskMessage.textContent = "正在创建本地任务…";
  void invoke<string>("submit_transcription", {
    request: { inputPath: mediaPath.value, modelPath: modelPath.value },
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

void invoke<string>("data_directory")
  .then((path) => {
    if (dataPath) dataPath.textContent = `本地数据：${path}`;
  })
  .catch(() => {
    if (dataPath) dataPath.textContent = "本地数据目录暂不可用。";
  });
void refresh();
window.setInterval(() => void refresh(), 2_000);
