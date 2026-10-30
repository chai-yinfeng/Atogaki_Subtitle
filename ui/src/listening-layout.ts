type Placement = "right" | "bottom" | "hidden";
type Layout = { version: 2; placement: Placement; right: number; bottom: number; video: number };
const defaults: Layout = { version: 2, placement: "right", right: 60, bottom: 72, video: 85 };
export function readListeningLayout(raw: string | null): Layout {
  try {
    const value = JSON.parse(raw ?? "null");
    if (![1, 2].includes(value?.version)) return { ...defaults };
    const ratio = (n: unknown, fallback: number, max = 75) => typeof n === "number" && Number.isFinite(n) ? Math.max(25, Math.min(max, n)) : fallback;
    // Bottom now contains stacked video/captions, so the old two-column height needs a new default.
    return { version: 2, placement: ["right", "bottom", "hidden"].includes(value.placement) ? value.placement : "right", right: ratio(value.right, 60), bottom: value.version === 2 ? ratio(value.bottom, 72) : 72, video: ratio(value.video, 85, 90) };
  } catch { return { ...defaults }; }
}
/** Layout changes retain the shared media element and transcript selection. */
export function mountListeningLayout(root: HTMLElement) {
  const key = "atogaki.listeningLayout.v2", events = new AbortController();
  let layout = { ...defaults };
  try { layout = readListeningLayout(localStorage.getItem(key) ?? localStorage.getItem("atogaki.listeningLayout.v1")); } catch { /* Optional preference. */ }
  const select = root.querySelector<HTMLSelectElement>("#listening-layout-placement")!;
  const handle = root.querySelector<HTMLElement>(".listening-splitter")!;
  const videoHandle = root.querySelector<HTMLElement>(".listening-video-splitter")!;
  const session = root.querySelector<HTMLElement>(".listening-session")!;
  const playback = root.querySelector<HTMLElement>(".listening-playback")!;
  const reading = root.querySelector<HTMLElement>(".listening-reading")!;
  type Field = "right" | "bottom" | "video";
  let drag: { id: number; field: Field; vertical: boolean; origin: number; value: number; extent: number } | null = null;
  const field = (target: HTMLElement): Field => target === videoHandle ? "video" : layout.placement === "bottom" ? "bottom" : "right";
  const persist = () => { try { localStorage.setItem(key, JSON.stringify(layout)); } catch { /* Optional preference. */ } };
  const render = () => {
    const outer = field(handle);
    session.dataset.placement = layout.placement;
    session.style.setProperty("--listening-primary-ratio", `${layout[outer]}fr`);
    session.style.setProperty("--listening-reading-ratio", `${100 - layout[outer]}fr`);
    playback.style.setProperty("--listening-video-ratio", `${layout.video}fr`);
    playback.style.setProperty("--listening-caption-ratio", `${100 - layout.video}fr`);
    select.value = layout.placement;
    reading.inert = layout.placement === "hidden";
    handle.setAttribute("aria-orientation", layout.placement === "bottom" ? "horizontal" : "vertical");
    handle.setAttribute("aria-valuenow", String(Math.round(layout[outer])));
    videoHandle.setAttribute("aria-valuenow", String(Math.round(layout.video)));
  };
  const change = (name: Field, value: number) => { layout[name] = Math.max(25, Math.min(name === "video" ? 90 : 75, value)); render(); };
  select.addEventListener("change", () => { layout.placement = select.value as Placement; render(); persist(); }, { signal: events.signal });
  for (const target of [handle, videoHandle]) {
    target.addEventListener("keydown", event => {
      const name = field(target), vertical = name !== "right";
      const negative = vertical ? "ArrowUp" : "ArrowLeft", positive = vertical ? "ArrowDown" : "ArrowRight";
      if (![negative, positive, "Home"].includes(event.key)) return;
      event.preventDefault(); event.stopPropagation();
      change(name, event.key === "Home" ? defaults[name] : layout[name] + (event.key === negative ? -2 : 2)); persist();
    }, { signal: events.signal });
    target.addEventListener("pointerdown", event => {
      if (event.button !== 0) return;
      const name = field(target), bounds = session.getBoundingClientRect();
      const extent = name === "video" ? playback.querySelector<HTMLElement>("#listening-media-host")!.clientHeight + playback.querySelector<HTMLElement>(".current-caption")!.clientHeight : name === "right" ? bounds.width : bounds.height;
      drag = { id: event.pointerId, field: name, vertical: name !== "right", origin: name === "right" ? event.clientX : event.clientY, value: layout[name], extent };
      target.setPointerCapture(event.pointerId); event.preventDefault();
    }, { signal: events.signal });
    target.addEventListener("pointermove", event => {
      if (!drag || drag.id !== event.pointerId) return;
      const position = drag.vertical ? event.clientY : event.clientX;
      change(drag.field, drag.value + (position - drag.origin) / Math.max(1, drag.extent) * 100);
    }, { signal: events.signal });
    const finish = () => { drag = null; persist(); };
    for (const name of ["pointerup", "pointercancel", "lostpointercapture"]) target.addEventListener(name, finish, { signal: events.signal });
  }
  render();
  return { dispose() { events.abort(); drag = null; } };
}
