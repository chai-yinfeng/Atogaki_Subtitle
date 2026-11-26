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
      const text = "It has taken off.";
      const items = [];
      const match = { id: "en:perfect", title: "have + 过去分词", explanation: "把动作与另一个时间点联系起来。", start: 3, end: 12 };
      const save = (selection, start, end, sourceText, meaning, grammar) => {
        const detail = {
          item: { id: grammar ?? "plane", item_type: "selection", source_language: "en", source_text: sourceText,
            meaning_text: meaning, grammar_id: grammar, occurrence_count: 1 },
          lookup_results: [],
          occurrences: [{ id: "context", job_id: selection.jobId, segment_id: selection.segmentId,
            job_display_name_snapshot: "Test program", segment_source_snapshot: text, segment_translation_snapshot: "它已经起飞了。",
            start_ms: 3000, end_ms: 5800, selection_start_utf16: start, selection_end_utf16: end }],
        };
        items.push(detail);
        return detail;
      };
      window.__englishRequests = [];
      window.__TAURI_INTERNALS__.invoke = async (command, args = {}) => {
        if (command === "lookup_listening_english") {
          window.__englishRequests.push(args.request);
          const isRange = args.request.start !== args.request.end;
          return { words: [{
            surface: isRange ? "taken off" : "taken", lemma: isRange ? "take off" : "take", start: 7, end: isRange ? 16 : 12,
            reading: "", unknown: false, entries: [{ id: "entry", headword: "take off", readings: [], providerId: "ecdict",
              providerName: "ECDICT", version: "test", attribution: "", license: "",
              senses: [{ part_of_speech: "短语", definitions: ["起飞"], examples: [] }] }],
          }], notices: [] };
        }
        if (command === "save_listening_english_word") {
          window.__englishSave = args.request;
          return save(args.request, args.request.start, args.request.end, "take off", "起飞");
        }
        if (command === "analyze_listening_english_grammar") return [match];
        if (command === "save_listening_english_grammar") return save(args.request.selection, match.start, match.end, match.title, match.explanation, match.id);
        if (command === "list_learning_items") return items;
        if (command === "export_learning_anki") {
          window.__ankiRequest = args;
          return "/qa/Atogaki-en-cloze.tsv";
        }
        if (command === "get_playback_position") return 0;
        const result = await original(command, args);
        if (command === "list_job_summaries") return result.map(job => ({
          ...job, source_language: "en", translation_status: "translated", translated_segment_count: job.segment_count,
        }));
        if (command === "get_job_detail") {
          result.job.source_language = "en";
          result.segments[1].source_text = text;
        }
        return result;
      };
    });
    await page.goto(process.env.ATOGAKI_EDITOR_URL || "http://127.0.0.1:1430");
    await page.locator("#show-listening").click();
    await page.locator("#open-listening-library").click();
    await page.locator("[data-listen-job]").first().click();
    const row = page.locator('[data-segment-id="cue-1"]');
    const source = row.locator("[data-learning-source]");
    const point = await source.evaluate(element => {
      const range = document.createRange();
      range.setStart(element.firstChild, 7);
      range.setEnd(element.firstChild, 8);
      const rect = range.getBoundingClientRect();
      return [rect.left + rect.width / 2, rect.top + rect.height / 2];
    });
    await page.mouse.click(...point);
    const preview = page.locator("#listening-dictionary-preview");
    await preview.getByRole("button", { name: "taken · take", exact: true }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.__englishRequests[0]), {
      text: "It has taken off.", start: 7, end: 7,
    });
    await page.locator("#close-learning-selection").click();
    await source.evaluate(element => {
      const range = document.createRange();
      range.setStart(element.firstChild, 7);
      range.setEnd(element.firstChild, 16);
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
      element.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    });
    await preview.getByRole("button", { name: "taken off · take off", exact: true }).waitFor();
    await preview.getByRole("button", { name: "收藏义项 1" }).click();
    await preview.waitFor({ state: "hidden" });
    const saved = await page.evaluate(() => window.__englishSave);
    assert.deepEqual([saved.segmentId, saved.start, saved.end, saved.providerId, saved.senseIndex], ["cue-1", 7, 16, "ecdict", 0]);
    await row.getByRole("button", { name: "分析句型" }).click();
    const grammar = page.locator("#listening-grammar-preview");
    await grammar.getByText("have + 过去分词").waitFor();
    await grammar.getByRole("button", { name: "收藏语法例句" }).click();
    await grammar.waitFor({ state: "hidden" });

    await page.locator("#show-learning").click();
    await page.locator('[data-learning-language="en"]').click();
    const card = page.locator('[data-learning-item="plane"]');
    await card.getByRole("heading", { name: "take off" }).waitFor();
    await page.getByRole("button", { name: "复习卡片" }).click();
    assert.equal(await card.getByLabel("待填写").textContent(), "[…]");
    assert.equal(await card.getByText("起飞", { exact: false }).first().isVisible(), false);
    await card.getByText("显示答案").click();
    await card.getByText("它已经起飞了。").waitFor();
    await page.getByRole("button", { name: "导出 Anki 挖空卡" }).click();
    await page.waitForFunction(() => window.__ankiRequest?.cloze === true);
    assert.deepEqual(await page.evaluate(() => window.__ankiRequest), { language: "en", cloze: true });
    await card.getByRole("button", { name: "播放例句" }).click();
    await page.waitForFunction(() => document.querySelector("#listening-media-host audio")?.currentTime >= 3);
    assert.deepEqual(errors, []);
    console.log("PASS: English point lookup, phrase selection, grammar, cards, replay and Anki cloze export");
  } finally {
    await browser.close();
  }
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
