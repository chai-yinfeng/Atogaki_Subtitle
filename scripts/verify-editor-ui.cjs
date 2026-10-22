const { chromium, webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const output = process.env.ATOGAKI_EDITOR_QA_OUTPUT || '/tmp/atogaki-editor-ui';
fs.mkdirSync(output, {recursive:true});
(async()=>{
const useWebkit=process.env.ATOGAKI_EDITOR_ENGINE==='webkit';
const browser=await (useWebkit?webkit:chromium).launch({...(useWebkit?{}:process.env.ATOGAKI_CHROME ? {executablePath:process.env.ATOGAKI_CHROME} : {channel:'chrome'}),headless:true});
const context=await browser.newContext({viewport:{width:1180,height:760}});
const page=await context.newPage();
const errors=[];page.on('pageerror',e=>errors.push(String(e)));
await require('./editor-ui-fixture.cjs').installEditorFixture(page);
await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1420');await page.locator('.job-open').first().click();
await page.locator('#editor-cue-list .editor-cue-row').first().waitFor();
assert.equal(await page.evaluate(()=>window.__test.calls.filter(call=>call.cmd==='get_job_detail').length),1);
assert.equal(await page.locator('.editor-workspace-label').innerText(),'字幕编辑');
assert.equal(await page.locator('#workspace-view').count(),0);
assert.equal(await page.locator('#editor-repair-dialog').count(),0);

const panelOrder=await page.evaluate(()=>{
 const rect=selector=>{const r=document.querySelector(selector).getBoundingClientRect();return {x:r.x,y:r.y,bottom:r.bottom,right:r.right};};
 return {video:rect('#editor-video-panel'),wave:rect('#editor-wave-panel'),edit:rect('.karaoke-current-segment'),list:rect('.editor-list-panel')};
});
const cueSpace=await page.evaluate(()=>{const box=s=>document.querySelector(s).getBoundingClientRect();return {navTop:box('.editor-cue-navigation').top,timeTop:box('.editor-timing-fields').top,toolbarBottom:box('.editor-cue-toolbar').bottom,textTop:box('#karaoke-current-source').top,textHeight:box('#karaoke-current-source').height};});
assert.ok(cueSpace.navTop<=cueSpace.timeTop);
assert.ok(cueSpace.toolbarBottom<=cueSpace.textTop && cueSpace.textHeight>=60);
assert.equal(panelOrder.video.y,panelOrder.wave.y);
assert.ok(panelOrder.video.right<=panelOrder.wave.x);
assert.ok(panelOrder.wave.bottom<=panelOrder.edit.y && panelOrder.edit.bottom<=panelOrder.list.y);
assert.equal(panelOrder.video.bottom,panelOrder.edit.bottom);
assert.equal(panelOrder.edit.x,panelOrder.wave.x);
assert.ok(await page.evaluate(()=>{
 const a=document.querySelector('#karaoke-current-source').getBoundingClientRect(),b=document.querySelector('#karaoke-current-translation').getBoundingClientRect();
 return a.bottom<b.top && a.width>document.querySelector('.editor-right').clientWidth*.9 && a.x===b.x;
}));

await page.screenshot({path:path.join(output,'editor-1180.png')});
console.log('startup errors',errors);
console.log('layout',await page.evaluate(()=>({viewport:[innerWidth,innerHeight],height:document.documentElement.scrollHeight,panels:[...document.querySelectorAll('.editor-top,#editor-wave-panel,.editor-list-panel')].map(e=>({class:e.className,rect:{h:e.clientHeight,w:e.clientWidth}})),rows:document.querySelectorAll('.editor-cue-row').length})));
assert.equal(await page.locator('#karaoke-current-source').inputValue(), '第 1 句：今日は音楽と日常について話します。');
await page.locator('[data-cue-id="cue-0"]').focus();await page.keyboard.press('ArrowDown');
assert.match(await page.locator('[data-cue-id="cue-1"]').getAttribute('class'),/selected/);
await page.keyboard.press('ArrowUp');
assert.match(await page.locator('[data-cue-id="cue-0"]').getAttribute('class'),/selected/);
await page.locator('#karaoke-current-source').fill('第一段修改');
await page.locator('[data-cue-id="cue-1"]').click();
assert.match(await page.locator('[data-cue-id="cue-1"]').getAttribute('class'),/selected/);
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第 2 句：今日は音楽と日常について話します。');
await page.locator('#karaoke-current-source').fill('第二段修改');
await page.waitForTimeout(800);
assert.equal(await page.evaluate(()=>window.__test.cues[0].source_text),'第一段修改');
assert.equal(await page.evaluate(()=>window.__test.cues[1].source_text),'第二段修改');
await page.locator('#editor-search').fill('第一段修改');
assert.equal(await page.locator('.editor-cue-row').count(),1);
assert.match(await page.locator('#editor-selection-message').innerText(),/不在筛选/);
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第二段修改');
await page.locator('#editor-search').fill('');
await page.locator('#editor-filter').selectOption('review');assert.equal(await page.locator('.editor-cue-row').count(),1);
await page.locator('#editor-filter').selectOption('all');
await page.locator('[data-editor-toggle="video"]').click();assert.equal(await page.locator('#editor-video-panel').getAttribute('aria-hidden'),null);
await page.locator('[data-editor-toggle="waveform"]').click();assert.equal(await page.locator('#editor-wave-panel').isVisible(),false);
await page.locator('#karaoke-open-workspace').click();await page.locator('.editor-task-tools').waitFor();
assert.equal(await page.locator('.editor-task-tools #workspace-tab-review').isVisible(),false);
assert.equal(await page.locator('#back-to-jobs').innerText(),'← 返回字幕编辑');
await page.locator('#back-to-jobs').click();assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第二段修改');
assert.equal(await page.locator('#workspace-view').count(),0);
for(let i=0;i<2;i++) {
 await page.locator('#karaoke-open-workspace').click();
 await page.locator('#back-to-jobs').waitFor();await page.keyboard.press('Escape');
 assert.equal(await page.locator('#workspace-view').count(),0);
 assert.equal(await page.locator('#karaoke-view').evaluate(root=>root.inert),false);
}
await page.locator('[data-editor-action="repair"]').click();
await page.locator('#editor-repair-dialog[open]').waitFor();
assert.equal(await page.locator('.shell > header').evaluate(node=>node.inert),true);
await page.locator('[data-editor-action="close-repair"]').focus();
await page.keyboard.press('Shift+Tab');
assert.equal(await page.evaluate(()=>document.activeElement.hasAttribute('data-cancel-candidate')),true);
await page.keyboard.press('Tab');
assert.equal(await page.evaluate(()=>document.activeElement.dataset.editorAction),'close-repair');

await page.locator('#asr-quality-review').evaluate(details=>details.open=true);
await page.locator('.asr-quality-signal').waitFor();
await page.locator('[data-editor-action="close-repair"]').click();
assert.equal(await page.locator('#editor-repair-dialog').count(),0);
assert.equal(await page.locator('.shell > header').evaluate(node=>node.inert),false);
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第二段修改');

await page.locator('[data-editor-reset]').click();
await page.setViewportSize({width:960,height:640});await page.screenshot({path:path.join(output,'editor-960.png')});
assert.ok(await page.evaluate(()=>document.documentElement.scrollHeight<=innerHeight));
console.log('small layout',await page.evaluate(()=>({height:document.documentElement.scrollHeight,view:document.querySelector('#karaoke-view').getBoundingClientRect().toJSON(),listHeight:document.querySelector('#editor-cue-list').clientHeight})));
// Playback must never replace the selected editor, and folding cannot remount media.
await page.locator('#karaoke-media-host audio').evaluate(async audio=>{ if(audio.readyState<1)await new Promise(resolve=>audio.addEventListener('loadedmetadata',resolve,{once:true}));await new Promise(resolve=>{audio.addEventListener('seeked',resolve,{once:true});audio.currentTime=7;}); audio.dispatchEvent(new Event('timeupdate')); });
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第二段修改');
console.log('playback', await page.evaluate(()=>({time:document.querySelector('#karaoke-media-host audio').currentTime,clock:document.querySelector('#karaoke-current-time').textContent,playing:[...document.querySelectorAll('.playing')].map(e=>e.dataset.cueId),errors:document.querySelector('#karaoke-timing-message').textContent})));
assert.match(await page.locator('[data-cue-id="cue-2"]').getAttribute('class'),/playing/);
const beforePosition=await page.locator('#karaoke-media-host audio').evaluate(audio=>{window.__qaMedia=audio; return audio.currentTime;});
await page.locator('[data-editor-toggle="video"]').click();
assert.ok(await page.locator('#karaoke-media-host audio').evaluate(audio=>audio===window.__qaMedia));
assert.equal(await page.locator('#karaoke-media-host audio').evaluate(audio=>audio.currentTime),beforePosition);
await page.locator('[data-editor-toggle="video"]').click();
await page.locator('[data-editor-resize="videoRatio"]').focus();await page.keyboard.press('ArrowRight');
assert.equal(await page.locator('[data-editor-resize="videoRatio"]').getAttribute('aria-valuenow'),'46');
// Explicit time save preserves typed values while playback advances.
await page.locator('#editor-cue-start').fill('00:00:03.100');
await page.locator('#editor-cue-end').fill('00:00:05.700');
await page.locator('[data-editor-action="save-time"]').click();
assert.equal(await page.evaluate(()=>window.__test.cues[1].start_ms),3100);
// Large text remains contained; panels provide independent scrolling.
await page.evaluate(()=>document.querySelector('#karaoke-view').style.fontSize='20px');
assert.ok(await page.evaluate(()=>document.documentElement.scrollHeight<=innerHeight));
// IME composition keeps its cue selected until compositionend.
await page.locator('#karaoke-current-source').evaluate(input=>{input.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));input.value='に';input.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true}));});
await page.locator('[data-cue-id="cue-2"]').click();
assert.match(await page.locator('[data-cue-id="cue-1"]').getAttribute('class'),/selected/);
await page.locator('#karaoke-current-source').evaluate(input=>{input.value='日本語確定';input.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}));});
await page.waitForTimeout(750);
assert.equal(await page.evaluate(()=>window.__test.cues[1].source_text),'日本語確定');
await page.evaluate(()=>window.__test.fail=true);await page.locator('#karaoke-current-source').fill('失败草稿');await page.waitForTimeout(800);
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'失败草稿');assert.match(await page.locator('#karaoke-timing-message').innerText(),/disk full/);
await page.locator('[data-cue-id="cue-2"]').click();await page.locator('[data-cue-id="cue-1"]').click();assert.equal(await page.locator('#karaoke-current-source').inputValue(),'失败草稿');
await page.evaluate(()=>window.__test.fail=false);await page.locator('#karaoke-save-text').click();await page.waitForTimeout(100);
assert.equal(await page.evaluate(()=>window.__test.cues[1].source_text),'失败草稿');
// Scrolling and recycling rows must keep offscreen drafts and keyboard navigation.
assert.ok(await page.locator('.editor-cue-row').count() <= 80);
await page.locator('#editor-cue-list').focus();await page.keyboard.press('End');
assert.match(await page.locator('[data-cue-id="cue-299"]').getAttribute('class'),/selected/);
await page.locator('#karaoke-current-source').fill('最后一段离屏保存');
await page.locator('#editor-cue-list').focus();await page.keyboard.press('Home');
await page.waitForTimeout(750);
await page.locator('#editor-cue-list').focus();await page.keyboard.press('End');
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'最后一段离屏保存');
assert.ok(await page.locator('.editor-cue-row').count() <= 80);
assert.equal(await page.evaluate(()=>window.__test.calls.filter(call=>call.cmd==='list_jobs').length),0);
assert.equal(await page.evaluate(()=>window.__test.calls.filter(call=>call.cmd==='list_job_summaries').length),1);
// Reload validates cross-session layout preferences and restores the same panel sizes.
await page.reload();await page.locator('.job-open').first().click();
await page.locator('#editor-cue-list .editor-cue-row').first().waitFor();
assert.equal(await page.locator('[data-editor-resize="videoRatio"]').getAttribute('aria-valuenow'),'46');
assert.equal(await page.locator('#subtitle-list textarea').count(),0);
// The same region still mounts as a read-only progress page for active jobs.
await page.evaluate(()=>{window.__test.job.status='transcribing';window.__test.job.segment_count=0;});
await page.locator('[data-editor-action="back"]').click();
await page.locator('.job-open').first().click();await page.locator('#workspace-view').waitFor();
assert.match(await page.locator('#workspace-title').innerText(),/任务进度/);
assert.equal(await page.locator('#subtitle-list textarea').count(),0);
await page.locator('#back-to-jobs').click();
assert.equal(await page.locator('#workspace-view').count(),0);
assert.deepEqual(errors,[]);console.log('PASS: selection, independent drafts, filters, tool return, folding, playback, time save, IME, failure retry, layout persistence');
await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
