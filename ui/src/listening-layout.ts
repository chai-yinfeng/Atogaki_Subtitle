type Placement = "right" | "bottom" | "hidden";
type Layout = { version: 1; placement: Placement; right: number; bottom: number };
const defaults: Layout = { version: 1, placement: "right", right: 43, bottom: 52 };
export function readListeningLayout(raw: string | null): Layout {
  try {
    const value = JSON.parse(raw ?? "null");
    if (value?.version !== 1) return { ...defaults };
    const ratio = (n: unknown, fallback: number) => typeof n === "number" && Number.isFinite(n) ? Math.max(25, Math.min(75, n)) : fallback;
    return { version: 1, placement: ["right", "bottom", "hidden"].includes(value.placement) ? value.placement : "right", right: ratio(value.right, 43), bottom: ratio(value.bottom, 52) };
  } catch { return { ...defaults }; }
}
/** Layout changes retain the shared media element and transcript selection. */
export function mountListeningLayout(root: HTMLElement) {
  const key = "atogaki.listeningLayout.v1", events = new AbortController();
  let layout = { ...defaults };
  try { layout = readListeningLayout(localStorage.getItem(key)); } catch { /* Optional preference. */ }
  const select = root.querySelector<HTMLSelectElement>("#listening-layout-placement")!;
  const handle = root.querySelector<HTMLElement>(".listening-splitter")!;
  const session = root.querySelector<HTMLElement>(".listening-session")!;
  const reading = root.querySelector<HTMLElement>(".listening-reading")!;
  let drag: { id: number; origin: number; value: number; extent: number } | null = null;
  const field = () => layout.placement === "bottom" ? "bottom" : "right";
  const persist = () => { try { localStorage.setItem(key, JSON.stringify(layout)); } catch { /* Optional preference. */ } };
  const render = () => {
    session.dataset.placement = layout.placement;
    session.style.setProperty("--listening-primary-ratio", `${layout[field()]}fr`);
    session.style.setProperty("--listening-reading-ratio", `${100 - layout[field()]}fr`);
    select.value = layout.placement;
    reading.inert = layout.placement === "hidden";
    handle.setAttribute("aria-orientation", layout.placement === "bottom" ? "horizontal" : "vertical");
    handle.setAttribute("aria-valuenow", String(Math.round(layout[field()])));
  };
  select.addEventListener("change", () => { layout.placement = select.value as Placement; render(); persist(); }, { signal: events.signal });
  handle.addEventListener("keydown", event => {
    const negative = layout.placement === "bottom" ? "ArrowUp" : "ArrowLeft";
    const positive = layout.placement === "bottom" ? "ArrowDown" : "ArrowRight";
    if (![negative, positive, "Home"].includes(event.key)) return;
    event.preventDefault(); event.stopPropagation();
    layout[field()] = event.key === "Home" ? defaults[field()] : Math.max(25, Math.min(75, layout[field()] + (event.key === negative ? -2 : 2)));
    render(); persist();
  }, { signal: events.signal });
  handle.addEventListener("pointerdown", event => {
    if (event.button !== 0) return;
    const bounds = session.getBoundingClientRect();
    drag = { id: event.pointerId, origin: layout.placement === "bottom" ? event.clientY : event.clientX, value: layout[field()], extent: layout.placement === "bottom" ? bounds.height : bounds.width };
    handle.setPointerCapture(event.pointerId); event.preventDefault();
  }, { signal: events.signal });
  handle.addEventListener("pointermove", event => {
    if (!drag || drag.id !== event.pointerId) return;
    const position = layout.placement === "bottom" ? event.clientY : event.clientX;
    layout[field()] = Math.max(25, Math.min(75, drag.value + (position - drag.origin) / Math.max(1, drag.extent) * 100)); render();
  }, { signal: events.signal });
  const finish = () => { drag = null; persist(); };
  for (const name of ["pointerup", "pointercancel", "lostpointercapture"]) handle.addEventListener(name, finish, { signal: events.signal });
  render();
  return { dispose() { events.abort(); drag = null; } };
}
