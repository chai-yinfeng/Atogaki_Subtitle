const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
(async () => {
  const video = await fs.readFile(process.env.ATOGAKI_TEST_VIDEO);
  const browser = await webkit.launch({ headless: true });
  try {
    for (const viewport of [{ width: 1180, height: 732 }, { width: 960, height: 612 }]) {
      const page = await browser.newPage({ viewport }), errors = [];
      page.on('pageerror', error => errors.push(String(error)));
      await require('./editor-ui-fixture.cjs').installEditorFixture(page, 149);
      await page.route('**/fixture-video.mp4', route => route.fulfill({ body: video, contentType: 'video/mp4' }));
      await page.addInitScript(() => {
        const invoke = window.__TAURI_INTERNALS__.invoke;
        window.__TAURI_INTERNALS__.invoke = async (command, args) => {
          const value = await invoke(command, args);
          if (command === 'get_playback_position') return 0;
          if (command === 'save_playback_position') return null;
          if (command === 'list_job_summaries') return Array.from({ length: 13 }, (_, index) => ({ ...value[0], job_id: index ? `library-${index}` : value[0].job_id, display_name: `收听节目 ${index + 1} · 长标题与全文阅读`, translation_status: 'translated', translated_segment_count: 149 }));
          if (command === 'get_job_detail') return { ...value, job: { ...value.job, input_path: '/fixture-video.mp4' }, playback_path: '/fixture-video.mp4' };
          return value;
        };
      });
      await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1430');
      await page.locator('#show-listening').click();
      await page.locator('[data-listen-job]').first().click();
      await page.waitForFunction(() => document.querySelector('#listening-media-host video')?.readyState >= 1);
      const measure = () => page.evaluate(() => {
        const list = document.querySelector('#listening-subtitle-list'), box = list.getBoundingClientRect();
        return { viewport: [innerWidth, innerHeight], pageHeight: document.scrollingElement.scrollHeight, pageWidth: document.scrollingElement.scrollWidth, listTop: box.top, listBottom: box.bottom, listHeight: list.clientHeight, rows: list.children.length, libraryHeight: document.querySelector('#listening-job-list').clientHeight };
      });
      const layout = await measure();
      assert.ok(layout.pageHeight <= viewport.height + 1, JSON.stringify(layout));
      assert.ok(layout.pageWidth <= viewport.width + 1, JSON.stringify(layout));
      assert.ok(layout.listHeight >= 300 && layout.listBottom <= viewport.height, JSON.stringify(layout));
      assert.equal(layout.rows, 149);
      assert.ok(await page.locator('.listening-text').first().evaluate(element => element.clientWidth >= 220), 'subtitle text must retain a readable column beside the timecode');
      const list = page.locator('#listening-subtitle-list');
      await list.hover(); await page.mouse.wheel(0, 400);
      await page.waitForFunction(() => document.querySelector('#listening-subtitle-list').scrollTop > 0);
      assert.equal(await page.evaluate(() => document.scrollingElement.scrollTop), 0);
      await page.locator('.listening-playback-info summary').click();
      assert.equal((await measure()).listHeight, layout.listHeight, 'opening playback details must not reduce subtitle reading space');
      if (process.env.ATOGAKI_SCREENSHOTS_DIR) {
        await fs.mkdir(process.env.ATOGAKI_SCREENSHOTS_DIR, { recursive: true });
        await page.screenshot({ path: path.join(process.env.ATOGAKI_SCREENSHOTS_DIR, `listening-${viewport.width}.png`) });
      }
      assert.deepEqual(errors, []);
      console.log(JSON.stringify(layout)); await page.close();
    }
    console.log('PASS: video, controls and independently scrolling subtitles fit default and minimum desktop windows');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
