const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
(async () => {
 const browser = await webkit.launch({headless:true});
 const page = await browser.newPage({viewport:{width:1180,height:760}});
 const errors=[]; page.on('pageerror',e=>errors.push(String(e)));
 await require('./editor-ui-fixture.cjs').installEditorFixture(page);
 await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1420');
 await page.locator('.job-open').first().click();
 const source=page.locator('#karaoke-current-source'), follow=page.locator('[data-editor-action="follow"]');
 const seek=async time=>page.locator('#karaoke-media-host audio').evaluate(async (audio,time)=>{
  if(audio.readyState<1) await new Promise(resolve=>audio.addEventListener('loadedmetadata',resolve,{once:true}));
  if(audio.currentTime!==time) await new Promise(resolve=>{audio.addEventListener('seeked',resolve,{once:true});audio.currentTime=time;});
  audio.dispatchEvent(new Event('timeupdate'));
 },time);
 await source.waitFor(); await seek(7);
 assert.match(await source.inputValue(),/第 3 句/);
 assert.equal(await page.locator('[data-cue-id="cue-2"]').getAttribute('aria-current'),'true');
 assert.equal(await follow.getAttribute('aria-pressed'),'true');
 await page.locator('#editor-cue-list').dispatchEvent('wheel',{deltaY:100});
 await seek(10); assert.match(await source.inputValue(),/第 4 句/);
 assert.equal(await follow.getAttribute('aria-pressed'),'true');
 assert.equal(await page.locator('[data-editor-action="reveal-playing"]').isVisible(),true);
 await page.locator('[data-cue-id="cue-1"]').click(); await seek(13);
 assert.match(await source.inputValue(),/第 2 句/);
 assert.equal(await follow.getAttribute('aria-pressed'),'false');
 assert.equal(await page.locator('#karaoke-cut-segment').isEnabled(),false);
 await follow.click(); assert.match(await source.inputValue(),/第 5 句/);
 assert.equal(await page.locator('#karaoke-cut-segment').isEnabled(),true);
 // Focus alone pins the editor, even before the first key or IME event.
 await source.focus(); await seek(16); assert.match(await source.inputValue(),/第 5 句/);
 await source.fill('正在保存的草稿');
 await page.evaluate(()=>window.__test.delay=350);
 await follow.click(); await source.focus(); await source.fill('保存期间继续输入');
 await page.waitForTimeout(800);
 assert.equal(await follow.getAttribute('aria-pressed'),'false');
 assert.equal(await source.inputValue(),'保存期间继续输入');
 await page.evaluate(()=>window.__test.delay=0);
 await follow.click(); assert.match(await source.inputValue(),/第 6 句/);
 assert.equal(await page.evaluate(()=>window.__test.cues[4].source_text),'保存期间继续输入');
 // Silence gaps clear follow display but must not destroy a pinned editor.
 await seek(17.9); assert.equal(await source.inputValue(),''); assert.equal(await source.isDisabled(),true);
 await seek(16); await source.focus(); await seek(17.9); assert.match(await source.inputValue(),/第 6 句/);
 // Unsaved timing must survive both following and row selection.
 await page.locator('#editor-cue-start').fill('00:00:15.500');
 await follow.click(); assert.equal(await follow.getAttribute('aria-pressed'),'false');
 await page.locator('[data-cue-id="cue-1"]').click(); assert.match(await source.inputValue(),/第 6 句/);
 assert.equal(await page.locator('#editor-cue-start').inputValue(),'00:00:15.500');
 await page.locator('#editor-cue-start').focus(); await page.keyboard.press('Escape');
 assert.equal(await page.locator('#editor-cue-start').inputValue(),'00:00:15.000');
 // A composition with unchanged text still blocks resume.
 await source.evaluate(input=>input.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true})));
 await follow.click(); assert.equal(await follow.getAttribute('aria-pressed'),'false');
 await source.evaluate(input=>input.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true})));
 await seek(16); await source.fill('保留失败草稿'); await page.evaluate(()=>window.__test.fail=true);
 await follow.click(); await page.waitForTimeout(100);
 assert.equal(await follow.getAttribute('aria-pressed'),'false'); assert.equal(await source.inputValue(),'保留失败草稿');
 await page.evaluate(()=>window.__test.fail=false); await follow.click();
 assert.equal(await follow.getAttribute('aria-pressed'),'true');
 await seek(7); await page.locator('#editor-search').fill('第 1 句');
 assert.match(await source.inputValue(),/第 3 句/);
 assert.match(await page.locator('#editor-selection-message').innerText(),/不在筛选/);
 assert.equal(await page.locator('#editor-search').inputValue(),'第 1 句');
 assert.deepEqual(errors,[]);
 console.log('PASS: follow, lock, independent scroll, gaps, focus, IME, in-flight typing, failed save, filtered playback, Cut target');
 await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
