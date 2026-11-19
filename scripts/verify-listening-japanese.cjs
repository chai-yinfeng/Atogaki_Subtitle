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
      const text = "昨日は本を読んでいたけど";
      const entry = (providerId, providerName, definition) => ({
        id: "1456360", headword: "読む", readings: ["よむ"], providerId, providerName,
        senses: [{ part_of_speech: "动词", definitions: [definition], examples: [] }],
        version: "test", attribution: providerName, license: "CC BY-SA 4.0",
      });
      let items = [];
      window.__japaneseRequests = [];
      window.__TAURI_INTERNALS__.invoke = async (command, args = {}) => {
        if (command === "lookup_listening_japanese") {
          window.__japaneseRequests.push(args.request);
          return { words: [
            { start: 5, end: 8, surface: "読んで", lemma: "読む", reading: "よむ", unknown: false,
              entries: [entry("tomoshi", "Tomoshi", "阅读"), entry("jmdict", "JMdict", "to read")] },
          ], notices: [] };
        }
        if (command === "save_listening_japanese_word") {
          window.__savedJapanese = args.request;
          const { jobId, segmentId, start, end } = args.request;
          const item = { id: "reading", item_type: "selection", source_language: "ja", source_text: "読む",
            occurrence_count: 1, meaning_text: "阅读", meaning_source_label: "Tomoshi" };
          items = [{ item, lookup_results: [], occurrences: [{ id: "source", job_id: jobId, segment_id: segmentId,
            job_display_name_snapshot: "测试电台", segment_source_snapshot: text, segment_translation_snapshot: "昨天读了书",
            selection_start_utf16: start, selection_end_utf16: end, start_ms: 3000, end_ms: 5800 }] }];
          return items[0];
        }
        if (command === "list_learning_items") return items;
        if (command === "get_playback_position") return 0;
        const result = await original(command, args);
        if (command === "list_job_summaries") return result.map(job => ({
          ...job, translation_status: "translated", translated_segment_count: job.segment_count,
        }));
        if (command === "get_job_detail") result.segments[1].source_text = text;
        return result;
      };
    });
    await page.goto(process.env.ATOGAKI_EDITOR_URL || "http://127.0.0.1:1430");
    await page.locator("#show-listening").click();
    await page.locator("#open-listening-library").click();
    await page.locator("[data-listen-job]").first().click();

    const source = page.locator('[data-segment-id="cue-1"] [data-learning-source]');
    const point = await source.evaluate(element => {
      const range = document.createRange();
      range.setStart(element.firstChild, 5);
      range.setEnd(element.firstChild, 6);
      const rect = range.getBoundingClientRect();
      return { x: rect.left + rect.width * 0.8, y: rect.top + rect.height / 2 };
    });
    await page.mouse.click(point.x, point.y);
    const preview = page.locator("#listening-dictionary-preview");
    await preview.getByText("阅读").waitFor();
    assert.deepEqual(await page.evaluate(() => window.__japaneseRequests[0]), {
      text: "昨日は本を読んでいたけど", start: 5, end: 5,
    });
    assert.equal(await preview.locator("mark").textContent(), "読んで");
    assert.equal(await page.locator("#save-learning-selection").isVisible(), false);
    await preview.locator("select").selectOption("1");
    await preview.getByText("to read").waitFor();
    await page.locator("#close-learning-selection").click();

    await source.evaluate(element => {
      const range = document.createRange();
      range.setStart(element.firstChild, 4);
      range.setEnd(element.firstChild, 9);
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
      element.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    });
    await preview.getByText("阅读").waitFor();
    assert.deepEqual(await page.evaluate(() => window.__japaneseRequests.at(-1)), {
      text: "昨日は本を読んでいたけど", start: 4, end: 9,
    });
    assert.equal(await page.locator("#save-learning-selection").textContent(), "收藏原选区");
    await preview.getByRole("button", { name: "收藏义项 1" }).click();
    await preview.waitFor({ state: "hidden" });
    const saved = await page.evaluate(() => window.__savedJapanese);
    assert.deepEqual([saved.segmentId, saved.start, saved.end, saved.providerId, saved.senseIndex], ["cue-1", 5, 8, "tomoshi", 0]);

    await page.locator("#show-learning").click();
    const card = page.locator('[data-learning-item="reading"]');
    await card.getByRole("heading", { name: "読む" }).waitFor();
    assert.equal(await card.locator(".learning-meaning").inputValue(), "阅读");
    await card.getByRole("button", { name: "播放例句" }).click();
    await page.waitForFunction(() => document.querySelector("#listening-media-host audio")?.currentTime >= 3);
    assert.deepEqual(errors, []);
    console.log("Japanese listening: WebKit click, imprecise selection, source switch, save and replay passed");
  } finally {
    await browser.close();
  }
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
