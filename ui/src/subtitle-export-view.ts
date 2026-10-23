import { mountToolDialog } from "./task-tools";
import type { SubtitleExportArtifact } from "./subtitle-export-types";

/** Mount a single retained set of format controls; close parks both regions
 * off-document and resolves pending choices. No native nested modal filtering. */
export function mountSubtitleExportDialogs(root: HTMLElement) {
  const dialog = document.createElement("dialog");
  dialog.id = "subtitle-export-dialog"; dialog.className = "rename-job-dialog subtitle-export-dialog";
  dialog.innerHTML = `<form id="subtitle-export-form"><div class="dialog-heading"><div><p class="eyebrow">SUBTITLE FILES</p><h2>选择字幕文件</h2></div><button id="cancel-subtitle-export" type="button" class="secondary">取消</button></div><p>原文 SRT 可以独立导出；含译文的文件要求全部译文已完成且没有过期。</p><div class="subtitle-export-options"><label><input type="checkbox" name="subtitle-artifact" value="source_srt" checked />原文 SRT</label><label><input type="checkbox" name="subtitle-artifact" value="translated_srt" checked />译文 SRT</label><label><input type="checkbox" name="subtitle-artifact" value="bilingual_srt" checked />双语 SRT</label><label><input type="checkbox" name="subtitle-artifact" value="bilingual_ass" checked />双语 ASS</label></div><div class="rename-job-footer"><span id="subtitle-export-message" role="status"></span><button type="submit">选择目录</button></div></form>`;
  const confirm = document.createElement("dialog");
  confirm.id = "subtitle-export-overwrite-dialog"; confirm.className = "rename-job-dialog subtitle-export-overwrite-dialog";
  confirm.innerHTML = '<h2>覆盖已有字幕？</h2><p data-overwrite-message></p><pre data-overwrite-files></pre><div class="confirmation-actions"><button type="button" class="secondary" data-cancel-overwrite>取消</button><button type="button" class="danger" data-accept-overwrite>覆盖字幕</button></div>';
  root.append(dialog, confirm);
  const events = new AbortController();
  let disposed = false;
  let trigger: HTMLButtonElement | null = null;
  let formats: ((value: SubtitleExportArtifact[] | null) => void) | null = null;
  let overwrite: ((value: boolean) => void) | null = null;
  const formatBoundary = mountToolDialog(dialog, () => { const resolve = formats; formats = null; resolve?.(null); });
  const overwriteBoundary = mountToolDialog(confirm, () => { const resolve = overwrite; overwrite = null; resolve?.(false); });
  const finishFormats = (value: SubtitleExportArtifact[] | null) => { const resolve = formats; formats = null; formatBoundary.close(); resolve?.(value); };
  const finishOverwrite = (value: boolean) => { const resolve = overwrite; overwrite = null; overwriteBoundary.close(); resolve?.(value); };
  dialog.querySelector("form")!.addEventListener("submit", event => {
    event.preventDefault();
    const selected = [...dialog.querySelectorAll<HTMLInputElement>('input[name="subtitle-artifact"]:checked')].map(input => input.value as SubtitleExportArtifact);
    if (!selected.length) { dialog.querySelector("#subtitle-export-message")!.textContent = "请至少选择一种字幕文件。"; return; }
    finishFormats(selected);
  }, { signal: events.signal });
  dialog.querySelector("#cancel-subtitle-export")!.addEventListener("click", () => finishFormats(null), { signal: events.signal });
  confirm.querySelector("[data-cancel-overwrite]")!.addEventListener("click", () => finishOverwrite(false), { signal: events.signal });
  confirm.querySelector("[data-accept-overwrite]")!.addEventListener("click", () => finishOverwrite(true), { signal: events.signal });
  const cancel = () => { finishOverwrite(false); finishFormats(null); };
  return {
    bindActions(exportButton: HTMLButtonElement | null, revealButton: HTMLButtonElement | null, start: () => void, reveal: () => void) {
      exportButton?.addEventListener("click", () => { trigger = exportButton; exportButton.focus(); start(); }, { signal: events.signal });
      revealButton?.addEventListener("click", () => { trigger = revealButton; revealButton.focus(); reveal(); }, { signal: events.signal });
    },
    idle() {
      if (!disposed && document.activeElement === document.body && trigger?.isConnected && !trigger.disabled && !trigger.closest("[inert]") && trigger.getClientRects().length) trigger.focus();
    },
    choose(): Promise<SubtitleExportArtifact[] | null> {
      if (disposed) return Promise.resolve(null);
      finishFormats(null);
      dialog.querySelector("#subtitle-export-message")!.textContent = "";
      return new Promise(resolve => { formats = resolve; formatBoundary.open(); });
    },
    overwrite(names: string[]): Promise<boolean> {
      if (disposed) return Promise.resolve(false);
      finishOverwrite(false);
      confirm.querySelector("[data-overwrite-message]")!.textContent = `以下 ${names.length} 个字幕文件已存在：`;
      confirm.querySelector("[data-overwrite-files]")!.textContent = names.join("\n");
      return new Promise(resolve => { overwrite = resolve; overwriteBoundary.open(); });
    },
    cancel,
    dispose() { if (disposed) return; disposed = true; cancel(); events.abort(); formatBoundary.dispose(); overwriteBoundary.dispose(); dialog.remove(); confirm.remove(); },
  };
}
