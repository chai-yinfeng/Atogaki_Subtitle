import { videoRenderTemplate } from "./video-render-template";
import { mountToolDialog } from "./task-tools";
import type { createVideoRenderSession } from "./video-render-session";
import { videoOutputExtension, videoProfileAvailable, type VideoExportMode, type VideoRender, type VideoRenderQuality } from "./video-render-types";
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
  const title = query("#video-render-title"), format = query("#video-render-format");
  const path = query<HTMLInputElement>("#video-output-path"), message = query("#video-render-message");
  const capabilities = query("#media-capabilities"), estimate = query("#video-render-estimate"), list = query("#video-render-list");
  const submit = query<HTMLButtonElement>("#submit-video-render"), choose = query<HTMLButtonElement>("#choose-video-output");
  const events = new AbortController(); let session: Session | null = null, disposed = false, lastAction: HTMLButtonElement | null = null;
  const confirm = document.createElement("dialog"); confirm.id = "video-render-confirmation"; confirm.className = "rename-job-dialog video-render-confirmation";
  confirm.innerHTML = '<h2>确认导出操作</h2><p></p><div class="confirmation-actions"><button type="button" class="secondary" data-cancel-render-confirmation>取消</button><button type="button" class="danger" data-accept-render-confirmation>继续</button></div>'; root.append(confirm);
  let resolveConfirm: ((accepted: boolean) => void) | null = null;
  const settle = (accepted: boolean) => { const resolve = resolveConfirm; resolveConfirm = null; confirmBoundary.close(); resolve?.(accepted); };
  const confirmBoundary = mountToolDialog(confirm, () => { const resolve = resolveConfirm; resolveConfirm = null; resolve?.(false); });
  const boundary = mountToolDialog(dialog, () => { settle(false); session?.close(); });
  let mode: VideoExportMode = "burn";
  const selectedQuality = () => mode === "soft_matroska" ? mode : quality.value as VideoRenderQuality;
  const rows = new Map<string, { node: HTMLElement; status: HTMLElement; details: HTMLElement; path: HTMLElement; progress: HTMLProgressElement; action: HTMLButtonElement }>();
  const text = (tag: string, value: string) => { const node = document.createElement(tag); node.textContent = value; return node; };
  const label = (value: VideoRender["subtitle_track"]) => value === "source" ? `仅${options.context()?.source ?? "原文"}原文` : value === "translation" ? `仅${options.context()?.target ?? "译文"}译文` : "原文＋译文";
  const suggestedName = () => {
    const safe = (options.context()?.name ?? "Atogaki").replace(/[\\/:*?"<>|\u0000-\u001f]/g, "_").replace(/[. ]+$/g, "").slice(0, 100) || "Atogaki";
    return `${safe}.${track.value}.${videoOutputExtension(selectedQuality())}`;
  };
  const renderEstimate = (error?: string) => {
    const profile = selectedQuality();
    const value = session?.estimates.find(item => item.quality === profile);
    const unavailable = !!session?.capabilities && !videoProfileAvailable(profile, session.capabilities);
    const hdr = value?.hdr_video && (profile === "quality" || profile === "prores");
    estimate.classList.toggle("warning", !!error || unavailable || !!hdr);
    if (error || !value) { estimate.textContent = error ?? "正在读取源视频规格…"; return; }
    if (unavailable) { estimate.textContent = "当前系统或内置 FFmpeg 不支持所选导出方式。"; return; }
    if (hdr) { estimate.textContent = "此烧录方式仅支持 SDR 片源。HDR 请选择 MKV 软字幕或独立字幕。"; return; }
    const resolution = value.width && value.height ? `${value.width} × ${value.height}` : "保留源分辨率";
    let description: string;
    if (value.quality === "soft_matroska") {
      description = "复制主视频和现有音轨，附加一条可开关的 ASS 字幕。播放器需支持 MKV 和 ASS。";
    } else if (value.quality === "quality") {
      description = "VideoToolbox H.264 按画质分配码率。成品大小随画面复杂度变化。";
    } else if (value.quality === "prores") {
      description = "烧录为 10-bit ProRes 422 HQ 和 PCM 音频。文件较大，导出后可用剪辑或转码工具再次压制。";
    } else {
      const size = value.estimated_size_bytes ?? 0;
      const formattedSize = size >= 1024 ** 3 ? `${(size / 1024 ** 3).toFixed(2)} GB` : `${(size / 1024 ** 2).toFixed(0)} MB`;
      const basis = value.bitrate_source === "source_bitrate" ? "按源码率估算" : "按分辨率估算";
      description = `目标视频码率 ${((value.target_video_bitrate_bps ?? 0) / 1_000_000).toFixed(2)} Mbps · 约 ${formattedSize} · ${basis}`;
    }
    estimate.replaceChildren(text("strong", resolution), text("span", description));
  };
  const renderCapabilities = (error?: string) => {
    capabilities.hidden = !error;
    capabilities.classList.toggle("warning", !!error);
    capabilities.textContent = error ?? "";
    if (!error && session?.capabilities && session.estimates.length) renderEstimate();
  };
  const history = () => {
    const renders = session?.renders ?? [], ids = new Set(renders.map(item => item.id));
    query("#video-render-count").textContent = `${renders.length} 条`;
    for (const [id, row] of rows) if (!ids.has(id)) { row.node.remove(); rows.delete(id); }
    list.querySelector(".empty-state")?.remove();
    if (!renders.length) { const empty = text("div", "还没有导出记录。"); empty.className = "empty-state"; list.append(empty); return; }
    for (const [index, render] of renders.entries()) {
      let row = rows.get(render.id);
      if (!row) {
        const node = document.createElement("article"); node.className = "video-render-row"; const main = document.createElement("div"); main.className = "video-render-row-main";
        const status = document.createElement("strong"), details = document.createElement("span"), output = document.createElement("code"), progress = document.createElement("progress"), action = document.createElement("button"); progress.max = 1; action.type = "button";
        main.append(status, details, output, progress); node.append(main, action); row = { node, status, details, path: output, progress, action }; rows.set(render.id, row);
      }
      row.status.textContent = render.status === "queued" ? "等待导出" : render.status === "running" ? `导出中 ${Math.round(render.progress * 100)}%` : render.status === "done" ? "已完成" : render.status === "cancelled" ? "已取消" : "失败";
      const encoders: Record<string, string> = { videotoolbox: "VideoToolbox", mpeg4: "MPEG-4 软件编码", prores: "ProRes 422 HQ", subtitle_mux: "ASS 软字幕 · 视频流复制" };
      row.details.textContent = render.status === "done" ? `${encoders[render.encoder ?? ""] ?? render.encoder ?? "未知编码器"} · 音频 ${render.audio_encoder ?? "未知"}${render.fallback_reason ? ` · ${render.fallback_reason}` : ""}` : render.error_message ?? label(render.subtitle_track);
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
  quality.addEventListener("change", () => {
    const extension = videoOutputExtension(selectedQuality());
    if (path.value.trim() && !path.value.trim().toLowerCase().endsWith(`.${extension}`)) path.value = "";
    path.placeholder = suggestedName();
    session?.outputChanged();
    renderEstimate();
  }, { signal: events.signal });
  confirm.querySelector("[data-cancel-render-confirmation]")!.addEventListener("click", () => settle(false), { signal: events.signal });
  confirm.querySelector("[data-accept-render-confirmation]")!.addEventListener("click", () => settle(true), { signal: events.signal });
  return {
    suggestedName,
    open(entry: VideoExportMode) {
      if (disposed) return;
      mode = entry;
      const soft = mode === "soft_matroska";
      title.textContent = soft ? "封装 MKV 软字幕" : "烧录导出";
      format.hidden = soft;
      submit.textContent = soft ? "开始封装" : "开始烧录";
      track.querySelector('option[value="source"]')!.textContent = `仅${options.context()?.source ?? "原文"}原文`;
      track.querySelector('option[value="translation"]')!.textContent = `仅${options.context()?.target ?? "译文"}译文`;
      path.placeholder = suggestedName(); boundary.open();
    },
    close() { settle(false); boundary.close(); },
    read: () => ({ outputPath: path.value, track: track.value as VideoRender["subtitle_track"], quality: selectedQuality() }),
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
    bind(controller: Session, trigger: HTMLButtonElement | null, entry: VideoExportMode) {
      session = controller;
      trigger?.addEventListener("click", () => { trigger.focus(); void session?.open(entry); }, { signal: events.signal });
    },
    dispose() { if (disposed) return; disposed = true; settle(false); events.abort(); confirmBoundary.dispose(); boundary.dispose(); rows.clear(); dialog.remove(); confirm.remove(); session = null; },
  };
}
