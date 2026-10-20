export type TextCue = { job_id: string; id: string; source_text: string; translated_text: string | null };
export type DraftText = { source: string; translation: string };
export type Draft<T> = { saved: T; value: DraftText; saving: boolean; error: string | null; reviewRequired?: boolean; recovered?: boolean; composing?: boolean; timer?: ReturnType<typeof setTimeout>; flight?: Promise<void> };
const text = (cue: TextCue): DraftText => ({ source: cue.source_text, translation: cue.translated_text ?? "" });
const same = (a: DraftText, b: DraftText) => a.source === b.source && a.translation === b.translation;
export const cueKey = (cue: TextCue): string => `${cue.job_id}:${cue.id}`;

// Each cue owns its draft and single writer; UI rebuilds never own pending text.
export class SubtitleDrafts<T extends TextCue> {
  readonly entries = new Map<string, Draft<T>>();
  private readonly write: (saved: T, value: DraftText) => Promise<T>;
  private readonly changed: (draft: Draft<T>, before?: T) => void;
  private readonly delay: number;
  constructor(
    write: (saved: T, value: DraftText) => Promise<T>,
    changed: (draft: Draft<T>, before?: T) => void,
    delay = 650,
  ) { this.write = write; this.changed = changed; this.delay = delay; }
  ensure(cue: T): Draft<T> {
    const key = cueKey(cue);
    let draft = this.entries.get(key);
    if (!draft) { draft = { saved: cue, value: text(cue), saving: false, error: null }; this.entries.set(key, draft); }
    else if (draft.recovered) {
      const unchanged = same(text(draft.saved), text(cue));
      draft.saved = cue; draft.recovered = false;
      if (!unchanged || draft.reviewRequired) { draft.reviewRequired = true; draft.error = "字幕已变化；恢复的草稿已保留，请核对后立即保存或放弃"; }
      else draft.timer = setTimeout(() => { void this.flush(key).catch(() => {}); }, this.delay);
    }
    else if (!this.pending(draft)) { draft.saved = cue; draft.value = text(cue); }
    return draft;
  }
  pending(draft: Draft<T>): boolean { return draft.saving || !same(draft.value, text(draft.saved)); }
  edit(cue: T, value: DraftText, composing = false): void {
    const draft = this.ensure(cue);
    draft.value = { ...value }; draft.error = null; draft.composing = composing; draft.reviewRequired = false;
    if (draft.timer) clearTimeout(draft.timer);
    draft.timer = undefined;
    if (!composing) draft.timer = setTimeout(() => { void this.flush(cueKey(cue)).catch(() => {}); }, this.delay);
    this.changed(draft);
  }
  discard(cue: T): void {
    const draft = this.ensure(cue);
    if (draft.saving) return;
    if (draft.timer) clearTimeout(draft.timer);
    draft.timer = undefined; draft.value = text(draft.saved); draft.error = null; draft.reviewRequired = false;
    this.changed(draft);
  }
  async flush(key: string, explicit = false): Promise<void> {
    const draft = this.entries.get(key);
    if (!draft || draft.recovered) return;
    if (explicit) draft.reviewRequired = false;
    if (draft.reviewRequired) throw new Error(draft.error ?? "请先核对恢复的草稿");
    if (draft.timer) clearTimeout(draft.timer);
    draft.timer = undefined;
    if (draft.flight) { await draft.flight; return this.flush(key, explicit); }
    if (!this.pending(draft)) return;
    const run = async () => {
      while (!draft.composing && !same(draft.value, text(draft.saved))) {
        const submitted = { ...draft.value }, before = draft.saved;
        draft.saving = true; draft.error = null; this.changed(draft);
        try {
          const saved = await this.write(before, submitted);
          draft.saved = saved;
          // Preserve typing that happened after this request started.
          if (same(draft.value, submitted)) draft.value = text(saved);
          this.changed(draft, before);
        } catch (error) {
          draft.error = String(error); throw error;
        } finally { draft.saving = false; this.changed(draft); }
      }
    };
    draft.flight = run();
    try { await draft.flight; } finally {
      draft.flight = undefined;
      if (!this.pending(draft) && draft.timer) { clearTimeout(draft.timer); draft.timer = undefined; }
    }
  }
  forgetJob(jobId: string): void {
    for (const [key, draft] of this.entries) {
      if (draft.saved.job_id !== jobId) continue;
      if (draft.timer) clearTimeout(draft.timer);
      this.entries.delete(key);
    }
  }
  snapshot(): string {
    return JSON.stringify([...this.entries.values()].filter(draft => this.pending(draft)).map(draft => ({ saved: draft.saved, value: draft.value, reviewRequired: draft.reviewRequired })));
  }
  recover(snapshot: string): void {
    try {
      const entries = JSON.parse(snapshot);
      if (!Array.isArray(entries)) return;
      for (const entry of entries) {
        if (typeof entry?.saved?.id !== "string" || typeof entry.saved.job_id !== "string" ||
            typeof entry.saved.source_text !== "string" ||
            !(entry.saved.translated_text === null || typeof entry.saved.translated_text === "string") ||
            typeof entry?.value?.source !== "string" || typeof entry.value.translation !== "string") continue;
        this.entries.set(cueKey(entry.saved), {saved: entry.saved, value: entry.value, saving: false, error: null, recovered: true, reviewRequired: !!entry.reviewRequired});
      }
    } catch { /* Ignore malformed caches; SQLite remains authoritative. */ }
  }
  async flushAll(): Promise<void> { await Promise.all([...this.entries.keys()].map(key => this.flush(key))); }
}
