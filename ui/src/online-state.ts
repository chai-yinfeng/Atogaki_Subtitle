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
export type OnlineSnapshot = { status: OnlineStatus; events: CaptionEvent[]; captionLocked?: boolean };
export const phaseLabel: Record<string, string> = {
  idle: "未开始", loading: "正在加载模型", requesting_audio: "等待系统音频权限",
  running: "正在接收系统音频", stopping: "正在停止采集", draining: "正在完成最后一段",
  stopped: "已停止", failed: "会话失败",
};
export const activePhase = (phase: string) => ["loading", "requesting_audio", "running", "stopping", "draining"].includes(phase);

const byTime = (a: LiveEvent, b: LiveEvent) => (a.start_s ?? 0) - (b.start_s ?? 0);

/** Keep settled captions independently of the rolling event transport buffer. */
export class OnlineCaptions {
  status: OnlineStatus = { sessionId: null, phase: "idle", error: null };
  translationEnabled = true;
  private events = new Map<number, CaptionEvent>();
  private sources = new Map<string, CaptionEvent>();
  private groups = new Map<string, CaptionEvent>();
  private prunedThrough = -1;
  private sourceEnd = 0;
  private translationEnd = 0;

  snapshot(snapshot: OnlineSnapshot): void {
    if (snapshot.status.sessionId !== this.status.sessionId) {
      this.events.clear();
      this.sources.clear();
      this.groups.clear();
      this.prunedThrough = -1;
      this.sourceEnd = this.translationEnd = 0;
    }
    this.status = snapshot.status;
    this.translationEnabled = snapshot.status.translationEnabled ?? true;
    for (const e of snapshot.events) this.accept(e);
  }

  accept(event: CaptionEvent): void {
    if (event.sessionId !== this.status.sessionId || event.sequence <= this.prunedThrough || this.events.has(event.sequence)) return;
    this.events.set(event.sequence, event);
    const e = event.event;
    if (e.segment_id && (e.kind === "source_draft" || e.kind === "source_final")) {
      if (this.update(this.sources, e.segment_id, event, 64)) this.sourceEnd = Math.max(this.sourceEnd, e.end_s ?? e.start_s ?? 0);
    }
    if (e.group_id && (e.kind === "translation_draft" || e.kind === "translation_final")) {
      if (this.update(this.groups, e.group_id, event, 64)) this.translationEnd = Math.max(this.translationEnd, e.end_s ?? e.start_s ?? 0);
    }
    while (this.events.size > 256) {
      const oldest = Math.min(...this.events.keys());
      this.events.delete(oldest);
      this.prunedThrough = Math.max(this.prunedThrough, oldest);
    }
  }

  private update(map: Map<string, CaptionEvent>, key: string, incoming: CaptionEvent, limit: number): boolean {
    const previous = map.get(key);
    if (previous && (previous.sequence >= incoming.sequence || previous.event.kind.endsWith("final"))) return false;
    map.set(key, incoming);
    while (map.size > limit) {
      const oldest = [...map.entries()].sort((a, b) => byTime(a[1].event, b[1].event))[0][0];
      map.delete(oldest);
    }
    return true;
  }

  tracks(): { source: LiveEvent | null; translation: LiveEvent | null; sourceParts: LiveEvent[]; translationParts: LiveEvent[] } {
    const sources = [...this.sources.values()].map(e => e.event).sort(byTime);
    const translations = [...this.groups.values()].map(e => e.event).sort(byTime);
    return {
      source: sources.at(-1) ?? null,
      translation: translations.at(-1) ?? null,
      sourceParts: readingWindow(sources, this.sourceEnd),
      translationParts: this.translationEnabled ? readingWindow(translations, this.translationEnd) : [],
    };
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
    if (!this.current || key(next) !== key(this.current) || next.kind.endsWith("final")) {
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
