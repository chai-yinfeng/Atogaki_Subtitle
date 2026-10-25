import type { TaskFileKind } from "./task-file-picker-backend.ts";
import type { createTaskFilePickerSession } from "./task-file-picker-session.ts";
type Options = { visit(): number; mediaAccepted(files: string[]): void; mediaEdited(): void; modelAccepted(): void };
const visible = (node: HTMLElement) => node.isConnected && node.getClientRects().length > 0 && !node.closest("[inert]");
export function mountTaskFilePickerView(root: HTMLFormElement, session: ReturnType<typeof createTaskFilePickerSession>, options: Options) {
  const ids: Record<string, TaskFileKind> = { "choose-media": "media", "choose-model": "model", "choose-vad-model": "vad" };
  const inputs = { media: root.querySelector<HTMLInputElement>("#media-path")!, model: root.querySelector<HTMLInputElement>("#model-path")!, vad: root.querySelector<HTMLInputElement>("#vad-model-path")! };
  const vad = root.querySelector<HTMLInputElement>("#vad-enabled")!;
  const buttons = Object.entries(ids).map(([id, kind]) => ({ node: root.querySelector<HTMLButtonElement>(`#${id}`)!, kind }));
  const versions = { media: 0, model: 0, vad: 0 }, events = new AbortController(); let disposed = false;
  const render = () => {
    if (disposed) return;
    if (inputs.vad.disabled !== !vad.checked) inputs.vad.disabled = !vad.checked;
    if (inputs.vad.required !== vad.checked) inputs.vad.required = vad.checked;
    for (const { node, kind } of buttons) { const disabled = session.busy || (kind === "vad" && !vad.checked); if (node.disabled !== disabled) node.disabled = disabled; }
  };
  const changed = (event: Event) => {
    for (const kind of ["media", "model", "vad"] as const) if (event.target === inputs[kind]) { ++versions[kind]; if (kind === "media") options.mediaEdited(); }
    if (event.target === vad) { ++versions.vad; render(); }
  };
  root.addEventListener("input", changed, { signal: events.signal });
  root.addEventListener("change", changed, { signal: events.signal });
  root.addEventListener("reset", () => { ++versions.media; ++versions.model; ++versions.vad; options.mediaEdited(); queueMicrotask(render); }, { signal: events.signal });
  root.addEventListener("click", event => {
    const target = event.target; if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("button"), kind = button ? ids[button.id] : undefined;
    if (!button || !kind || button.disabled || !visible(button)) return;
    const visit = options.visit(), focused = document.activeElement === button;
    void session.pick(kind).finally(() => {
      if (!disposed && focused && visit === options.visit() && !button.disabled && visible(button) && (document.activeElement === document.body || document.activeElement === root)) button.focus();
    });
  }, { signal: events.signal });
  render();
  return {
    render,
    invalidateModels() { if (!disposed) { ++versions.model; ++versions.vad; } },
    capture(kind: TaskFileKind) {
      if (disposed || (kind === "vad" && !vad.checked)) return null;
      const input = inputs[kind], value = input.value, version = versions[kind];
      return { apply(files: string[]): boolean {
        if (disposed || !files.length || input.value !== value || versions[kind] !== version || (kind === "vad" && !vad.checked)) return false;
        input.value = files[0];
        if (kind === "media") options.mediaAccepted([...files]); else options.modelAccepted();
        return true;
      } };
    },
    dispose() { disposed = true; events.abort(); },
  };
}
