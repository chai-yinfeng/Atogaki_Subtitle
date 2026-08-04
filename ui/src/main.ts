import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type LocalJob = {
  job_id: string;
  input_path: string | null;
  status: string;
  message: string;
  error_message: string | null;
  updated_at_unix: number;
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
  </main>
`;

const jobList = document.querySelector<HTMLDivElement>("#job-list");
const jobCount = document.querySelector<HTMLSpanElement>("#job-count");
const dataPath = document.querySelector<HTMLParagraphElement>("#data-path");
const mediaPath = document.querySelector<HTMLInputElement>("#media-path");
const modelPath = document.querySelector<HTMLInputElement>("#model-path");
const taskMessage = document.querySelector<HTMLSpanElement>("#task-message");
const submitButton = document.querySelector<HTMLButtonElement>("#submit-task");

function displayName(job: LocalJob): string {
  const source = job.input_path?.split("/").at(-1);
  return source || job.job_id;
}

function statusLabel(status: string): string {
  return status.replaceAll("_", " ");
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
    jobList.innerHTML = `<div class="empty-state"><strong>还没有本地任务。</strong><span>下一步会在这里导入媒体并创建离线转写。</span></div>`;
    return;
  }

  jobList.innerHTML = jobs
    .map(
      (job) => `
        <article class="job-card">
          <div><h3>${escapeHtml(displayName(job))}</h3><p>${escapeHtml(job.message)}</p></div>
          <span class="status status-${escapeHtml(job.status)}">${escapeHtml(statusLabel(job.status))}</span>
        </article>`,
    )
    .join("");
}

async function refresh(): Promise<void> {
  if (!jobList) return;
  if (refreshing) return;
  refreshing = true;
  jobList.innerHTML = `<div class="empty-state">正在读取任务…</div>`;
  try {
    const jobs = await invoke<LocalJob[]>("list_jobs");
    renderJobs(jobs);
  } catch (error) {
    jobList.innerHTML = `<div class="empty-state error">无法读取本地任务：${String(error)}</div>`;
  } finally {
    refreshing = false;
  }
}

let refreshing = false;

document.querySelector<HTMLButtonElement>("#refresh")?.addEventListener("click", () => {
  void refresh();
});

async function chooseFile(kind: "media" | "model"): Promise<void> {
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
  }
}

document.querySelector<HTMLButtonElement>("#choose-media")?.addEventListener("click", () => {
  void chooseFile("media");
});
document.querySelector<HTMLButtonElement>("#choose-model")?.addEventListener("click", () => {
  void chooseFile("model");
});

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
