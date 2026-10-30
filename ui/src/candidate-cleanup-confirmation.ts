import { mountToolDialog } from "./task-tools";

/** A processing-only confirmation: native modals nested over the review region
 * hide all controls from WKWebView accessibility. Other app dialogs retain their
 * existing behavior. The history controller still owns authorization and release. */
export function mountCandidateCleanupConfirmation(root: HTMLElement) {
  const dialog = document.createElement("dialog");
  dialog.id = "candidate-cleanup-dialog";
  dialog.className = "editor-cleanup-dialog";
  dialog.innerHTML = '<h2>清理这个未采用候选？</h2><p>释放候选的音频缓存和识别产物，后台保留运行摘要。清理成功后从历史候选列表移除，无法再打开；当前字幕不变。</p><div class="confirmation-actions"><button type="button" class="secondary" data-cancel-cleanup>取消</button><button type="button" class="danger" data-accept-cleanup>清理产物</button></div>';
  root.append(dialog);
  const events = new AbortController();
  let disposed = false;
  let pending: ((accepted: boolean) => void) | null = null;
  const boundary = mountToolDialog(dialog, () => {
    const resolve = pending;
    pending = null;
    resolve?.(false);
  });
  const settle = (accepted: boolean) => {
    const resolve = pending;
    pending = null;
    boundary.close();
    resolve?.(accepted);
  };
  dialog.querySelector("[data-cancel-cleanup]")!.addEventListener("click", () => settle(false), { signal: events.signal });
  dialog.querySelector("[data-accept-cleanup]")!.addEventListener("click", () => settle(true), { signal: events.signal });
  return {
    get open() { return dialog.open; },
    confirm(): Promise<boolean> {
      if (disposed) return Promise.resolve(false);
      if (pending) settle(false);
      return new Promise(resolve => { pending = resolve; boundary.open(); });
    },
    cancel() { if (pending || dialog.open) settle(false); },
    dispose() { disposed = true; settle(false); events.abort(); boundary.dispose(); dialog.remove(); },
  };
}
