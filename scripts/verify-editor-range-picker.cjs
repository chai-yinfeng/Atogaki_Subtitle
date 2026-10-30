const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE), assert = require('node:assert/strict');
(async () => {
 const browser = await webkit.launch({ headless: true });
 try {
  for (const size of [{width:1180,height:732},{width:960,height:612}]) {
   const page = await browser.newPage({viewport:size}), errors=[];
   page.on('pageerror', error=>errors.push(String(error)));
   await require('./editor-ui-fixture.cjs').installEditorFixture(page);
   await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1438');
   await page.locator('.job-open').first().click();
   await page.locator('.editor-cue-row').first().click();
   const source = await page.locator('#karaoke-current-source').inputValue();
   await page.locator('#karaoke-waveform').evaluate(el=>el.dataset.identity='shared');
   const zoomBefore=await page.locator('#karaoke-zoom').inputValue();
   await page.locator('[data-editor-action="repair"]').click();
   await page.waitForFunction(()=>document.querySelector('#repair-wave-status').textContent.includes('同步'));
   assert.equal(await page.locator('#editor-repair-dialog #karaoke-waveform').count(),1);
   assert.equal(await page.locator('#karaoke-waveform').count(),1);
   assert.equal(await page.locator('#karaoke-zoom').inputValue(),zoomBefore);
   // Each graphical edge changes only its own timestamp.
   const start=page.locator('#retranscription-start'),end=page.locator('#retranscription-end');
   const canvas=page.locator('#karaoke-waveform'),box=await canvas.boundingBox();
   assert.ok(box.height>=140 && box.width>600);
   const oldStart=await start.inputValue(),oldEnd=await end.inputValue();
   assert.ok((await page.locator('[data-range-edge="end"]').boundingBox()).height >= box.height-1, 'edge hot area must cover the entire vertical line');
   const handle=await page.locator('[data-range-edge="end"]').boundingBox();
   await page.mouse.move(handle.x+handle.width/2,handle.y+handle.height-6);await page.mouse.down();
   await page.mouse.move(handle.x+handle.width/2+16,handle.y+handle.height-6,{steps:5});await page.mouse.up();
   assert.equal(await start.inputValue(),oldStart);assert.notEqual(await end.inputValue(),oldEnd);
   const heldEnd=await end.inputValue();
   await page.locator('[data-range-edge="start"]').focus();await page.keyboard.press('ArrowRight');
   assert.equal(await end.inputValue(),heldEnd);assert.equal(await start.inputValue(),'00:00:00.010');
   const firstEdge=await page.locator('[data-range-edge="start"]').boundingBox();
   await page.mouse.move(firstEdge.x+firstEdge.width/2,firstEdge.y+5);await page.mouse.down();
   await page.mouse.move(firstEdge.x+firstEdge.width/2+10,firstEdge.y+5,{steps:3});await page.mouse.up();
   assert.equal(await end.inputValue(),heldEnd);assert.notEqual(await start.inputValue(),'00:00:00.010');
   // Waveform drags replace the whole old selection, in either direction.
   const ms=value=>value.split(':').reduce((n,v)=>n*60+Number(v),0)*1000;
   await page.mouse.move(box.x+box.width*.4,box.y+50);await page.mouse.down();
   await page.mouse.move(box.x+box.width*.6,box.y+50,{steps:6});await page.mouse.up();
   const replacedStart=ms(await start.inputValue()),replacedEnd=ms(await end.inputValue());
   assert.ok(replacedStart>5000 && replacedEnd>replacedStart);
   await page.mouse.move(box.x+box.width*.85,box.y+60);await page.mouse.down();
   await page.mouse.move(box.x+box.width*.7,box.y+60,{steps:6});await page.mouse.up();
   assert.ok(ms(await start.inputValue())>replacedEnd && ms(await end.inputValue())>ms(await start.inputValue()));
   await end.fill(heldEnd);
   await start.fill('incomplete');assert.equal(await end.inputValue(),heldEnd);
   await start.fill('00:00:05.000');await end.fill('00:00:02.000');
   await page.locator('#preview-retranscription').click();
   assert.match(await page.locator('#retranscription-message').textContent(),/有效范围/);
   assert.equal(await start.inputValue(),'00:00:05.000');assert.equal(await end.inputValue(),'00:00:02.000');
   // Main controls, viewport and paused playhead are the same objects in repair.
   await page.locator('#karaoke-zoom').focus();await page.keyboard.press('ArrowLeft');
   const zoomAfter=await page.locator('#karaoke-zoom').inputValue();assert.notEqual(zoomAfter,zoomBefore);
   await page.locator('#karaoke-waveform-gain').focus();await page.keyboard.press('ArrowRight');
   const gainAfter=await page.locator('#karaoke-waveform-gain').inputValue();
   await page.locator('#retranscription-use-window').click();
   const viewport=await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request);
   assert.ok(Math.abs(ms(await end.inputValue())-ms(await start.inputValue())-(viewport.endMs-viewport.startMs))<2);
   const b=await canvas.boundingBox();
   await page.mouse.click(b.x+b.width*.12,b.y+10);
   await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio').currentTime>0);
   await page.locator('#retranscription-use-current').click();
   assert.match(await page.locator('#retranscription-message').textContent(),/已选第/);
   const pixels=await canvas.evaluate(el=>{const c=el.getContext('2d'),d=c.getImageData(0,0,el.width,el.height).data;let n=0;for(let i=0;i<d.length;i+=4)if(d[i]>140&&d[i]<220&&d[i+1]<100&&d[i+2]<100)n++;return n;});
   assert.ok(pixels>10,'paused playhead must be drawn');
   // Audition leaves the new playback position on returning, and never edits cues.
   await start.fill('00:00:03.000');await end.fill('00:00:03.250');
   await page.locator('#repair-range-audition').click();
   await page.waitForFunction(()=>{const m=document.querySelector('#karaoke-media-host audio');return m.currentTime>=3.25&&m.paused;});
   const position=await page.locator('audio').evaluate(el=>el.currentTime);
   assert.equal(await page.locator('#karaoke-current-source').inputValue(),source);
   if(process.env.ATOGAKI_SCREENSHOTS_DIR) await page.screenshot({path:require('node:path').join(process.env.ATOGAKI_SCREENSHOTS_DIR,`shared-repair-${size.width}.png`)});
   await page.locator('[data-editor-action="close-repair"]').click();
   assert.equal(await page.locator('#editor-wave-panel #karaoke-waveform').count(),1);
   assert.equal(await canvas.getAttribute('data-identity'),'shared');
   assert.equal(await page.locator('#karaoke-zoom').inputValue(),zoomAfter);
   assert.equal(await page.locator('#karaoke-waveform-gain').inputValue(),gainAfter);
   assert.ok(Math.abs(await page.locator('audio').evaluate(el=>el.currentTime)-position)<.05);
   assert.equal(await page.locator('#karaoke-current-source').inputValue(),source);
   assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>c.cmd==='save_subtitle_timing_recorded'||c.cmd==='update_subtitle')),false);
   await page.locator('[data-editor-action="repair"]').click();
   assert.equal(await page.locator('#editor-repair-dialog #karaoke-waveform').count(),1);
   await page.locator('[data-editor-action="close-repair"]').click();
   assert.deepEqual(errors,[]);
   console.log(`PASS ${size.width}: shared timeline controls/playhead, independent edges, invalid input retention, viewport selection, audition and return`);
   await page.close();
  }
 } finally { await browser.close(); }
})().catch(error=>{ console.error(error); process.exitCode=1; });
