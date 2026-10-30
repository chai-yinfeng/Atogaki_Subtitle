import type { CandidateLabelInput } from "./themes";

export type CandidateRun = CandidateLabelInput & {
  status: string; reopenable?: boolean; released?: boolean; cleanable?: boolean; adopted?: boolean; cleanup_pending?: boolean; cleanupError?: string;
};
type Context = { jobId: string; navigation: number };
type HistoryOptions = {
  context(): Context | null;
  visible(): boolean;
  list(jobId: string): Promise<CandidateRun[]>;
  release(jobId: string, runId: string): Promise<number>;
  confirmRelease(): Promise<boolean>;
  rows(runs: readonly CandidateRun[]): void;
  busy(runId: string, busy: boolean): void;
  released(runId: string, bytes: number): void;
  failed(error: unknown): void;
};

/** Own only candidate history reads/releases. Subtitle adoption stays in the
 * processing session and backend transactions retain authority over cleanup. */
export function createCandidateHistory(options: HistoryOptions) {
  let epoch = 0, request = 0, disposed = false;
  let rows: readonly CandidateRun[] = [];
  const pending = new Map<string, object>();
  const publishRows = () => {
    options.rows(rows);
    for (const runId of pending.keys()) options.busy(runId, true);
  };
  const matches = (context: Context, generation: number) => {
    const current = options.context();
    return !disposed && generation === epoch && options.visible() && current?.jobId === context.jobId && current.navigation === context.navigation;
  };
  const refresh = async () => {
    const context = options.context(), generation = epoch, revision = ++request;
    if (!context || disposed || !options.visible()) return;
    try {
      const runs = await options.list(context.jobId);
      if (!matches(context, generation) || revision !== request) return;
      rows = runs.filter(run => !run.released || run.cleanup_pending || run.adopted || run.is_candidate === false)
        .map(run => {
          const error = run.cleanable && rows.find(old => old.id === run.id)?.cleanupError;
          return error ? { ...run, cleanupError: error } : run;
        });
      publishRows();
    } catch (error) { if (matches(context, generation) && revision === request) options.failed(error); }
  };
  const reset = () => {
    epoch++; request++; rows = []; pending.clear(); options.rows(rows);
  };
  return {
    refresh,
    markAdopted(runId: string) {
      if (disposed) return;
      // Invalidate older reads before exposing the committed adoption locally.
      request++;
      if (!rows.some(run => run.id === runId)) return;
      rows = rows.map(run => run.id === runId ? { ...run, adopted: true, cleanable: false, cleanupError: undefined } : run);
      publishRows();
    },
    async release(runId: string) {
      const context = options.context(), generation = epoch;
      if (!context || !matches(context, generation) || pending.has(runId) || !rows.some(run => run.id === runId && run.cleanable)) return;
      const ticket = {}; pending.set(runId, ticket);
      const current = () => pending.get(runId) === ticket && matches(context, generation);
      let releasing = false;
      try {
        if (!await options.confirmRelease() || !current() || !rows.some(run => run.id === runId && run.cleanable)) return;
        options.busy(runId, true);
        releasing = true;
        const bytes = await options.release(context.jobId, runId);
        if (!current()) return;
        request++; rows = rows.filter(run => run.id !== runId); publishRows();
        options.released(runId, bytes);
        await refresh();
      } catch (error) {
        if (current()) {
          if (releasing) { rows = rows.map(run => run.id === runId ? { ...run, cleanupError: String(error) } : run); publishRows(); }
          options.failed(error);
        }
      }
      finally {
        if (pending.get(runId) === ticket) {
          pending.delete(runId);
          if (matches(context, generation)) options.busy(runId, false);
        }
      }
    },
    reset,
    dispose() { reset(); disposed = true; },
  };
}

type ViewOptions = Omit<HistoryOptions, "rows" | "busy"> & {
  label(run: CandidateRun): string;
  full(): void;
  cancel(): void;
  reopen(runId: string): void;
};
/** A single delegated listener; closing releases history rows, dispose removes
 * the toolbar/listener. No appended per-row listeners or hidden history DOM. */
