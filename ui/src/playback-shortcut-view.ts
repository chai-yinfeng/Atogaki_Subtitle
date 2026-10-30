import { playbackActionForKey, type PlaybackAction } from "./playback-shortcuts";
type Options = {
  enabled(): boolean;
  editor(): boolean;
  blocked(): boolean;
  cut(): void;
  undo(): void;
  step(milliseconds: number): void;
  playback(action: PlaybackAction): void;
};
const textEntry = (target: EventTarget | null) => target instanceof HTMLInputElement
  || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement || target instanceof HTMLButtonElement
  || target instanceof HTMLElement && (target.isContentEditable || target.getAttribute("role") === "separator");

/** Shared document shortcuts; editing fields and modal/task tools keep their keys. */
export function mountPlaybackShortcutView(root: Document, options: Options) {
  const events = new AbortController();
  root.addEventListener("keydown", event => {
    if (event.defaultPrevented || event.isComposing || event.altKey) return;
    if (root.querySelector("dialog[open]") || options.blocked()) return;
    if (options.editor() && !event.shiftKey && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z") {
      const target = event.target;
      const editable = target instanceof HTMLTextAreaElement || target instanceof HTMLInputElement && ["text", "search", "email", "url", "tel", "password", "number"].includes(target.type)
        || target instanceof HTMLElement && target.isContentEditable;
      if (editable) {
        // macOS uses Command+Z natively; alias Control+Z to the same DOM history.
        if (event.ctrlKey && !event.metaKey && !((target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) && (target.readOnly || target.disabled))) {
          root.execCommand("undo"); event.preventDefault();
        }
      } else if (!(target instanceof HTMLSelectElement || target instanceof HTMLInputElement)) {
        options.undo(); event.preventDefault();
      }
      return;
    }
    if (!options.enabled() || textEntry(event.target)) return;
    if (options.editor() && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b") {
      options.cut(); event.preventDefault(); return;
    }
    if (event.metaKey || event.ctrlKey) return;
    if (options.editor() && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
      options.step((event.key === "ArrowLeft" ? -1 : 1) * (event.shiftKey ? 10 : 100));
      event.preventDefault(); return;
    }
    const action = playbackActionForKey(event);
    if (!action || event.repeat && (action === "toggle-playback" || action === "toggle-overlay")) return;
    options.playback(action); event.preventDefault();
  }, { signal: events.signal });
  return { dispose() { events.abort(); } };
}
