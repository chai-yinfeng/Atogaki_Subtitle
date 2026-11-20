import { invoke } from "@tauri-apps/api/core";
import { escaped, matchesQuery } from "./offline-tools";

export type GrammarSelection = {
  jobId: string;
  segmentId: string;
  sourceText: string;
  start: number;
  end: number;
  endCue?: { segmentId: string; sourceText: string };
};
type GrammarMatch = { id: string; start: number; end: number; formNote?: string };
type GrammarPreview = { text: string; focusStart: number; focusEnd: number; matches: GrammarMatch[] };
export type SaveGrammarMatch = Pick<GrammarMatch, "id" | "start" | "end"> & {
  selection: GrammarSelection;
  text: string;
};
type GrammarSummary = {
  id: string;
  pattern: string;
  level: string;
  reading: string;
  romaji: string;
  meaning: string;
  language: "zh-Hans" | "en";
  automatic: boolean;
};
type GrammarDetail = {
  entry: { formation: string; notes: string; lesson?: string; examples: { ja: string; translation: string }[] };
  summary: GrammarSummary;
  related: GrammarSummary[];
};

export const japaneseGrammarDialog = `
<dialog id="japanese-grammar-dialog" class="learning-dictionary-dialog japanese-grammar-dialog" aria-labelledby="japanese-grammar-title">
  <div class="dialog-heading">
    <h2 id="japanese-grammar-title">日语语法</h2>
    <button type="button" class="secondary" data-close-grammar>关闭</button>
  </div>
  <div class="japanese-grammar-search" data-grammar-search>
    <label>查句型 <input type="search" placeholder="例如 そうだ、たら、推断" data-grammar-query></label>
    <label>等级 <select data-grammar-level>
      <option value="">全部</option><option>N5</option><option>N4</option><option>N3</option>
      <option>N2</option><option>N1</option><option>基础概念</option>
    </select></label>
  </div>
  <div class="japanese-grammar-body" data-grammar-body></div>
  <footer class="learning-source-meta">
    JLPT 等级为社区参考。英文条目由 OpenJLPT contributors 编写。
    <button type="button" class="secondary" data-grammar-link="source">数据来源</button>
    <button type="button" class="secondary" data-grammar-link="license">CC BY-SA 4.0</button>
  </footer>
</dialog>`;

function grammarDetailMarkup({ entry, summary, related }: GrammarDetail): string {
  const source = summary.language === "en" ? "OpenJLPT 英文条目" : "Atogaki 中文讲解";
  return `<article class="japanese-grammar-entry">
    <h3 lang="ja">${escaped(summary.pattern)}</h3>
    <small>${escaped(summary.level)} · ${source}${summary.automatic ? " · 支持本地匹配" : ""}</small>
    <p lang="${summary.language}">${escaped(summary.meaning)}</p>
    <dl lang="${summary.language}"><dt>接续</dt><dd>${escaped(entry.formation)}</dd>
    ${entry.notes ? `<dt>用法</dt><dd>${escaped(entry.notes)}</dd>` : ""}</dl>
    ${entry.lesson ? `<p><button type="button" class="secondary" data-grammar-link="course">${escaped(entry.lesson)} · 课程大纲</button></p>` : ""}
    <h4>例句</h4>
    <ul class="japanese-grammar-examples">${entry.examples.map(example => `<li>
      <span lang="ja">${escaped(example.ja)}</span><span lang="${summary.language}">${escaped(example.translation)}</span>
    </li>`).join("")}</ul>
    ${related.length ? `<h4>相关条目</h4><div class="japanese-grammar-related">${related.map(entry =>
      `<button type="button" class="secondary" data-grammar-entry="${escaped(entry.id)}">${escaped(entry.pattern)}</button>`,
    ).join("")}</div>` : ""}
  </article>`;
}

