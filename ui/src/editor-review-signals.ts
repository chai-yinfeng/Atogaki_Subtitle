import type { AsrQualitySignal, ReviewDecision } from "./editor-processing-types";

type Options = {
  time(ms: number): string;
  escape(value: string): string;
  locate(signal: AsrQualitySignal): void;
  repair(signal: AsrQualitySignal): void;
  review(signalId: string, decision: ReviewDecision): void;
};
/** The review view retains only display/filter state. Reads, writes and the
 * editor's review-filter index stay with the existing processing session. */
export function mountReviewSignals(dialog: HTMLDialogElement, options: Options) {
  const host = dialog.querySelector<HTMLDivElement>("#asr-quality-signals")!;
  const disclosure = dialog.querySelector<HTMLDetailsElement>("#asr-quality-review");
  const summary = dialog.querySelector<HTMLElement>("#asr-quality-summary");
  const events = new AbortController();
  let jobId = "", signalsAll: readonly AsrQualitySignal[] = [], showProcessedSignals = false;
  const clear = () => { signalsAll = []; host.replaceChildren(); };
  const prepare = (nextJob: string) => {
    if (jobId === nextJob) return;
    jobId = nextJob; showProcessedSignals = false; clear();
    if (disclosure) { disclosure.open = false; disclosure.dataset.jobId = jobId; }
    if (summary) summary.textContent = "转录待复核提示 · 正在读取";
  };
  const render = () => {
    if (!dialog.open) { host.replaceChildren(); return; }
    const processedCount = signalsAll.filter(s => s.latest_decision === "false_positive" || s.latest_decision === "resolved").length;
    const signals = signalsAll.filter(s => showProcessedSignals || (s.latest_decision !== "false_positive" && s.latest_decision !== "resolved"));
    if (summary) summary.textContent = `转录待复核提示 · ${signals.length} 项，${signals.filter(signal => signal.latest_decision === null).length} 项待复核${!showProcessedSignals && processedCount ? ` · 已隐藏 ${processedCount} 项` : ""}`;
    const filterBar = `<label class="asr-review-filter"><input type="checkbox" data-show-processed ${showProcessedSignals ? "checked" : ""}>显示已处理（${processedCount}）</label>`;
    if (signals.length === 0) {
      host.innerHTML = filterBar + `<p class="media-message">${processedCount ? '误报和已解决提示已隐藏。可勾选“显示已处理”找回。' : '未发现自动提示，仍可根据试听结果手动选择重新识别的范围。'}</p>`;
      return;
    }
    const signalLabels: Record<AsrQualitySignal["signal_kind"], string> = {
      boundary_repetition: "边界文字重复", repeated_loop: "相邻字幕重复",
      abnormal_speech_rate: "文字与时长不匹配", speech_during_silence: "静音区出现文字", manual_concern: "接缝待复核",
    };
    host.innerHTML = filterBar + '<p class="media-message">以下提示由转录结果自动检查产生，不是重跑或翻译历史，也不代表一定识别错误。点击提示可定位时间轴；试听后可确认问题或标记误报。选取范围后，再点击“识别并预览”生成候选。</p>' + signals.map((signal) => {
      const range = signal.start_ms === null || signal.end_ms === null
        ? "未指定范围"
        : `${options.time(signal.start_ms)} → ${options.time(signal.end_ms)}`;
      const review = signal.latest_decision === "confirmed_problem" ? "已确认问题"
        : signal.latest_decision === "false_positive" ? "已标记误报"
          : signal.latest_decision === "resolved" ? "已修复" : "待审查";
      const checked = signal.latest_decision !== null;
      return `${checked ? `<details class="reviewed-signal"><summary>${range} · ${options.escape(signalLabels[signal.signal_kind])} · ${review}</summary>` : ""}<article class="asr-quality-signal" data-severity="${signal.severity}">
        <div>${signal.start_ms === null || signal.end_ms === null
          ? `<p>${options.escape(signal.message)}</p><span>${range} · ${options.escape(signalLabels[signal.signal_kind] ?? "待复核")} · ${review}</span>`
          : `<button type="button" class="asr-quality-locate" data-locate-signal="${options.escape(signal.id)}" data-job-id="${options.escape(jobId)}" data-start-ms="${signal.start_ms}" data-end-ms="${signal.end_ms}" aria-label="定位 ${range}：${options.escape(signal.message)}"><span class="asr-quality-message">${options.escape(signal.message)}</span><span>${range} · ${options.escape(signalLabels[signal.signal_kind] ?? "待复核")} · ${review} · 点击定位</span></button>`}</div>
        <div class="asr-quality-actions"><button type="button" class="secondary" data-asr-review="confirmed_problem" data-signal-id="${options.escape(signal.id)}">确认问题</button><button type="button" class="secondary" data-asr-review="false_positive" data-signal-id="${options.escape(signal.id)}">误报</button><button type="button" class="secondary" data-asr-review="resolved" data-signal-id="${options.escape(signal.id)}">已解决</button>${signal.start_ms === null || signal.end_ms === null ? "" : `<button type="button" data-repair-signal="${options.escape(signal.id)}" data-start-ms="${signal.start_ms}" data-end-ms="${signal.end_ms}">选取此范围</button>`}</div>
      </article>${checked ? "</details>" : ""}`;
    }).join("");
  };
  host.addEventListener("change", event => {
    const input = event.target;
    if (input instanceof HTMLInputElement && input.matches("[data-show-processed]")) { showProcessedSignals = input.checked; render(); host.querySelector<HTMLInputElement>("[data-show-processed]")?.focus(); }
  }, { signal: events.signal });
  host.addEventListener("click", event => {
    const button = (event.target as Element).closest<HTMLButtonElement>("button");
    if (!button || !dialog.open) return;
    const signal = signalsAll.find(value => value.id === (button.dataset.locateSignal ?? button.dataset.signalId ?? button.dataset.repairSignal));
    if (!signal) return;
    if (button.dataset.locateSignal) options.locate(signal);
    else if (button.dataset.repairSignal) options.repair(signal);
    else if (["confirmed_problem", "false_positive", "resolved"].includes(button.dataset.asrReview ?? "")) options.review(signal.id, button.dataset.asrReview as ReviewDecision);
  }, { signal: events.signal });
  return {
    prepare,
    show(nextJob: string, signals: readonly AsrQualitySignal[]) { prepare(nextJob); signalsAll = signals; render(); },
    failed(error: unknown) {
      if (!dialog.open) return;
      if (summary) summary.textContent = "转录待复核提示 · 读取失败";
      host.innerHTML = `<p class="media-message warning">无法读取 ASR 审查信号：${options.escape(String(error))}</p>`;
    },
    clear,
    dispose() { events.abort(); clear(); },
  };
}
