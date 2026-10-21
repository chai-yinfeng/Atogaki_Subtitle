const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
const fs=require('node:fs'), path=require('node:path');
(async()=>{
 const b=await webkit.launch({headless:true});
 const p=await b.newPage({viewport:{width:1180,height:760}}), errors=[];
 p.on('pageerror',e=>errors.push(String(e)));
 await require('./editor-ui-fixture.cjs').installEditorFixture(p,10000,20000);
 // Old settings preserve folds but reset geometry to the new layout.
 await p.addInitScript(()=>localStorage.setItem('atogaki.editorLayout.v2',JSON.stringify({version:2,video:true,waveform:true,topRatio:24,editorRatio:50,videoRatio:20})));
 await p.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1420');
 await p.locator('.job-open').first().click(); await p.locator('.editor-cue-row').first().waitFor();
 const output=process.env.ATOGAKI_EDITOR_QA_OUTPUT || '/tmp/atogaki-editor-compact';fs.mkdirSync(output,{recursive:true});
 const ratios={videoRatio:44,topRatio:67,waveRatio:34};
 for(const [field,value] of Object.entries(ratios)) assert.equal(await p.locator(`[data-editor-resize="${field}"]`).getAttribute('aria-valuenow'),String(value));
 const check=async()=>{
  const geometry=await p.evaluate(()=>{
   const box=s=>{const r=document.querySelector(s).getBoundingClientRect();return {x:r.x,y:r.y,w:r.width,h:r.height,b:r.bottom,r:r.right}};
   return {video:box('#editor-video-panel'),wave:box('#editor-wave-panel'),edit:box('.karaoke-current-segment'),source:box('#karaoke-current-source'),translation:box('#karaoke-current-translation'),list:box('.editor-list-panel'),height:document.documentElement.scrollHeight,bodyWidth:document.documentElement.scrollWidth,viewport:[innerWidth,innerHeight],rows:document.querySelectorAll('.editor-cue-row').length};
  });
  assert.ok(geometry.height<=geometry.viewport[1]);assert.ok(geometry.bodyWidth<=geometry.viewport[0]);
  assert.equal(geometry.video.y,geometry.wave.y);assert.ok(geometry.video.r<=geometry.wave.x);
  assert.equal(geometry.video.b,geometry.edit.b);assert.equal(geometry.wave.x,geometry.edit.x);
  assert.ok(geometry.source.b<geometry.translation.y);assert.ok(geometry.source.w>geometry.edit.w*.9);
  assert.ok(geometry.source.h>=36);assert.ok(geometry.edit.b<=geometry.list.y);
  assert.ok(geometry.list.h>=110);assert.ok(geometry.rows<=40);
  return geometry;
 };
 console.log('1180',await check());await p.screenshot({path:path.join(output,'compact-1180.png')});
 assert.equal(await p.locator('#karaoke-toggle-playback').isVisible(),true);
 const source=p.locator('#karaoke-current-source');await source.focus();
 await p.locator('.editor-menu summary').click(); await p.locator('#karaoke-insert-block').focus(); await p.keyboard.press('Escape');
 assert.equal(await p.locator('.editor-menu').evaluate(menu=>menu.open),false);
 assert.equal(await p.evaluate(()=>document.activeElement.tagName),'SUMMARY');
 await p.locator('.editor-menu summary').click(); await p.locator('#editor-search').click();
 assert.equal(await p.locator('.editor-menu').evaluate(menu=>menu.open),false);
 // Folded media keeps a usable transport and the same media element.
 await p.locator('#karaoke-media-host audio').evaluate(audio=>window.__sameAudio=audio);
 for(const panel of ['video','waveform']) await p.locator(`[data-editor-toggle="${panel}"]`).click();
 assert.equal(await p.locator('#karaoke-toggle-playback').isVisible(),true);
 assert.equal(await p.locator('#karaoke-media-host').evaluate(host=>host.inert),true);
 assert.equal(await p.locator('#karaoke-media-host audio').evaluate(audio=>audio===window.__sameAudio),true);
 assert.equal(await p.locator('#karaoke-current-source').isVisible(),true);
 for(const panel of ['video','waveform']) await p.locator(`[data-editor-toggle="${panel}"]`).click();
 // Pointer resizing and keyboard resizing use different parent extents.
 const wave=p.locator('[data-editor-resize="waveRatio"]');await wave.focus();await p.keyboard.press('ArrowDown');
 assert.equal(await wave.getAttribute('aria-valuenow'),'36');
 const bar=await p.locator('[data-editor-resize="videoRatio"]').boundingBox();
 await p.mouse.move(bar.x+bar.width/2,bar.y+30);await p.mouse.down();await p.mouse.move(bar.x+50,bar.y+30);await p.mouse.up();
 assert.ok(Number(await p.locator('[data-editor-resize="videoRatio"]').getAttribute('aria-valuenow'))>44);
 await p.locator('[data-editor-reset]').click();
 await p.setViewportSize({width:960,height:640});console.log('960',await check());await p.screenshot({path:path.join(output,'compact-960.png')});
 // A theme switch does not recolor editor chrome or leak the editor palette to listening.
 const palette=()=>p.locator('#karaoke-view').evaluate(e=>getComputedStyle(e).getPropertyValue('--surface'));
 const before=await palette(); await p.evaluate(()=>document.documentElement.dataset.theme='letter');assert.equal(await palette(),before);
 await p.addStyleTag({content:'#karaoke-view button,#karaoke-view summary,#karaoke-view input,#karaoke-view textarea {font-size:18px !important}'});
 assert.ok(await p.evaluate(()=>document.documentElement.scrollHeight<=innerHeight && document.documentElement.scrollWidth<=innerWidth));
 await p.screenshot({path:path.join(output,'compact-large-type.png')});
 await p.reload();await p.locator('.job-open').first().click();await p.locator('.editor-cue-row').first().waitFor();
 assert.equal(await wave.getAttribute('aria-valuenow'),'34');
 // Late playback reveals only viewport rows, then manual scrolling pauses scrolling alone.
 const seek=async time=>p.locator('#karaoke-media-host audio').evaluate(async(a,time)=>{
  if(a.readyState<1)await new Promise(r=>a.addEventListener('loadedmetadata',r,{once:true}));
  await new Promise(r=>{a.addEventListener('seeked',r,{once:true});a.currentTime=time});a.dispatchEvent(new Event('timeupdate'));
 },time);
 await seek(18);assert.ok(Number(await p.locator('#editor-cue-list').evaluate(e=>e.scrollTop))>200000);
 assert.equal(await p.locator('[data-cue-id="cue-9000"]').getAttribute('aria-current'),'true');
 await p.locator('#editor-cue-list').dispatchEvent('wheel',{deltaY:-200});
 await p.locator('#editor-cue-list').evaluate(e=>e.scrollTop=0);await seek(19);
 assert.equal(await p.locator('#editor-cue-list').evaluate(e=>e.scrollTop),0);
 assert.match(await source.inputValue(),/第 9501 句/);
 await p.locator('[data-editor-action="reveal-playing"]').click();
 assert.equal(await p.locator('[data-cue-id="cue-9500"]').getAttribute('aria-current'),'true');
 assert.ok(await p.locator('.editor-cue-row').count()<=40);
 await p.locator('[data-editor-action="back"]').click();
 assert.equal(await p.locator('.shell > header').isVisible(),true);
 assert.notEqual(await p.locator('.shell').evaluate(e=>getComputedStyle(e).getPropertyValue('--surface').trim()),'#fff');
 assert.deepEqual(errors,[]);console.log('PASS: compact geometry, migration, menus, fold transport, resizing, large type, palette isolation, 10k follow and scroll');
 await b.close();
})().catch(e=>{console.error(e);process.exit(1)});