export function mountCandidateHistory(root: HTMLElement, options: ViewOptions) {
  const events = new AbortController(), toolbar = document.createElement("div");
  toolbar.className = "offline-tools";
  toolbar.innerHTML = '<button type="button" class="secondary" data-full-candidate>生成全片本地候选</button><button type="button" class="secondary" data-candidate-history>历史候选</button><button type="button" class="secondary" data-cancel-candidate>取消候选运行</button><div class="candidate-history"></div>';
  root.append(toolbar);
  const host = toolbar.querySelector<HTMLDivElement>(".candidate-history")!;
  let requested = false;
  const history = createCandidateHistory({ ...options,
    rows(runs) {
      const focused = document.activeElement instanceof HTMLButtonElement && host.contains(document.activeElement)
        ? document.activeElement.dataset.reopenRun ?? document.activeElement.dataset.releaseRun : null;
      host.replaceChildren();
      for (const run of runs) {
        const row = document.createElement("article"), info = document.createElement("div"), title = document.createElement("strong"), status = document.createElement("span");
        row.className = "candidate-history-row";
        title.textContent = options.label(run);
        status.textContent = run.cleanupError ? `清理失败：${run.cleanupError}` : run.cleanup_pending ? "上次清理未完成，请重试" : run.adopted ? "已采用，保留来源" : run.is_candidate === false ? "原始识别，保留来源" : ({ succeeded: "可对照", failed: "运行失败", running: "识别中", queued: "排队中" } as Record<string,string>)[run.status] ?? run.status;
        info.append(title, status); row.append(info);
        const button = (key: string, text: string) => {
          const control = document.createElement("button"); control.type = "button"; control.className = "secondary"; control.dataset[key] = run.id; control.textContent = text; row.append(control);
        };
        if (run.status === "succeeded" && run.reopenable) button("reopenRun", "对照");
        if (run.cleanable) button("releaseRun", run.cleanup_pending || run.cleanupError ? "重试清理" : "清理产物");
        host.append(row);
      }
      if (!runs.length) host.textContent = "暂无历史候选。";
      if (focused) {
        const replacement = [...host.querySelectorAll<HTMLButtonElement>("button")].find(button => (button.dataset.reopenRun ?? button.dataset.releaseRun) === focused);
        (replacement ?? toolbar.querySelector<HTMLButtonElement>("[data-candidate-history]"))?.focus();
      }
    },
    busy(runId, busy) {
      const controls = [...host.querySelectorAll<HTMLButtonElement>("[data-release-run], [data-reopen-run]")];
      controls.forEach(button => {
        if ((button.dataset.releaseRun ?? button.dataset.reopenRun) === runId) button.disabled = busy;
      });
      if (!busy && options.visible() && document.activeElement === document.body) {
        (controls.find(button => (button.dataset.releaseRun ?? button.dataset.reopenRun) === runId)
          ?? toolbar.querySelector<HTMLButtonElement>("[data-candidate-history]"))?.focus();
      }
    },
  });
  // Keep initial/closed history truly empty until explicitly requested.
  const clear = () => { requested = false; history.reset(); host.replaceChildren(); };
  toolbar.addEventListener("click", event => {
    const button = (event.target as Element).closest<HTMLButtonElement>("button");
    if (!button || button.disabled || !options.visible()) return;
    if (button.hasAttribute("data-full-candidate")) options.full();
    else if (button.hasAttribute("data-candidate-history")) { requested = true; void history.refresh(); }
    else if (button.hasAttribute("data-cancel-candidate")) options.cancel();
    else if (button.dataset.releaseRun) { button.focus(); void history.release(button.dataset.releaseRun); }
    else if (button.dataset.reopenRun) { button.focus(); options.reopen(button.dataset.reopenRun); }
  }, { signal: events.signal });
  return {
    clear,
    adopted(runId: string) {
      history.markAdopted(runId);
      if (requested) void history.refresh();
    },
    dispose() { events.abort(); history.dispose(); toolbar.remove(); } };
}