export function mountJapaneseGrammar(
  dialog: HTMLDialogElement, preview: HTMLElement,
  save: (request: SaveGrammarMatch) => Promise<void>,
) {
  const body = dialog.querySelector<HTMLElement>("[data-grammar-body]")!;
  const controls = dialog.querySelector<HTMLElement>("[data-grammar-search]")!;
  const query = dialog.querySelector<HTMLInputElement>("[data-grammar-query]")!;
  const level = dialog.querySelector<HTMLSelectElement>("[data-grammar-level]")!;
  const close = dialog.querySelector<HTMLButtonElement>("[data-close-grammar]")!;
  const details = new Map<string, Promise<GrammarDetail>>();
  const history: string[] = [];
  let catalog: Promise<GrammarSummary[]> | undefined;
  let catalogRequest = 0;
  let previewRequest = 0;
  let detailRequest = 0;
  let returnFocus: HTMLElement | null = null;
  let busy = false;

  function loadEntry(id: string): Promise<GrammarDetail> {
    const cached = details.get(id);
    if (cached) return cached;
    const pending = invoke<GrammarDetail>("japanese_grammar_entry", { id }).catch(error => {
      details.delete(id);
      throw error;
    });
    details.set(id, pending);
    return pending;
  }

  async function showCatalog(): Promise<void> {
    const request = ++catalogRequest;
    controls.hidden = false;
    body.textContent = "正在读取语法目录…";
    try {
      catalog ??= invoke<GrammarSummary[]>("list_japanese_grammar").catch(error => {
        catalog = undefined;
        throw error;
      });
      const entries = await catalog;
      if (request !== catalogRequest || !dialog.open) return;
      const needle = query.value.replace(/[～〜~]/g, "").trim();
      const matches = entries.filter(entry =>
        (!level.value || entry.level === level.value) && matchesQuery(needle, [
          entry.pattern, entry.id, entry.meaning, entry.reading, entry.romaji,
        ]),
      );
      body.innerHTML = `<p role="status">${matches.length} 个条目</p><ul class="japanese-grammar-results">${matches.map(entry =>
        `<li><button type="button" class="secondary" data-grammar-entry="${escaped(entry.id)}">
          <strong lang="ja">${escaped(entry.pattern)}</strong><small>${escaped(entry.level)}${entry.language === "en" ? " · 英文" : ""}</small>
          <span>${escaped(entry.meaning)}</span>
        </button></li>`,
      ).join("")}</ul>`;
      body.scrollTop = 0;
    } catch (error) {
      if (request === catalogRequest) body.textContent = String(error);
    }
  }

  async function showEntry(id: string): Promise<void> {
    const request = ++catalogRequest;
    controls.hidden = true;
    body.textContent = "正在读取句型条目…";
    try {
      const detail = await loadEntry(id);
      if (request !== catalogRequest || !dialog.open) return;
      body.innerHTML = `<button type="button" class="secondary" data-grammar-back>${history.length > 1 ? "返回上个条目" : "返回目录"}</button>${grammarDetailMarkup(detail)}`;
      body.querySelector<HTMLButtonElement>("[data-grammar-back]")?.focus();
      body.scrollTop = 0;
    } catch (error) {
      if (request === catalogRequest) {
        body.textContent = String(error);
        const back = document.createElement("button");
        back.type = "button";
        back.textContent = "返回目录";
        back.dataset.grammarBack = "";
        body.prepend(back);
      }
    }
  }

  function open(id?: string): void {
    if (!dialog.open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      history.length = 0;
      dialog.showModal();
    }
    if (id) {
      history.push(id);
      void showEntry(id);
    } else {
      history.length = 0;
      void showCatalog();
      query.focus();
    }
  }

  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("cancel", event => { event.preventDefault(); dialog.close(); });
  dialog.addEventListener("close", () => {
    catalogRequest += 1;
    if (returnFocus?.isConnected) returnFocus.focus();
    returnFocus = null;
  });
  query.addEventListener("input", () => void showCatalog());
  level.addEventListener("change", () => void showCatalog());
  const resourceUrls = new Map([
    ["course", "https://higher.smartedu.cn/course/680182bcb836e5522a7be76d"],
    ["source", "https://github.com/evanclan/OpenJLPT/tree/0d1d3410bec90bd4098a7c72de820543cb4f707c/data/json/grammar"],
    ["license", "https://creativecommons.org/licenses/by-sa/4.0/"],
  ]);
  function openResource(event: Event): void {
    const button = (event.target as Element).closest<HTMLButtonElement>("[data-grammar-link]");
    const url = resourceUrls.get(button?.dataset.grammarLink ?? "");
    if (!url) return;
    void invoke("open_external_url", { url }).catch(error => {
      const host = dialog.open ? body : preview;
      const status = document.createElement("p");
      status.setAttribute("role", "status");
      status.textContent = String(error);
      host.append(status);
    });
  }
  dialog.addEventListener("click", openResource);
  preview.addEventListener("click", openResource);
  body.addEventListener("click", event => {
    const button = (event.target as Element).closest<HTMLButtonElement>("button");
    if (button?.dataset.grammarEntry) open(button.dataset.grammarEntry);
    if (button?.hasAttribute("data-grammar-back")) {
      history.pop();
      const previous = history.at(-1);
      if (previous) void showEntry(previous);
      else open();
    }
  });

  async function analyze(selection: GrammarSelection): Promise<void> {
    const request = ++previewRequest;
    detailRequest += 1;
    preview.classList.remove("hidden");
    preview.innerHTML = '<p role="status">正在本机分析句型…</p>';
    try {
      const result = await invoke<GrammarPreview>("analyze_listening_japanese_grammar", { request: selection });
      if (request !== previewRequest) return;
      const options = await Promise.all(result.matches.map(match => loadEntry(match.id)));
      if (request !== previewRequest) return;
      preview.innerHTML = `<div class="listening-word-options" aria-label="已匹配句型"></div>
        <div data-grammar-match-detail></div><button class="secondary" type="button" data-browse-grammar>浏览语法目录</button>`;
      const optionsHost = preview.querySelector<HTMLElement>(".listening-word-options")!;
      const matchHost = preview.querySelector<HTMLElement>("[data-grammar-match-detail]")!;
      const browse = preview.querySelector<HTMLButtonElement>("[data-browse-grammar]")!;
      browse.addEventListener("click", () => open());
      if (!result.matches.length) {
        matchHost.innerHTML = `<p lang="ja" class="listening-word-context">${rangeMarkup(result.text, result.focusStart, result.focusEnd)}</p>
          <p>选区暂未识别到有把握的句型。可以调整选区，或在目录查找。</p>`;
        return;
      }
      const buttons = options.map((option, index) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "secondary";
        const m = result.matches[index];
        button.textContent = `${option.summary.pattern} · ${result.text.slice(m.start, m.end)}`;
        button.addEventListener("click", () => choose(index));
        optionsHost.append(button);
        return button;
      });
      function choose(index: number): void {
        const match = result.matches[index];
        buttons.forEach((button, current) => button.setAttribute("aria-pressed", String(index === current)));
        matchHost.innerHTML = `<p lang="ja" class="listening-word-context">${rangeMarkup(result.text, match.start, match.end)}</p>
          ${match.formNote ? `<p>本句形式 · ${escaped(match.formNote)}</p>` : ""}
          <button type="button" data-save-grammar>收藏语法例句</button>
          <p role="status" data-grammar-save-status></p>
          ${grammarDetailMarkup(options[index])}`;
        const saveButton = matchHost.querySelector<HTMLButtonElement>("[data-save-grammar]")!;
        const key = ++detailRequest;
        saveButton.disabled = busy;
        saveButton.addEventListener("click", async () => {
          try {
            await save({ selection, text: result.text, id: match.id, start: match.start, end: match.end });
          } catch (error) {
            if (request === previewRequest && key === detailRequest) {
              matchHost.querySelector<HTMLElement>("[data-grammar-save-status]")!.textContent = String(error);
            }
          }
        });
        matchHost.querySelectorAll<HTMLButtonElement>("[data-grammar-entry]").forEach(button => {
          button.addEventListener("click", () => open(button.dataset.grammarEntry));
        });
      }
      choose(0);
    } catch (error) {
      if (request === previewRequest) {
        preview.innerHTML = '<p role="status"></p><button class="secondary" type="button">浏览语法目录</button>';
        preview.querySelector("p")!.textContent = String(error);
        preview.querySelector("button")!.addEventListener("click", () => open());
      }
    }
  }

  return {
    open,
    analyze,
    clear() {
      previewRequest += 1;
      detailRequest += 1;
      preview.classList.add("hidden");
      preview.replaceChildren();
    },
    setBusy(value: boolean) {
      busy = value;
      const action = preview.querySelector<HTMLButtonElement>("[data-save-grammar]");
      if (action) action.disabled = busy;
    },
  };
}

function rangeMarkup(text: string, start: number, end: number): string {
  return escaped(text.slice(0, start)) +
    `<mark>${escaped(text.slice(start, end))}</mark>` + escaped(text.slice(end));
}
