const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || "playwright");
const assert = require("node:assert/strict");
const { installEditorFixture } = require("./editor-ui-fixture.cjs");

(async () => {
  const browser = await webkit.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1180, height: 760 } });
    const errors = [];
    page.on("pageerror", error => errors.push(String(error)));
    await installEditorFixture(page, 3);
    await page.addInitScript(() => {
      const original = window.__TAURI_INTERNALS__.invoke;
      const start = "まだ食べて";
      const end = "いなかった。";
      const text = start + end;
      const pattern = { id: "te-iru", pattern: "～ている", level: "N5", meaning: "表示动作或状态持续。", language: "zh-Hans", automatic: true };
      const concept = { id: "te-form", pattern: "て形", level: "基础概念", meaning: "连接动作。", language: "zh-Hans", automatic: false };
      const advanced = { id: "ga-hayai-ka", pattern: "〜が早いか", level: "N1", romaji: "ga hayai ka", meaning: "刚……就……", language: "zh-Hans", translatedFromOpenjlpt: true, automatic: false };
      const entries = { [pattern.id]: pattern, [concept.id]: concept, [advanced.id]: advanced };
      const match = { id: pattern.id, start: 2, end: 10, formNote: "过去否定", usageNote: "本句用て形接「いる」。" };
      let items = [];
      window.__grammarRequests = [];
      window.__grammarFeedback = [];
      window.__TAURI_INTERNALS__.invoke = async (command, args = {}) => {
        if (command === "list_japanese_grammar") return Object.values(entries);
        if (command === "japanese_grammar_entry") return {
          summary: entries[args.id], related: args.id === pattern.id ? [concept] : [],
          entry: { formation: "动词て形＋いる", notes: "例句说明", examples: [{ ja: "待っている。", translation: "正在等。" }] },
        };
        if (command === "lookup_listening_japanese") return { notices: [], words: [] };
        if (command === "analyze_listening_japanese_grammar") {
          window.__grammarRequests.push(args.request);
          return args.request.segmentId === "cue-0"
            ? { text: "今日はお休みです。", focusStart: 0, focusEnd: 9, matches: [] }
            : { text, focusStart: 0, focusEnd: start.length, matches: [match] };
        }
        if (command === "save_listening_japanese_grammar") {
          window.__collectedGrammar = args.request;
          const item = { id: "grammar-1", grammar_id: pattern.id, item_type: "selection", source_language: "ja",
            source_text: pattern.pattern, meaning_text: pattern.meaning, occurrence_count: 1 };
          items.push({ item, lookup_results: [], occurrences: [{ id: "source", job_id: args.request.selection.jobId,
            segment_id: "cue-1", job_display_name_snapshot: "测试节目", segment_source_snapshot: text,
            selection_start_utf16: match.start, selection_end_utf16: match.end, start_ms: 3000, end_ms: 9000 }] });
          return items.at(-1);
        }
        if (command === "save_listening_japanese_grammar_feedback") {
          const request = args.request;
          window.__grammarFeedback.push(request);
          const { reportedMatch, selection } = request;
          const kind = reportedMatch ? "wrong" : "missed";
          const { start, end } = reportedMatch ?? selection;
          items.push({
            item: { id: kind, grammar_id: reportedMatch?.id, grammar_feedback: kind, item_type: "selection",
              source_language: "ja", source_text: request.text.slice(start, end), occurrence_count: 1 },
            lookup_results: [], occurrences: [{ id: kind, job_id: selection.jobId, segment_id: selection.segmentId,
              job_display_name_snapshot: "测试节目", segment_source_snapshot: request.text,
              selection_start_utf16: start, selection_end_utf16: end, start_ms: reportedMatch ? 3000 : 0, end_ms: 9000 }],
          });
          return items.at(-1);
        }
        if (command === "list_learning_items") return items;
        if (command === "get_playback_position") return 0;
        const result = await original(command, args);
        if (command === "list_job_summaries") return result.map(job => ({
          ...job, translation_status: "translated", translated_segment_count: job.segment_count,
        }));
        if (command === "get_job_detail") {
          result.segments[0].source_text = "今日はお休みです。";
          result.segments[1].source_text = start;
          result.segments[2].source_text = end;
        }
        return result;
      };
    });
    await page.goto(process.env.ATOGAKI_EDITOR_URL || "http://127.0.0.1:1430");
    await page.locator("#show-listening").click();
    await page.locator("#open-listening-library").click();
    await page.locator("[data-listen-job]").first().click();

    const preview = page.locator("#listening-grammar-preview");
    const catalog = page.locator("#japanese-grammar-dialog");
    await page.locator('[data-cue-grammar="cue-0"]').click();
    await preview.getByText(/暂未识别到/).waitFor();
    await preview.getByRole("button", { name: "标记漏报" }).click();
    await preview.getByText("已标记，可在学习区回到原句。").waitFor();
    assert.equal(await page.evaluate(() => window.__grammarFeedback[0].reportedMatch), undefined);
    await preview.getByRole("button", { name: "浏览语法目录" }).click();
    await catalog.locator("[data-grammar-level]").selectOption("N1");
    await catalog.getByText("〜が早いか").waitFor();
    assert.equal(await catalog.getByText("～ている").count(), 0);
    await catalog.getByRole("button", { name: /が早いか/ }).click();
    await catalog.getByText("OpenJLPT 中文译文", { exact: false }).waitFor();
    await catalog.getByRole("button", { name: "返回目录" }).click();
    await catalog.locator("[data-grammar-level]").selectOption("基础概念");
    await catalog.getByRole("button", { name: /て形/ }).click();
    await catalog.getByText("基础概念 · Atogaki 中文讲解", { exact: true }).waitFor();
    await catalog.getByRole("button", { name: "关闭" }).click();
    await page.locator("#close-learning-selection").click();

    const source = page.locator('[data-segment-id="cue-1"] [data-learning-source]');
    await source.evaluate(element => {
      const range = document.createRange();
      range.setStart(element.firstChild, 1);
      range.setEnd(element.firstChild, 4);
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
      element.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    });
    await page.locator("#analyze-listening-grammar").click();
    await preview.getByText(/过去否定/).waitFor();
    await preview.getByText("本句用て形接「いる」。", { exact: false }).waitFor();
    assert.equal(await preview.locator("mark").textContent(), "食べていなかった");
    assert.deepEqual(await page.evaluate(() => window.__grammarRequests.at(-1)), {
      jobId: "editor-test", segmentId: "cue-1", sourceText: "まだ食べて", start: 1, end: 4,
    });
    await page.locator("#close-learning-selection").click();
    const endSource = page.locator('[data-segment-id="cue-2"] [data-learning-source]');
    await endSource.scrollIntoViewIfNeeded();
    const pointAt = (element, offset) => {
      const range = document.createRange();
      range.setStart(element.firstChild, offset);
      range.setEnd(element.firstChild, offset + 1);
      const rect = range.getBoundingClientRect();
      return { x: rect.left + rect.width * 0.15, y: rect.top + rect.height / 2 };
    };
    const firstPoint = await source.evaluate(pointAt, 2);
    const lastPoint = await endSource.evaluate(pointAt, 4);
    // Drag backwards to exercise the browser's normalization of range endpoints.
    await page.mouse.move(lastPoint.x, lastPoint.y);
    await page.mouse.down();
    await page.mouse.move(firstPoint.x, firstPoint.y, { steps: 15 });
    await page.mouse.up();
    assert.equal(await page.locator("#listening-selection-text").textContent(), "食べていなかっ");
    assert.equal(await page.locator("#save-learning-selection").isVisible(), false);
    await page.locator("#analyze-listening-grammar").click();
    await preview.getByText(/过去否定/).waitFor();
    assert.deepEqual(await page.evaluate(() => window.__grammarRequests.at(-1)), {
      jobId: "editor-test", segmentId: "cue-1", sourceText: "まだ食べて", start: 2, end: 4,
      endCue: { segmentId: "cue-2", sourceText: "いなかった。" },
    });
    await preview.getByRole("button", { name: "て形", exact: true }).click();
    await catalog.getByRole("heading", { name: "て形" }).waitFor();
    await catalog.getByRole("button", { name: "关闭" }).click();
    await preview.getByRole("button", { name: "标记误报" }).click();
    await preview.getByText("已标记，可在学习区回到原句。").waitFor();
    assert.deepEqual(await page.evaluate(() => window.__grammarFeedback[1].reportedMatch), { id: "te-iru", start: 2, end: 10 });
    await preview.getByRole("button", { name: "收藏语法例句" }).click();
    await preview.waitFor({ state: "hidden" });
    const collected = await page.evaluate(() => window.__collectedGrammar);
    assert.equal(collected.id, "te-iru");
    assert.equal(collected.selection.endCue.segmentId, "cue-2");

    await page.locator("#show-learning").click();
    await page.locator('[data-learning-item="missed"]').getByText(/疑似漏报/).waitFor();
    const feedback = page.locator('[data-learning-item="wrong"]');
    await feedback.getByText(/疑似误报/).waitFor();
    assert.equal(await feedback.locator("blockquote mark").first().textContent(), "食べていなかった");
    assert.equal(await feedback.getByRole("button", { name: "保存译义" }).count(), 0);
    const card = page.locator('[data-learning-item="grammar-1"]');
    await card.getByRole("heading", { name: "～ている" }).waitFor();
    assert.equal(await card.locator("blockquote mark").first().textContent(), "食べていなかった");
    await card.getByRole("button", { name: "查看句型" }).click();
    await catalog.getByRole("heading", { name: "～ている" }).waitFor();
    await catalog.getByText(/本句用法.*表示动作或状态持续/).waitFor();
    assert.equal(await catalog.locator("mark").textContent(), "食べていなかった");
    await catalog.getByRole("button", { name: "关闭" }).click();
    await feedback.getByRole("button", { name: "播放例句" }).click();
    await page.waitForFunction(() => document.querySelector("#listening-media-host audio")?.currentTime >= 3);
    assert.deepEqual(errors, []);
    console.log("Japanese grammar: selection, non-match, level filter, cross-cue highlight, related entry, save and replay passed");
  } finally {
    await browser.close();
  }
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
