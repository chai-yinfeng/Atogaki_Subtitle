import { mountToolDialog } from "./task-tools";

type Context = { jobId: string; navigation: number };
type Insertion = { startMs: number; endMs: number; source: string; translation: string };
type Options = {
  trigger?: HTMLElement;
  context(): Context | null;
  time(ms: number): string;
  parse(value: string): number | null;
  overlaps(startMs: number, endMs: number): boolean;
  submit(context: Context, insertion: Insertion): Promise<string | null>;
};

/** One retained form over the existing structure writer; no independent drafts. */
export function mountInsertSubtitleBlock(root: HTMLElement, options: Options) {
  const dialog = document.createElement("dialog");
  dialog.id = "insert-subtitle-block-dialog";
  dialog.className = "insert-block-dialog editor-insert-dialog";
  dialog.innerHTML = '<form><h2>插入字幕块</h2><p>在空白区间新增字幕，不移动已有块。</p><fieldset><label>开始<input name="start" required></label><label>结束<input name="end" required></label><label>原文<textarea name="source" rows="3" required></textarea></label><label>译文（可选）<textarea name="translation" rows="2"></textarea></label></fieldset><p role="status" tabindex="-1"></p><div class="confirmation-actions"><button type="button" class="secondary" data-close>取消</button><button type="submit">插入</button></div></form>';
  root.append(dialog);
  const events = new AbortController();
  const form = dialog.querySelector("form")!;
  const fieldset = dialog.querySelector("fieldset")!;
  const field = (name: string) => dialog.querySelector<HTMLInputElement | HTMLTextAreaElement>(`[name="${name}"]`)!;
  const status = dialog.querySelector<HTMLElement>('[role="status"]')!;
  const buttons = [...dialog.querySelectorAll<HTMLButtonElement>("button")];
  let disposed = false, busy = false, composing = false, generation = 0;
  let opened: (Context & { durationMs: number }) | null = null;
  const setBusy = (value: boolean) => {
    busy = value; fieldset.disabled = value;
    for (const button of buttons) button.disabled = value;
    status.tabIndex = value ? 0 : -1;
    dialog.setAttribute("aria-busy", String(value));
  };
  const boundary = mountToolDialog(dialog, () => {
    generation++; opened = null; composing = false; setBusy(false); form.reset(); status.textContent = "";
  }, () => !busy);
  const current = (context: Context, ticket: number) => {
    if (disposed || !dialog.open || ticket !== generation) return false;
    const now = options.context();
    return now?.jobId === context.jobId && now.navigation === context.navigation;
  };
  form.addEventListener("compositionstart", () => { composing = true; }, { signal: events.signal });
  form.addEventListener("compositionend", () => { composing = false; }, { signal: events.signal });
  dialog.querySelector("[data-close]")!.addEventListener("click", () => boundary.close(), { signal: events.signal });
  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (disposed || busy || !opened) return;
    const context = opened, ticket = generation;
    if (!current(context, ticket)) { boundary.close(true); return; }
    if (composing) { status.textContent = "请先完成输入法选字，再插入字幕。"; return; }
    const startMs = options.parse(field("start").value), endMs = options.parse(field("end").value);
    const source = field("source").value.trim(), translation = field("translation").value.trim();
    if (startMs === null || endMs === null || startMs < 0 || endMs <= startMs || endMs > context.durationMs || !source || options.overlaps(startMs, endMs)) {
      status.textContent = "请输入有效原文和媒体范围内的空白区间，不能与已有字幕重叠。"; return;
    }
    setBusy(true); status.textContent = "正在插入字幕块…"; status.focus();
    try {
      const error = await options.submit(context, { startMs, endMs, source, translation });
      if (!current(context, ticket)) return;
      if (error === null) boundary.close(true);
      else status.textContent = error;
    } catch (error) {
      if (current(context, ticket)) status.textContent = `插入失败：${String(error)}`;
    } finally {
      if (!disposed && ticket === generation && dialog.open) {
        if (!current(context, ticket)) boundary.close(true);
        else { setBusy(false); status.tabIndex = 0; status.focus(); }
      }
    }
  }, { signal: events.signal });
  return {
    open(range: { startMs: number; endMs: number; durationMs: number }) {
      if (disposed || dialog.open) return;
      const context = options.context(); if (!context) return;
      opened = { ...context, durationMs: range.durationMs }; generation++;
      form.reset(); setBusy(false); composing = false;
      field("start").value = options.time(range.startMs);
      field("end").value = options.time(Math.max(range.startMs, range.endMs));
      status.textContent = range.endMs <= range.startMs ? "当前位置后方没有空白，请手动选择其他空白区间，或先修剪已有字幕边界。" : "";
      options.trigger?.focus(); boundary.open(); field("start").focus();
    },
    cancel() { boundary.close(true); },
    dispose() { if (disposed) return; disposed = true; events.abort(); boundary.dispose(); dialog.remove(); },
  };
}
