export type OnlineStatus = {
  sessionId: string | null;
  phase: string;
  error: string | null;
  translationEnabled?: boolean;
};
export type LiveEvent = {
  kind: string;
  text?: string;
  source_text?: string;
  source_revisions?: Record<string, number>;
  segment_id?: string;
  group_id?: string;
  start_s?: number;
  end_s?: number;
  after_audio_end_s?: number;
  after_first_audio_end_s?: number;
  stale_source?: boolean;
  provisional?: boolean;
};
export type CaptionEvent = { sessionId: string; sequence: number; event: LiveEvent };
export type OnlineSnapshot = { status: OnlineStatus; events: CaptionEvent[]; captionLocked?: boolean; captionVisible?: boolean; captionOpacity?: number };
export const phaseLabel: Record<string, string> = {
  idle: "未开始", loading: "正在加载模型", requesting_audio: "等待系统音频权限",
  running: "正在接收系统音频", stopping: "正在停止采集", draining: "正在完成最后一段",
  stopped: "已停止", failed: "会话失败",
};
export const activePhase = (phase: string) => ["loading", "requesting_audio", "running", "stopping", "draining"].includes(phase);

const endOf = (event: LiveEvent) => event.end_s ?? event.start_s ?? 0;
const settled = (event: LiveEvent) => event.kind === "translation_final" || event.kind === "translation_failed";
export const stableReadingMs = 1000;
const sameReading = (a: LiveEvent, b: LiveEvent) => a.group_id === b.group_id &&
  a.text === b.text && a.start_s === b.start_s && a.end_s === b.end_s;
const byTime = (a: LiveEvent, b: LiveEvent) => (a.start_s ?? 0) - (b.start_s ?? 0);

/** Keep settled captions independently of the rolling event transport buffer. */
export class OnlineCaptions {
  status: OnlineStatus = { sessionId: null, phase: "idle", error: null };
  translationEnabled = true;
  translationFailures = 0;
  private events = new Map<number, CaptionEvent>();
  private sources = new Map<string, CaptionEvent>();
  private groups = new Map<string, CaptionEvent>();
  private groupSources = new Map<string, LiveEvent[]>();
  private prunedThrough = -1;
  private sourceEnd = 0;
  private displayAfter = -Infinity;
  private page: LiveEvent | null = null;
  private pageDeadline: number | null = null;
  private shownDraft: { event: LiveEvent; since: number } | null = null;

  private clock: () => number;
  constructor(clock: () => number = () => performance.now()) { this.clock = clock; }

  snapshot(snapshot: OnlineSnapshot): void {
    const changed = snapshot.status.sessionId !== this.status.sessionId;
    if (changed) {
      this.translationFailures = 0;
      this.events.clear();
      this.sources.clear();
      this.groups.clear();
      this.groupSources.clear();
      this.prunedThrough = -1;
      this.sourceEnd = 0;
      this.displayAfter = -Infinity;
      this.page = null;
      this.pageDeadline = null;
      this.shownDraft = null;
    }
    this.status = snapshot.status;
    this.translationEnabled = snapshot.status.translationEnabled ?? true;
    for (const e of snapshot.events) this.accept(e);
    // Restoring a window is not a replay of all retained transport history.
    if (changed && this.groups.size) {
      this.page = [...this.groups.values()].map(e => e.event).sort(byTime).at(-1)!;
      this.displayAfter = this.page.start_s ?? -Infinity;
    }
  }

  /** Record what the view actually painted, after draft revision gating. */
  observeTranslation(event: LiveEvent | undefined): void {
    if (!event || event.kind !== "translation_draft" || !event.text) {
      this.shownDraft = null;
    } else if (!this.shownDraft || !sameReading(this.shownDraft.event, event)) {
      this.shownDraft = { event, since: this.clock() };
    }
  }

  accept(event: CaptionEvent): void {
    if (event.sessionId !== this.status.sessionId || event.sequence <= this.prunedThrough || this.events.has(event.sequence)) return;
    if (event.event.kind === "translation_failed") event = {...event, event:{...event.event, text:"本段翻译失败"}};
    this.events.set(event.sequence, event);
    const e = event.event;
    if (e.segment_id && (e.kind === "source_draft" || e.kind === "source_final")) {
      if (this.update(this.sources, e.segment_id, event, 64)) {
        this.sourceEnd = Math.max(this.sourceEnd, e.end_s ?? e.start_s ?? 0);
        if (e.kind === "source_final") for (const [id, parts] of this.groupSources) {
          this.groupSources.set(id, parts.map(part => part.segment_id === e.segment_id ? e : part));
        }
      }
    }
    if (e.group_id && (e.kind === "translation_draft" || e.kind === "translation_final" || e.kind === "translation_failed")) {
      if (this.update(this.groups, e.group_id, event, 64)) {
        if (e.kind === "translation_failed") this.translationFailures++;
        this.groupSources.set(e.group_id, this.sourcesFor(e));
        for (const id of this.groupSources.keys()) if (!this.groups.has(id)) this.groupSources.delete(id);
      }
    }
    while (this.events.size > 256) {
      const oldest = Math.min(...this.events.keys());
      this.events.delete(oldest);
      this.prunedThrough = Math.max(this.prunedThrough, oldest);
    }
  }

