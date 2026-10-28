import type { RetranscriptionCandidate, RetranscriptionPreview } from "./editor-processing-types";
import type { SubtitleSegment } from "./editor-types";
import { candidateTextDiff } from "./offline-tools";

type Options = {
  time(milliseconds: number): string;
  escape(value: string): string;
  refresh(): void;
  open(): void;
};

/** Candidate presentation only; the processing session owns requests and adoption. */
export function mountCandidateComparisonView(root: HTMLDialogElement, options: Options) {
  const events = new AbortController();
  const summary = root.querySelector<HTMLElement>("#retranscription-preview-summary");
  const original = root.querySelector<HTMLElement>("#retranscription-original-list");
  const candidate = root.querySelector<HTMLElement>("#retranscription-candidate-list");
  const status = root.querySelector<HTMLElement>("#retranscription-confirm-message");
  root.addEventListener("click", event => {
    const button = event.target instanceof Element ? event.target.closest<HTMLButtonElement>(".refresh-comparison") : null;
    if (button && root.contains(button) && !button.disabled) options.refresh();
  }, { signal: events.signal });
  function renderItems(host: HTMLElement | null, items: Array<SubtitleSegment | RetranscriptionCandidate>): void {
    if (!host) return;
    host.replaceChildren();
    if (!items.length) {
      const empty = document.createElement("p");
      empty.className = "muted";
      empty.textContent = "这个范围没有字幕。";
      host.append(empty);
    }
    for (const item of items) {
      const row = document.createElement("article");
      const time = document.createElement("span");
      const text = document.createElement("p");
      time.textContent = `${options.time(item.start_ms)} → ${options.time(item.end_ms)}`;
      text.textContent = item.source_text;
      row.append(time, text);
      host.append(row);
    }
  }
  return {
    show(preview: RetranscriptionPreview): void {
      if (events.signal.aborted) return;
      if (summary) summary.textContent = `${options.time(preview.start_ms)} → ${options.time(preview.end_ms)} · 当前 ${preview.original_segments.length} 段 · 候选 ${preview.candidate_segments.length} 段 · ${preview.provider_name ?? "识别候选"} · ${preview.model_identity ?? ""} · ${preview.context_mode === "isolated" ? "前文文字已隔离" : "独立识别候选"}`;
      renderItems(original, preview.original_segments);
      renderItems(candidate, preview.candidate_segments);
      if (status) status.textContent = "";
      root.querySelector(".candidate-diff")?.remove();
      root.querySelector(".refresh-comparison")?.remove();
      const refresh = document.createElement("button");
      refresh.type = "button";
      refresh.className = "secondary refresh-comparison";
      refresh.textContent = "以当前字幕重新对照（不重跑识别）";
      root.querySelector(".retranscription-comparison")?.before(refresh);
      const diff = document.createElement("details");
      diff.className = "candidate-diff";
      diff.innerHTML = `<summary>文本差异与运行参数</summary><p>${candidateTextDiff(preview.original_segments.map(s => s.source_text).join("\n"), preview.candidate_segments.map(s => s.source_text).join("\n"))}</p><p>时间证据：${options.escape(preview.timing_granularities?.join(" / ") || "未知")} · 工作区版本：${preview.workspace_revision ?? "历史候选，采用前需重新对照"}</p><pre>${options.escape(JSON.stringify(preview.configuration ?? {}, null, 2))}</pre>`;
      root.querySelector(".retranscription-comparison")?.after(diff);
      options.open();
    },
    dispose(): void {
      events.abort();
      root.querySelector(".candidate-diff")?.remove();
      root.querySelector(".refresh-comparison")?.remove();
      original?.replaceChildren();
      candidate?.replaceChildren();
      if (summary) summary.textContent = "";
    },
  };
}
