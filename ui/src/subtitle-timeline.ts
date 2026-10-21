export type TimedCue = { id: string; start_ms: number; end_ms: number };

/** Index workspace timing snapshots. Text-only replacements preserve the index.
 * Overlaps keep the original first-cue
 * selection semantics, rather than assuming end times are sorted. */
export class SubtitleTimeline<T extends TimedCue> {
  private ordered: { cue: T; index: number }[];
  private maximumEnd: number[] = [];
  private byId = new Map<string, { cue: T; index: number }>();
  constructor(cues: readonly T[]) {
    this.ordered = cues.map((cue, index) => ({ cue, index })).sort((a, b) => a.cue.start_ms - b.cue.start_ms || a.index - b.index);
    let maximum = -Infinity;
    this.ordered.forEach(entry => {
      maximum = Math.max(maximum, entry.cue.end_ms);
      this.maximumEnd.push(maximum);
      this.byId.set(entry.cue.id, entry);
    });
  }
  private firstEndingAfter(time: number, inclusive = false): number {
    let left = 0, right = this.maximumEnd.length;
    while (left < right) {
      const mid = (left + right) >>> 1;
      if (inclusive ? this.maximumEnd[mid] < time : this.maximumEnd[mid] <= time) left = mid + 1;
      else right = mid;
    }
    return left;
  }
  at(time: number): T | undefined {
    let found: { cue: T; index: number } | undefined;
    for (let i = this.firstEndingAfter(time); i < this.ordered.length; i++) {
      const entry = this.ordered[i];
      if (entry.cue.start_ms > time) break;
      if (entry.cue.end_ms > time && (!found || entry.index < found.index)) found = entry;
    }
    return found?.cue;
  }
  visible(start: number, end: number): T[] {
    const result: T[] = [];
    for (let i = this.firstEndingAfter(start, true); i < this.ordered.length; i++) {
      const { cue } = this.ordered[i];
      if (cue.start_ms > end) break;
      if (cue.end_ms >= start) result.push(cue);
    }
    return result;
  }
  replaceText(cue: T): boolean {
    const entry = this.byId.get(cue.id);
    if (!entry || entry.cue.start_ms !== cue.start_ms || entry.cue.end_ms !== cue.end_ms) return false;
    entry.cue = cue;
    return true;
  }
  get(id: string | null): T | undefined { return id ? this.byId.get(id)?.cue : undefined; }
  position(id: string): number { return this.byId.get(id)?.index ?? -1; }
}
