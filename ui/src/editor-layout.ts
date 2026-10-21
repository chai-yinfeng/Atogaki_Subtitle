export type EditorLayout = { version: 3; video: boolean; waveform: boolean; videoRatio: number; topRatio: number; waveRatio: number };
export const defaultEditorLayout: EditorLayout = { version: 3, video: true, waveform: true, videoRatio: 44, topRatio: 67, waveRatio: 34 };
const limit = (value: unknown, fallback: number, min: number, max: number): number => typeof value === "number" && Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
export function readEditorLayout(raw: string | null): EditorLayout {
  try {
    const value = JSON.parse(raw ?? "null");
    if (![1, 2, 3].includes(value?.version)) return { ...defaultEditorLayout };
    // Old horizontal sizes were chosen for video + waveform alone. Reset all
    // ratios to suit the larger video + stacked right workspace; keep folds.
    return { version: 3, video: typeof value.video === "boolean" ? value.video : true,
      waveform: typeof value.waveform === "boolean" ? value.waveform : true,
      videoRatio: value.version === 3 ? limit(value.videoRatio, 44, 25, 65) : 44,
      topRatio: value.version === 3 ? limit(value.topRatio, 67, 45, 78) : 67,
      waveRatio: value.version === 3 ? limit(value.waveRatio, 34, 20, 55) : 34 };
  } catch { return { ...defaultEditorLayout }; }
}
type Ratio = "videoRatio" | "topRatio" | "waveRatio";
/** One owner for resize handlers and layout preferences; hiding video keeps media mounted. */
export function mountEditorLayout(root: HTMLElement): { dispose(): void; refresh(): void } {
  const key = "atogaki.editorLayout.v3";
  let layout = readEditorLayout(null);
  try { layout = readEditorLayout(localStorage.getItem(key) ?? localStorage.getItem("atogaki.editorLayout.v2") ?? localStorage.getItem("atogaki.editorLayout.v1")); } catch { /* Preferences are optional. */ }
  const events = new AbortController();
  let drag: { pointerId: number; field: Ratio; origin: number; value: number; extent: number; target: HTMLElement } | null = null;
  const render = () => {
    root.classList.toggle("editor-video-hidden", !layout.video);
    root.classList.toggle("editor-waveform-hidden", !layout.waveform);
    const mediaHost = root.querySelector<HTMLElement>("#karaoke-media-host");
    if (mediaHost) mediaHost.inert = !layout.video;
    root.style.setProperty("--editor-video-ratio", `${layout.videoRatio}fr`);
    root.style.setProperty("--editor-right-width", `${100 - layout.videoRatio}fr`);
    root.style.setProperty("--editor-top-ratio", `${layout.topRatio}fr`);
    root.style.setProperty("--editor-list-ratio", `${100 - layout.topRatio}fr`);
    root.style.setProperty("--editor-wave-ratio", `${layout.waveRatio}fr`);
    root.style.setProperty("--editor-edit-ratio", `${100 - layout.waveRatio}fr`);
    for (const panel of ["video", "waveform"] as const) {
      const toggle = root.querySelector<HTMLButtonElement>(`[data-editor-toggle="${panel}"]`);
      toggle?.setAttribute("aria-expanded", String(layout[panel]));
      if (toggle) toggle.textContent = `${layout[panel] ? "收起" : "展开"}${panel === "video" ? "视频" : "波形"}`;
    }
    root.querySelectorAll<HTMLElement>("[data-editor-resize]").forEach(handle => handle.setAttribute("aria-valuenow", String(Math.round(layout[handle.dataset.editorResize as Ratio]))));
  };
  const persist = () => { try { localStorage.setItem(key, JSON.stringify(layout)); } catch { /* Layout still works without storage. */ } };
  root.addEventListener("click", event => {
    const target = (event.target as HTMLElement).closest<HTMLElement>("[data-editor-toggle], [data-editor-reset]");
    if (!target) return;
    const panel = target.dataset.editorToggle;
    if (panel === "video" || panel === "waveform") layout[panel] = !layout[panel];
    else layout = { ...defaultEditorLayout };
    render(); persist();
  }, { signal: events.signal });
  root.querySelectorAll<HTMLElement>("[data-editor-resize]").forEach(handle => {
    const field = handle.dataset.editorResize as Ratio;
    const change = (value: number) => { layout = readEditorLayout(JSON.stringify({ ...layout, [field]: value })); render(); };
    handle.addEventListener("keydown", event => {
      const negative = field === "videoRatio" ? "ArrowLeft" : "ArrowUp", positive = field === "videoRatio" ? "ArrowRight" : "ArrowDown";
      if (event.key !== negative && event.key !== positive && event.key !== "Home") return;
      event.preventDefault(); event.stopPropagation();
      change(event.key === "Home" ? defaultEditorLayout[field] : layout[field] + (event.key === negative ? -2 : 2)); persist();
    }, { signal: events.signal });
    handle.addEventListener("pointerdown", event => {
      if (event.button !== 0) return;
      event.preventDefault(); handle.setPointerCapture(event.pointerId);
      const bounds = root.querySelector<HTMLElement>(field === "videoRatio" ? ".editor-top" : field === "waveRatio" ? ".editor-right" : ".editor-panels")!.getBoundingClientRect();
      drag = { pointerId: event.pointerId, field, origin: field === "videoRatio" ? event.clientX : event.clientY, value: layout[field], extent: field === "videoRatio" ? bounds.width : bounds.height, target: handle };
    }, { signal: events.signal });
    handle.addEventListener("pointermove", event => {
      if (!drag || drag.target !== handle || drag.pointerId !== event.pointerId) return;
      change(drag.value + ((field === "videoRatio" ? event.clientX : event.clientY) - drag.origin) / Math.max(1, drag.extent) * 100);
    }, { signal: events.signal });
    const finish = () => { drag = null; persist(); };
    handle.addEventListener("pointerup", finish, { signal: events.signal });
    handle.addEventListener("pointercancel", finish, { signal: events.signal });
    handle.addEventListener("lostpointercapture", finish, { signal: events.signal });
  });
  render();
  return { refresh: render, dispose() { events.abort(); drag = null; } };
}
