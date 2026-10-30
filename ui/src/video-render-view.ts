import { videoRenderTemplate } from "./video-render-template";
import { mountToolDialog } from "./task-tools";
import type { createVideoRenderSession } from "./video-render-session";
import type { VideoRender, VideoRenderQuality } from "./video-render-types";
type Session = ReturnType<typeof createVideoRenderSession>;
/** One retained tool instance. Session owns all requests and polling. */
export function mountVideoRenderView(root: HTMLElement, options: {
  context(): { name: string; source: string; target: string } | null;
  platform: string; fileManager: string;
}) {
  const template = document.createElement("template"); template.innerHTML = videoRenderTemplate(options.platform);
  const dialog = template.content.firstElementChild as HTMLDialogElement; root.append(dialog);
  const query = <T extends HTMLElement>(selector: string) => dialog.querySelector<T>(selector)!;
  const track = query<HTMLSelectElement>("#video-subtitle-track"), quality = query<HTMLSelectElement>("#video-render-quality");
  const path = query<HTMLInputElement>("#video-output-path"), message = query("#video-render-message");
  const capabilities = query("#media-capabilities"), estimate = query("#video-render-estimate"), list = query("#video-render-list");
  const submit = query<HTMLButtonElement>("#submit-video-render"), choose = query<HTMLButtonElement>("#choose-video-output");
  const events = new AbortController(); let session: Session | null = null, disposed = false, lastAction: HTMLButtonElement | null = null;
  const confirm = document.createElement("dialog"); confirm.id = "video-render-confirmation"; confirm.className = "rename-job-dialog video-render-confirmation";
  confirm.innerHTML = '<h2>确认烧录操作</h2><p></p><div class="confirmation-actions"><button type="button" class="secondary" data-cancel-render-confirmation>取消</button><button type="button" class="danger" data-accept-render-confirmation>继续</button></div>'; root.append(confirm);
  let resolveConfirm: ((accepted: boolean) => void) | null = null;
  const settle = (accepted: boolean) => { const resolve = resolveConfirm; resolveConfirm = null; confirmBoundary.close(); resolve?.(accepted); };
  const confirmBoundary = mountToolDialog(confirm, () => { const resolve = resolveConfirm; resolveConfirm = null; resolve?.(false); });
  const boundary = mountToolDialog(dialog, () => { settle(false); session?.close(); });
  const rows = new Map<string, { node: HTMLElement; status: HTMLElement; details: HTMLElement; path: HTMLElement; progress: HTMLProgressElement; action: HTMLButtonElement }>();
  const text = (tag: string, value: string) => { const node = document.createElement(tag); node.textContent = value; return node; };
  const label = (value: VideoRender["subtitle_track"]) => value === "source" ? `仅${options.context()?.source ?? "原文"}原文` : value === "translation" ? `仅${options.context()?.target ?? "译文"}译文` : "原文＋译文";
  const suggestedName = () => {
    const safe = (options.context()?.name ?? "Atogaki").replace(/[\\/:*?"<>|\u0000-\u001f]/g, "_").replace(/[. ]+$/g, "").slice(0, 100) || "Atogaki";
    return `${safe}.${track.value}.mp4`;
  };
  const renderEstimate = (error?: string) => {
    const value = session?.estimates.find(item => item.quality === quality.value);
    estimate.classList.toggle("warning", !!error || value?.bitrate_source === "resolution_fallback");
    if (error || !value) { estimate.textContent = error ?? "正在读取源视频规格…"; return; }
    const resolution = value.width && value.height ? `${value.width} × ${value.height}（保持原分辨率）` : "保持原分辨率";
    const size = value.estimated_size_bytes >= 1024 ** 3 ? `${(value.estimated_size_bytes / 1024 ** 3).toFixed(2)} GB` : `${Math.max(0, value.estimated_size_bytes / 1024 ** 2).toFixed(0)} MB`;
    estimate.replaceChildren(text("strong", `${resolution} · 目标视频码率 ${(value.target_video_bitrate_bps / 1_000_000).toFixed(2)} Mbps`), text("span", `预计成品约 ${size} · ${value.bitrate_source === "source_bitrate" ? "按源视频码率计算" : "源文件未报告视频码率，按分辨率基准计算"}`));
  };
  const renderCapabilities = (error?: string) => {
    const value = session?.capabilities;
    capabilities.classList.toggle("warning", !!error || !!value && !value.ready_for_hard_subtitles);
    if (error || !value) { capabilities.textContent = error ?? "正在检查内置 FFmpeg sidecar…"; return; }
    const encoder = [options.platform === "macos" ? value.videotoolbox_encoder ? "VideoToolbox 可用" : "VideoToolbox 不可用" : "", value.mpeg4_encoder ? options.platform === "macos" ? "MPEG-4 回退可用" : "MPEG-4 软件编码可用" : options.platform === "macos" ? "MPEG-4 回退缺失" : "MPEG-4 软件编码缺失"].filter(Boolean).join(" · ");
    capabilities.replaceChildren(text("strong", value.ready_for_hard_subtitles ? "烧录环境就绪" : "烧录环境不可用"), text("span", `${encoder} · libass ${value.ass_filter ? "可用" : "缺失"}`), text("code", value.binary_path), text("small", value.version));
  };
  const history = () => {
    const renders = session?.renders ?? [], ids = new Set(renders.map(item => item.id));
    query("#video-render-count").textContent = `${renders.length} 条`;
    for (const [id, row] of rows) if (!ids.has(id)) { row.node.remove(); rows.delete(id); }
    list.querySelector(".empty-state")?.remove();
    if (!renders.length) { const empty = text("div", "还没有烧录记录。"); empty.className = "empty-state"; list.append(empty); return; }
    for (const [index, render] of renders.entries()) {
      let row = rows.get(render.id);
      if (!row) {
        const node = document.createElement("article"); node.className = "video-render-row"; const main = document.createElement("div"); main.className = "video-render-row-main";
        const status = document.createElement("strong"), details = document.createElement("span"), output = document.createElement("code"), progress = document.createElement("progress"), action = document.createElement("button"); progress.max = 1; action.type = "button";
        main.append(status, details, output, progress); node.append(main, action); row = { node, status, details, path: output, progress, action }; rows.set(render.id, row);
      }
      row.status.textContent = render.status === "queued" ? "等待烧录" : render.status === "running" ? `烧录中 ${Math.round(render.progress * 100)}%` : render.status === "done" ? "已完成" : render.status === "cancelled" ? "已取消" : "失败";
      row.details.textContent = render.status === "done" ? `${render.encoder === "videotoolbox" ? "VideoToolbox" : render.encoder === "mpeg4" ? "MPEG-4 软件编码" : render.encoder ?? "未知编码器"} · 音频 ${render.audio_encoder ?? "未知"}${render.fallback_reason ? ` · ${render.fallback_reason}` : ""}` : render.error_message ?? label(render.subtitle_track);
      row.path.textContent = render.output_path; row.progress.value = render.progress; row.progress.hidden = render.status !== "running";
      delete row.action.dataset.cancelRender; delete row.action.dataset.revealRender;
      const active = render.status === "queued" || render.status === "running";
      row.action.hidden = !active && render.status !== "done"; row.action.className = active ? "danger" : "secondary";
      row.action.textContent = active ? "取消" : `在 ${options.fileManager} 中显示`;
      if (active) row.action.dataset.cancelRender = render.id; else if (render.status === "done") row.action.dataset.revealRender = render.output_path;
      if (list.children[index] !== row.node) list.insertBefore(row.node, list.children[index] ?? null);
    }
  };
  dialog.addEventListener("click", event => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button"); if (!button || !session) return;
    lastAction = button; button.focus();
    if (button.id === "close-video-render") session.close();
    else if (button.id === "choose-video-output") void session.chooseOutput();
    else if (button.id === "submit-video-render") void session.submit();
    else if (button.dataset.cancelRender) void session.cancel(button.dataset.cancelRender);
    else if (button.dataset.revealRender) void session.reveal(button.dataset.revealRender);
  }, { signal: events.signal });
  path.addEventListener("input", () => session?.outputChanged(), { signal: events.signal });
  track.addEventListener("change", () => { session?.outputChanged(); if (!path.value) path.placeholder = suggestedName(); }, { signal: events.signal });
  quality.addEventListener("change", () => { session?.outputChanged(); renderEstimate(); }, { signal: events.signal });
  confirm.querySelector("[data-cancel-render-confirmation]")!.addEventListener("click", () => settle(false), { signal: events.signal });
  confirm.querySelector("[data-accept-render-confirmation]")!.addEventListener("click", () => settle(true), { signal: events.signal });
  return {
    suggestedName,
    open() {
      if (disposed) return;
      track.querySelector('option[value="source"]')!.textContent = `仅${options.context()?.source ?? "原文"}原文`;
      track.querySelector('option[value="translation"]')!.textContent = `仅${options.context()?.target ?? "译文"}译文`;
      path.placeholder = suggestedName(); boundary.open();
    },
    close() { settle(false); boundary.close(); },
    read: () => ({ outputPath: path.value, track: track.value as VideoRender["subtitle_track"], quality: quality.value as VideoRenderQuality }),
    output(value: string) { path.value = value; }, message(value: string) { message.textContent = value; },
    capabilities: renderCapabilities, estimates: renderEstimate, history,
    busy(value: boolean, canSubmit: boolean) {
      submit.disabled = !canSubmit; choose.disabled = value;
      if (document.activeElement === document.body && lastAction?.isConnected && !lastAction.disabled && !lastAction.hidden && !lastAction.closest("[inert]")) lastAction.focus();
    },
    confirm(value: { title: string; message: string; confirmLabel: string; danger: boolean }): Promise<boolean> {
      if (disposed) return Promise.resolve(false); settle(false);
      confirm.querySelector("h2")!.textContent = value.title; confirm.setAttribute("aria-label", value.title); confirm.querySelector("p")!.textContent = value.message;
      confirm.querySelector("[data-accept-render-confirmation]")!.textContent = value.confirmLabel;
      return new Promise(resolve => { resolveConfirm = resolve; confirmBoundary.open(); });
    },
    bind(controller: Session, trigger: HTMLButtonElement | null) { session = controller; trigger?.addEventListener("click", () => { trigger.focus(); void session?.open(); }, { signal: events.signal }); },
    dispose() { if (disposed) return; disposed = true; settle(false); events.abort(); confirmBoundary.dispose(); boundary.dispose(); rows.clear(); dialog.remove(); confirm.remove(); session = null; },
  };
}
