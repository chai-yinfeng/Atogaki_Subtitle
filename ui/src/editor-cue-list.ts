import type { EditorCue } from "./editor-session";
import { cueRowHeight, cueWindow } from "./editor-virtual-window";
export type CueRow = EditorCue & { segment_index: number };
type ListCallbacks<T> = { select(id: string): boolean; seek(id: string): void; browse(): void; time(ms: number): string; status(cue: T): string };
/** Own only viewport rows. Cue snapshots and drafts live outside the DOM. */
export function mountEditorCueList<T extends CueRow>(host: HTMLElement, callbacks: ListCallbacks<T>) {
  const events = new AbortController();
  const content = document.createElement("div");
  content.className = "editor-cue-content";
  content.setAttribute("role", "rowgroup");
  const rows = new Map<string, HTMLElement>();
  let cues: readonly T[] = [], positions = new Map<string, number>();
  let selected: string | null = null, playback: string | null = null;
  let frame: number | null = null;
  let windowStart = -1, windowEnd = -1;
  host.tabIndex = 0;

  const patch = (row: HTMLElement, cue: T) => {
    const values = [String(cue.segment_index + 1), `${callbacks.time(cue.start_ms)} → ${callbacks.time(cue.end_ms)}`, cue.source_text, cue.translated_text ?? "尚无译文", callbacks.status(cue)];
    [...row.children].forEach((cell, index) => {
      if (cell.textContent !== values[index]) { cell.textContent = values[index]; cell.setAttribute("title", values[index]); }
    });
    row.classList.toggle("selected", cue.id === selected);
    row.classList.toggle("playing", cue.id === playback);
    row.setAttribute("aria-selected", String(cue.id === selected));
    if (cue.id === playback) row.setAttribute("aria-current", "true");
    else row.removeAttribute("aria-current");
    row.setAttribute("aria-rowindex", String((positions.get(cue.id) ?? 0) + 1));
    row.tabIndex = cue.id === selected ? 0 : -1;
  };
  const draw = (refresh = false) => {
    if (frame !== null) { cancelAnimationFrame(frame); frame = null; }
    const { start, end } = cueWindow(cues.length, host.scrollTop, host.clientHeight);
    // Pixel scrolling within a fixed-height row does not change its DOM window.
    // Explicit render/update/mark paths still refresh changed data and state.
    if (!refresh && start === windowStart && end === windowEnd) return;
    windowStart = start; windowEnd = end;
    const wanted = new Set(cues.slice(start, end).map(cue => cue.id));
    for (const [id, row] of rows) if (!wanted.has(id)) {
      if (row.contains(document.activeElement)) host.focus({ preventScroll: true });
      row.remove(); rows.delete(id);
    }
    for (let index = start; index < end; index++) {
      const cue = cues[index];
      let row = rows.get(cue.id);
      if (!row) {
        row = document.createElement("div"); row.className = "editor-cue-row"; row.dataset.cueId = cue.id;
        row.setAttribute("role", "row");
        row.innerHTML = '<span role="gridcell"></span><button type="button" class="secondary" data-cue-time role="gridcell"></button><span role="gridcell" class="cue-source"></span><span role="gridcell" class="cue-translation"></span><span role="gridcell" class="cue-status"></span>';
        rows.set(cue.id, row);
        row.style.top = `${index * cueRowHeight}px`;
        patch(row, cue);
      } else if (refresh) {
        // Filtering and structural changes can move an existing cue to a new row.
        row.style.top = `${index * cueRowHeight}px`;
        patch(row, cue);
      }
    }
    if (!cues.length) {
      if (!content.querySelector(".empty-state")) {
        const empty = document.createElement("p"); empty.className = "empty-state";
        empty.textContent = "没有符合条件的字幕。"; content.replaceChildren(empty);
      }
    } else {
      content.querySelector(".empty-state")?.remove();
      // Keep retained nodes attached. A blur-triggered save can complete between
      // pointerdown and click; reparenting the target suppresses click in WebKit.
      let cursor = content.firstChild;
      for (let index = start; index < end; index++) {
        const row = rows.get(cues[index].id)!;
        if (row === cursor) cursor = cursor.nextSibling;
        else content.insertBefore(row, cursor);
      }
    }
    if (!content.isConnected) host.replaceChildren(content);
  };
  const schedule = () => { if (frame === null) frame = requestAnimationFrame(() => draw()); };
  const reveal = (id: string | null) => {
    const index = id ? positions.get(id) : undefined;
    if (index === undefined) return;
    const top = index * cueRowHeight;
    if (top < host.scrollTop) host.scrollTop = top;
    else if (top + cueRowHeight > host.scrollTop + host.clientHeight) host.scrollTop = top + cueRowHeight - host.clientHeight;
    draw();
  };
  const mark = (nextSelected: string | null, nextPlayback: string | null, follow = false) => {
    const oldSelected = selected, oldPlayback = playback;
    selected = nextSelected; playback = nextPlayback;
    for (const id of new Set([oldSelected, oldPlayback, selected, playback])) {
      const index = id ? positions.get(id) : undefined, row = id ? rows.get(id) : undefined;
      if (row && index !== undefined) patch(row, cues[index]);
    }
    if (follow && oldPlayback !== playback) reveal(playback);
  };
  const rowForEvent = (event: Event) => (event.target as HTMLElement).closest<HTMLElement>("[data-cue-id]");
  host.addEventListener("click", event => {
    const id = rowForEvent(event)?.dataset.cueId;
    if (!id) return;
    if (callbacks.select(id) && (event.target as HTMLElement).closest("[data-cue-time]")) callbacks.seek(id);
  }, { signal: events.signal });
  host.addEventListener("dblclick", event => {
    const id = rowForEvent(event)?.dataset.cueId;
    if (id && !(event.target as HTMLElement).closest("[data-cue-time]") && callbacks.select(id)) callbacks.seek(id);
  }, { signal: events.signal });
  host.addEventListener("keydown", event => {
    const row = rowForEvent(event);
    if (["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
      event.preventDefault(); event.stopPropagation();
      const current = positions.get(row?.dataset.cueId ?? selected ?? "") ?? 0;
      const index = event.key === "Home" ? 0 : event.key === "End" ? cues.length - 1
        : Math.max(0, Math.min(cues.length - 1, current + (event.key === "ArrowUp" ? -1 : 1)));
      if (cues[index] && callbacks.select(cues[index].id)) { reveal(cues[index].id); rows.get(cues[index].id)?.focus({ preventScroll: true }); }
    } else if (row?.dataset.cueId && (event.key === "Enter" || event.key === " ") && event.target === row) {
      event.preventDefault(); callbacks.select(row.dataset.cueId);
    }
  }, { signal: events.signal });
  for (const event of ["wheel", "touchmove"] as const) host.addEventListener(event, callbacks.browse, { passive: true, signal: events.signal });
  host.addEventListener("pointerdown", event => {
    // The scrollbar is manual browsing; selecting a row is a separate action.
    if (!rowForEvent(event)) callbacks.browse();
  }, { passive: true, signal: events.signal });
  host.addEventListener("keydown", event => {
    if (["PageUp", "PageDown"].includes(event.key)) callbacks.browse();
  }, { signal: events.signal });
  host.addEventListener("scroll", schedule, { passive: true, signal: events.signal });
  const resize = new ResizeObserver(schedule); resize.observe(host);
  return {
    render(next: readonly T[], nextSelected: string | null, nextPlayback: string | null) {
      if (cues !== next) {
        cues = next; positions = new Map(cues.map((cue, index) => [cue.id, index]));
        content.style.height = cues.length ? `${cues.length * cueRowHeight}px` : "auto";
        // Filtering or structural edits can shorten the scroll extent.
        host.scrollTop = Math.min(host.scrollTop, Math.max(0, cues.length * cueRowHeight - host.clientHeight));
      }
      selected = nextSelected; playback = nextPlayback;
      host.setAttribute("aria-rowcount", String(cues.length)); draw(true);
    },
    update(cue: T) { const row = rows.get(cue.id); if (row) patch(row, cue); },
    clear() {
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null; cues = []; positions.clear(); rows.clear(); content.replaceChildren();
      windowStart = -1; windowEnd = -1;
      content.style.height = "0px"; host.scrollTop = 0; host.setAttribute("aria-rowcount", "0");
    },
    mark, reveal,
    dispose() { events.abort(); resize.disconnect(); if (frame !== null) cancelAnimationFrame(frame); rows.clear(); host.replaceChildren(); },
  };
}
