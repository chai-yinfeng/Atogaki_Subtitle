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
      assert.equal(await page.locator('#listening-view .loop-tools, #listening-view [data-loop], #listening-view .loop-state').count(), 0);
      assert.deepEqual(await page.locator('#listening-view .playback-tools button').allTextContents(), ['上一句', '−5 秒', '播放', '+5 秒', '下一句']);
      assert.equal(await page.locator('#listening-playback-rate').isVisible(), true);
      const measure = () => page.evaluate(() => {
        const list = document.querySelector('#listening-subtitle-list'), box = list.getBoundingClientRect();
        return { viewport: [innerWidth, innerHeight], pageHeight: document.scrollingElement.scrollHeight, pageWidth: document.scrollingElement.scrollWidth, listTop: box.top, listBottom: box.bottom, listHeight: list.clientHeight, rows: list.children.length, libraryHeight: document.querySelector('#listening-job-list').clientHeight };
      });
      const videoSize = () => page.locator('#listening-media-host video').evaluate(el => {
        const r=el.getBoundingClientRect(),caption=document.querySelector('.listening-playback .current-caption').getBoundingClientRect(),controls=document.querySelector('.listening-playback .playback-tools').getBoundingClientRect();
        return {height:r.height,width:r.width,fitHeight:Math.min(r.height,r.width*el.videoHeight/el.videoWidth),bottom:r.bottom,captionTop:caption.top,captionBottom:caption.bottom,controlsBottom:controls.bottom};
      });
      const layout = await measure(), initialVideo = await videoSize();
      assert.ok(initialVideo.height > viewport.height*.24, 'video must use available height beyond the old 24vh cap');
      assert.ok(initialVideo.captionTop >= initialVideo.bottom-1 && initialVideo.captionBottom <= viewport.height && initialVideo.controlsBottom <= viewport.height);
      assert.ok(layout.pageHeight <= viewport.height + 1, JSON.stringify(layout));
      assert.ok(layout.pageWidth <= viewport.width + 1, JSON.stringify(layout));
      assert.ok(layout.listHeight >= 300 && layout.listBottom <= viewport.height, JSON.stringify(layout));
      assert.equal(layout.rows, 149);
      assert.ok(await page.locator('.listening-text').first().evaluate(element => element.clientWidth >= 160), 'subtitle text must retain a readable column beside the timecode');
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
      const media = await page.locator('#listening-media-host video').evaluate(el => { el.dataset.layoutIdentity = 'retained'; return el.dataset.layoutIdentity; });
      const split = page.locator('.listening-splitter');
      await split.focus(); await page.keyboard.press('ArrowRight');
      assert.equal(await split.getAttribute('aria-valuenow'), '62');
      await page.locator('.listening-playback-info summary').click();
      await page.locator('#listening-layout-placement').selectOption('bottom');
      await page.waitForFunction(() => document.querySelector('.listening-session').dataset.placement === 'bottom');
      const bottom = await measure();
      assert.ok(bottom.listHeight >= 100 && bottom.listBottom <= viewport.height, JSON.stringify(bottom));
      const bottomVideo = await videoSize();
      assert.ok(bottomVideo.captionTop >= bottomVideo.bottom-1 && bottomVideo.controlsBottom <= viewport.height,'bottom transcript must keep caption below video and controls visible');
      await split.focus(); await page.keyboard.press('ArrowDown');
      assert.equal(await split.getAttribute('aria-valuenow'), '74');
      await page.locator('#listening-layout-placement').selectOption('hidden');
      assert.equal(await page.locator('.listening-reading').isVisible(), false);
      const hiddenVideo = await videoSize();
      assert.ok(hiddenVideo.fitHeight > initialVideo.fitHeight+5, JSON.stringify({initialVideo,hiddenVideo}));
      const videoHandle=page.locator('.listening-video-splitter'),handleBox=await videoHandle.boundingBox();
      await page.mouse.move(handleBox.x+handleBox.width/2,handleBox.y+handleBox.height/2);await page.mouse.down();
      await page.mouse.move(handleBox.x+handleBox.width/2,handleBox.y+handleBox.height/2-70,{steps:6});await page.mouse.up();
      const smaller=await videoSize();assert.ok(smaller.height < hiddenVideo.height-10);
      assert.ok(smaller.captionTop >= smaller.bottom-1 && smaller.controlsBottom<=viewport.height);
      const savedVideo=JSON.parse(await page.evaluate(()=>localStorage.getItem('atogaki.listeningLayout.v2'))).video;
      await videoHandle.focus();await page.keyboard.press('Home');
      assert.equal(await videoHandle.getAttribute('aria-valuenow'),'85');
      assert.ok(savedVideo<85);
      await page.locator('#listening-layout-placement').selectOption('right');
      assert.equal(await split.getAttribute('aria-valuenow'), '62');
      assert.equal(await page.locator('#listening-media-host video').getAttribute('data-layout-identity'), media);
      assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('atogaki.listeningLayout.v2'))), { version: 2, placement: 'right', right: 62, bottom: 74, video: 85 });
      if (process.env.ATOGAKI_SCREENSHOTS_DIR) {
        for(const placement of ['right','bottom','hidden']){
          await page.locator('#listening-layout-placement').selectOption(placement);
          await page.screenshot({path:path.join(process.env.ATOGAKI_SCREENSHOTS_DIR,`listening-${placement}-${viewport.width}.png`)});
        }
      }
      await page.reload();await page.locator('#show-listening').click();await page.locator('[data-listen-job]').first().click();
      assert.equal(await page.locator('#listening-layout-placement').inputValue(),process.env.ATOGAKI_SCREENSHOTS_DIR?'hidden':'right');
      assert.equal(await videoHandle.getAttribute('aria-valuenow'),'85');
      assert.deepEqual(errors, []);
      console.log(JSON.stringify(layout)); await page.close();
    }
    console.log('PASS: video grows when transcript is hidden; captions stay below video in every layout; video height drag, keyboard reset, persistence and unique player remain correct');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