  private sourcesFor(group: LiveEvent): LiveEvent[] {
    const previous = this.groupSources.get(group.group_id!) ?? [];
    const ids = Object.keys(group.source_revisions ?? {});
    const parts = ids.length ? ids.map(id => this.sources.get(id)?.event ?? previous.find(e => e.segment_id === id)).filter((e): e is LiveEvent => !!e)
      : [...this.sources.values()].map(e => e.event).filter(e =>
        (e.end_s ?? e.start_s ?? 0) > (group.start_s ?? 0) && (e.start_s ?? 0) < (group.end_s ?? 0));
    if (parts.length && (!ids.length || parts.length === ids.length)) return parts.sort(byTime);
    // A newly opened window may only receive the MT snapshot after older ASR
    // events left the transport ring. Its actual input still recovers the pair.
    // MT finals require all-final Qwen source; unknown draft portions stay blue.
    return group.source_text ? [{kind: (group.kind === "translation_final" || group.kind === "translation_failed") ? "source_final" : "source_draft",
      segment_id: `paired:${group.group_id}`, start_s: group.start_s, end_s: group.end_s, text: group.source_text}] : parts;
  }

  private update(map: Map<string, CaptionEvent>, key: string, incoming: CaptionEvent, limit: number): boolean {
    const previous = map.get(key);
    if (previous && (previous.sequence >= incoming.sequence || (previous.event.kind.endsWith("final") || previous.event.kind === "translation_failed"))) return false;
    map.set(key, incoming);
    while (map.size > limit) {
      const oldest = [...map.entries()].sort((a, b) => byTime(a[1].event, b[1].event))[0][0];
      map.delete(oldest);
    }
    return true;
  }

  tracks(displayedTranslations?: LiveEvent[]): { source: LiveEvent | null; translation: LiveEvent | null; sourceParts: LiveEvent[]; translationParts: LiveEvent[] } {
    const sources = [...this.sources.values()].map(e => e.event).sort(byTime);
    const translations = [...this.groups.values()].map(e => e.event).sort(byTime);
    const current = this.translationEnabled && displayedTranslations === undefined ? this.readingPage(translations) : null;
    const translationParts = this.translationEnabled ? displayedTranslations ?? (current ? [current] : []) : [];
    let sourceParts = readingWindow(sources, this.sourceEnd);
    if (this.translationEnabled) {
      const group = translationParts[0];
      if (group) {
        const paired = this.sourcesFor(group);
        const ids = new Set(paired.map(e => e.segment_id));
        // A settled page has an exact source boundary. B continues in memory,
        // independently of A's reading timer; no B readiness gates retirement.
        const next = translations.find(e => (e.start_s ?? 0) > (group.start_s ?? 0));
        const tail = settled(group) ? [] : sources.filter(e => !ids.has(e.segment_id) &&
          endOf(e) > endOf(group) && (!next || (e.start_s ?? 0) < (next.start_s ?? 0)));
        sourceParts = [...paired, ...tail].sort(byTime);
      } else {
        sourceParts = sources.filter(e => endOf(e) > this.displayAfter);
      }
    }
    return {
      source: sources.at(-1) ?? null,
      translation: translations.at(-1) ?? null,
      sourceParts,
      translationParts,
    };
  }

  private readingPage(parts: LiveEvent[]): LiveEvent | null {
    const now = this.clock();
    if (this.page) this.page = parts.find(e => e.group_id === this.page!.group_id) ?? this.page;
    if (!this.page) this.page = parts.find(e => endOf(e) > this.displayAfter) ?? null;
    while (this.page && settled(this.page)) {
      if (this.pageDeadline === null) {
        const draft = this.shownDraft;
        const unchanged = this.page.kind === "translation_final" && draft && sameReading(draft.event, this.page);
        this.pageDeadline = (unchanged ? draft.since : now) + stableReadingMs;
      }
      if (now < this.pageDeadline) break;
      this.displayAfter = Math.max(this.displayAfter, endOf(this.page!));
      this.page = parts.find(e => endOf(e) > this.displayAfter) ?? null;
      this.pageDeadline = null;
    }
    return this.page;
  }
}

/** Five seconds is a soft reading range: keep complete units and the locked
 * prefix immediately before a mutable tail. No word timings are invented. */
function readingWindow(parts: LiveEvent[], end: number): LiveEvent[] {
  const cutoff = end - 5;
  const firstDraft = parts.findIndex(e => e.kind.endsWith("draft"));
  const previousFinal = (firstDraft < 0 ? parts : parts.slice(0, firstDraft)).filter(e => e.kind.endsWith("final")).at(-1);
  return parts.filter(e => e.kind.endsWith("draft") || e === previousFinal || (e.end_s ?? e.start_s ?? 0) > cutoff);
}

/** Prefix edits are atomic, revisable snapshots; append-only growth stays prompt. */
export class DraftPresentation {
  private current: LiveEvent | null = null;
  private lastRewrite = -Infinity;
  private intervalMs = 2000;

  select(next: LiveEvent | null, now: number): LiveEvent | null {
    if (!next) { this.current = null; this.lastRewrite = -Infinity; return null; }
    const key = (e: LiveEvent) => e.group_id ?? e.segment_id;
    if (!this.current || key(next) !== key(this.current) || (next.kind.endsWith("final") || next.kind === "translation_failed")) {
      this.current = next;
      this.lastRewrite = now;
    } else if (next.text === this.current.text) {
      this.current = next;
    } else if ((next.text ?? "").startsWith(this.current.text ?? "")) {
      this.current = next;
    } else if (now - this.lastRewrite >= this.intervalMs) {
      this.current = next;
      this.lastRewrite = now;
    }
    return this.current;
  }
}
