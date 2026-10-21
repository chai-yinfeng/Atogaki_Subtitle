import type { EditorCue } from "./editor-session";
export type CueRow = EditorCue & { segment_index: number };
export function mountEditorCueList<T extends CueRow>(host: HTMLElement, callbacks: { select(id: string): void; seek(id: string): void; browse(): void; time(ms: number): string; status(cue: T): string }): {
  render(cues: readonly T[], selectedId: string | null, playbackId: string | null): void;
  mark(selectedId: string | null, playbackId: string | null, follow?: boolean): void;
  reveal(id: string | null): void;
  dispose(): void;
} {
  const events = new AbortController();
  const rows = new Map<string, HTMLElement>();
  let lastOrder = "";
  let previousSelected: string | null = null, previousPlayback: string | null = null;
  const rowForEvent = (event: Event) => (event.target as HTMLElement).closest<HTMLElement>("[data-cue-id]");
  host.addEventListener("click", event => {
    const row = rowForEvent(event);
    if (!row?.dataset.cueId) return;
    callbacks.select(row.dataset.cueId);
    if ((event.target as HTMLElement).closest("[data-cue-time]")) callbacks.seek(row.dataset.cueId);
  }, { signal: events.signal });
  host.addEventListener("dblclick", event => {
    const row = rowForEvent(event);
    if (row?.dataset.cueId && !(event.target as HTMLElement).closest("[data-cue-time]")) callbacks.seek(row.dataset.cueId);
  }, { signal: events.signal });
  host.addEventListener("keydown", event => {
    const row = rowForEvent(event);
    if (row?.dataset.cueId && ["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
      event.preventDefault(); event.stopPropagation();
      const visible = [...host.querySelectorAll<HTMLElement>("[data-cue-id]")];
      const index = visible.indexOf(row);
      const target = visible[event.key === "Home" ? 0 : event.key === "End" ? visible.length - 1 : Math.max(0, Math.min(visible.length - 1, index + (event.key === "ArrowUp" ? -1 : 1)))];
      if (target?.dataset.cueId) { callbacks.select(target.dataset.cueId); target.focus({ preventScroll: true }); reveal(target.dataset.cueId); }
      return;
    }
    if (row?.dataset.cueId && (event.key === "Enter" || event.key === " ") && event.target === row) {
      event.preventDefault(); callbacks.select(row.dataset.cueId);
    }
  }, { signal: events.signal });
  for (const event of ["wheel", "touchstart", "pointerdown"] as const) host.addEventListener(event, callbacks.browse, { passive: true, signal: events.signal });
  const reveal = (id: string | null) => {
    const row = id ? rows.get(id) : undefined;
    if (!row?.isConnected) return;
    const top = row.offsetTop - host.offsetTop;
    if (top < host.scrollTop) host.scrollTop = top;
    else if (top + row.offsetHeight > host.scrollTop + host.clientHeight) host.scrollTop = top + row.offsetHeight - host.clientHeight;
  };
  const mark = (selected: string | null, playback: string | null, follow = false) => {
    for (const id of new Set([previousSelected, previousPlayback, selected, playback])) {
      const row = id ? rows.get(id) : undefined;
      if (!row) continue;
      row.classList.toggle("selected", id === selected);
      row.classList.toggle("playing", id === playback);
      row.setAttribute("aria-selected", String(id === selected));
      row.tabIndex = id === selected ? 0 : -1;
    }
    if (follow && previousPlayback !== playback) reveal(playback);
    previousSelected = selected; previousPlayback = playback;
  };
  return {
    render(cues, selected, playback) {
      const ids = new Set(cues.map(cue => cue.id));
      for (const [id, row] of rows) if (!ids.has(id)) { row.remove(); rows.delete(id); }
      for (const cue of cues) {
        let row = rows.get(cue.id);
        if (!row) {
          row = document.createElement("div"); row.className = "editor-cue-row"; row.dataset.cueId = cue.id;
          row.setAttribute("role", "row");
          row.innerHTML = '<span role="gridcell"></span><button type="button" class="secondary" data-cue-time role="gridcell"></button><span role="gridcell" class="cue-source"></span><span role="gridcell" class="cue-translation"></span><span role="gridcell" class="cue-status"></span>';
          rows.set(cue.id, row);
        }
        const values = [String(cue.segment_index + 1), `${callbacks.time(cue.start_ms)} → ${callbacks.time(cue.end_ms)}`, cue.source_text, cue.translated_text ?? "尚无译文", callbacks.status(cue)];
        [...row.children].forEach((cell, index) => { if (cell.textContent !== values[index]) cell.textContent = values[index]; cell.setAttribute("title", values[index]); });
        row.tabIndex = cue.id === selected ? 0 : -1;
      }
      const order = JSON.stringify(cues.map(cue => cue.id));
      if (order !== lastOrder || host.childElementCount === 0) {
        const fragment = document.createDocumentFragment();
        if (!cues.length) { const empty = document.createElement("p"); empty.className = "empty-state"; empty.textContent = "没有符合条件的字幕。"; fragment.append(empty); }
        else for (const cue of cues) fragment.append(rows.get(cue.id)!);
        host.replaceChildren(fragment); lastOrder = order;
      }
      host.setAttribute("aria-rowcount", String(cues.length));
      mark(selected, playback);
    }, mark, reveal,
    dispose() { events.abort(); rows.clear(); host.replaceChildren(); },
  };
}
