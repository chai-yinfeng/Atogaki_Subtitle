import type { EditorCue, EditorSession } from "./editor-session";

type FollowOptions<T extends EditorCue> = {
  session: EditorSession;
  current(): T | undefined;
  visible(): readonly T[];
  busy(): boolean;
  jobId(): string | null;
  navigation(): number;
  active(): boolean;
  flush(): Promise<boolean>;
  time(ms: number): string;
  status(message: string, error?: boolean): void;
  resumed(): void;
};

/** Coordinate follow intent without owning a player, draft queue or backend. */
export function mountEditorFollow<T extends EditorCue>(root: HTMLElement, options: FollowOptions<T>) {
  const { session } = options;
  const start = root.querySelector<HTMLInputElement>("#editor-cue-start")!;
  const end = root.querySelector<HTMLInputElement>("#editor-cue-end")!;
  const button = root.querySelector<HTMLButtonElement>('[data-editor-action="follow"]')!;
  const reveal = root.querySelector<HTMLButtonElement>('[data-editor-action="reveal-playing"]')!;
  const message = root.querySelector<HTMLElement>("#editor-selection-message")!;
  const events = new AbortController();
  let intent = 0;
  const syncMessage = () => {
    const id = session.selectedCueId;
    const hidden = id && !options.visible().some(cue => cue.id === id);
    const mode = session.followPlayback ? (id ? "跟随播放中" : "当前位置没有字幕") : "已锁定编辑段";
    const text = `${mode}${hidden ? " · 当前段不在筛选结果中" : ""}${!session.listAutoScroll ? " · 列表自动滚动已暂停" : ""} · 单击锁定编辑，双击或时间码定位播放`;
    if (message.textContent !== text) message.textContent = text;
  };
  const sync = () => {
    button.setAttribute("aria-pressed", String(session.followPlayback));
    button.textContent = session.followPlayback ? "正在跟随播放" : "跟随播放";
    reveal.hidden = session.listAutoScroll;
    syncMessage();
  };
  const lock = (id = session.selectedCueId) => { intent++; session.lock(id); sync(); };
  const timingDirty = () => {
    const cue = options.current();
    return !!cue && (start.value !== options.time(cue.start_ms) || end.value !== options.time(cue.end_ms));
  };
  const resume = async () => {
    if (options.busy() || !options.jobId()) return;
    if (timingDirty()) { options.status("请先保存时间，或在时间输入框按 Escape 恢复，再跟随播放。", true); return; }
    const navigation = options.navigation(), jobId = options.jobId(), request = ++intent;
    if (!await options.flush() || events.signal.aborted || request !== intent || navigation !== options.navigation() || options.jobId() !== jobId || !options.active()) return;
    session.resumePlayback(); options.resumed(); sync();
  };
  for (const input of [start, end]) input.addEventListener("keydown", event => {
    if (event.key !== "Escape") return;
    event.preventDefault(); event.stopPropagation();
    const cue = options.current();
    start.value = cue ? options.time(cue.start_ms) : "";
    end.value = cue ? options.time(cue.end_ms) : "";
    options.status("已恢复当前段尚未保存的时间输入。");
  }, { signal: events.signal });
  root.addEventListener("focusin", event => {
    if ((event.target as HTMLElement).closest(".karaoke-current-segment")) lock();
  }, { signal: events.signal });
  return { lock, resume, sync, syncMessage, timingDirty,
    browse() { session.listAutoScroll = false; sync(); },
    dispose() { intent++; events.abort(); },
  };
}
