import { cueKey, type Draft, type SubtitleDrafts, type TextCue } from "./subtitle-drafts";

type EditorOptions<T extends TextCue> = {
  drafts: SubtitleDrafts<T>;
  current(): T | undefined;
  lock(): void;
  canUndo(cue: T): boolean;
  undo(): Promise<void>;
  status(message: string, error?: boolean): void;
};

/** One text view over the session's existing cue drafts; owns no second writer. */
export function mountCurrentCueEditor<T extends TextCue>(root: HTMLElement, options: EditorOptions<T>) {
  const source = root.querySelector<HTMLTextAreaElement>("#karaoke-current-source")!;
  const translation = root.querySelector<HTMLTextAreaElement>("#karaoke-current-translation")!;
  const save = root.querySelector<HTMLButtonElement>("#karaoke-save-text")!;
  const discard = root.querySelector<HTMLButtonElement>("#karaoke-discard-text")!;
  const undo = root.querySelector<HTMLButtonElement>("#karaoke-undo-text")!;
  const events = new AbortController();
  let shown: T | undefined;
  const currentDraft = () => shown ? options.drafts.entries.get(cueKey(shown)) : undefined;
  const dirty = () => { const draft = currentDraft(); return !!draft && options.drafts.pending(draft); };
  const isShowing = (cue: T) => shown?.id === cue.id && shown.job_id === cue.job_id;
  const sync = (cue: T | undefined, force = false) => {
    if (dirty() && !force) return;
    shown = cue;
    const draft = cue ? options.drafts.ensure(cue) : undefined;
    const text = draft?.value ?? { source: "", translation: "" };
    if (source.value !== text.source) source.value = text.source;
    if (translation.value !== text.translation) translation.value = text.translation;
    source.disabled = translation.disabled = !cue;
    save.disabled = discard.disabled = !dirty() || !!draft?.saving;
    undo.disabled = dirty() || !cue || !options.canUndo(cue);
    if (draft?.error) options.status(draft.error, true);
  };
  const edit = (composing: boolean) => {
    options.lock();
    const cue = options.current();
    if (!cue || !isShowing(cue)) return;
    options.drafts.edit(cue, { source: source.value, translation: translation.value }, composing);
  };
  for (const input of [source, translation]) {
    input.addEventListener("focus", options.lock, { signal: events.signal });
    input.addEventListener("input", event => edit((event as InputEvent).isComposing ?? false), { signal: events.signal });
    input.addEventListener("compositionstart", () => edit(true), { signal: events.signal });
    input.addEventListener("compositionend", () => edit(false), { signal: events.signal });
    input.addEventListener("blur", () => {
      if (shown) void options.drafts.flush(cueKey(shown)).catch(() => {});
    }, { signal: events.signal });
  }
  save.addEventListener("click", () => {
    if (shown) void options.drafts.flush(cueKey(shown), true).catch(() => {});
  }, { signal: events.signal });
  discard.addEventListener("click", () => {
    if (shown) options.drafts.discard(shown);
    sync(shown, true);
    options.status("已放弃当前字幕尚未保存的文字修改。");
  }, { signal: events.signal });
  undo.addEventListener("click", () => void options.undo(), { signal: events.signal });
  return {
    get dirty() { return dirty(); },
    sync, isShowing,
    matches(cue: T | undefined) {
      return shown?.id === cue?.id && shown?.job_id === cue?.job_id && source.value === (cue?.source_text ?? "") && translation.value === (cue?.translated_text ?? "");
    },
    refresh(draft: Draft<T>) {
      if (!isShowing(draft.saved)) return;
      save.disabled = discard.disabled = !dirty() || draft.saving;
      undo.disabled = dirty() || !options.canUndo(draft.saved);
      if (!dirty()) sync(draft.saved, true);
    },
    clear() { shown = undefined; sync(undefined, true); },
    dispose() { events.abort(); shown = undefined; },
  };
}
