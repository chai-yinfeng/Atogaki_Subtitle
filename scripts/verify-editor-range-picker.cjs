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
   // A stalled waveform read must not stall the visual viewport while panning.
   await page.evaluate(()=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    window.__heldWave=[];
    window.__TAURI_INTERNALS__.invoke=(command,args)=>command==='get_waveform_window'
     ? new Promise(resolve=>window.__heldWave.push(()=>resolve(invoke(command,args)))) : invoke(command,args);
   });
   const mainCanvas=page.locator('#karaoke-waveform'), mainBox=await mainCanvas.boundingBox();
   const bitmapBefore=await mainCanvas.evaluate(el=>el.toDataURL());
   await page.mouse.move(mainBox.x+mainBox.width*.7,mainBox.y+35); await page.mouse.down();
   await page.mouse.move(mainBox.x+mainBox.width*.7-30,mainBox.y+35,{steps:5}); await page.mouse.up();
   await page.waitForFunction(()=>window.__heldWave.length>0);
   await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
   assert.notEqual(await mainCanvas.evaluate(el=>el.toDataURL()),bitmapBefore,'pan must repaint before backend data arrives');
   await page.evaluate(()=>{ for(const release of window.__heldWave.splice(0)) release(); });
   await page.waitForTimeout(30);
   await page.evaluate(()=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=(command,args)=>{
     const result=invoke(command,args);
     if(command==='get_waveform_window') for(const release of window.__heldWave.splice(0)) release();
     return result;
    };
   });
   await page.locator('[data-editor-action="repair"]').click();
   await page.waitForFunction(()=>document.querySelector('#repair-wave-status').textContent.includes('拖动'));
   assert.equal(await page.locator('#retranscription-start').inputValue(),'00:00:00.000');
   assert.equal(await page.locator('#retranscription-end').inputValue(),'00:00:02.800');
   assert.match(await page.locator('#repair-range-text').textContent(), /#1/);
   const box = await page.locator('#repair-range-canvas').boundingBox();
   assert.ok(box.height>=140 && box.width>600);
   // Select part of two cues, then explicitly expand to complete blocks.
   await page.mouse.move(box.x+box.width*.12,box.y+40); await page.mouse.down();
   await page.mouse.move(box.x+box.width*.5,box.y+40,{steps:8}); await page.mouse.up();
   await page.waitForFunction(()=>document.querySelector('#repair-range-summary').textContent.includes('切穿'));
   const before=await page.locator('#retranscription-start').inputValue();
   await page.locator('#retranscription-expand-segments').click();
   await page.waitForFunction(()=>document.querySelector('#repair-range-summary').textContent.includes('完整字幕范围'));
   assert.notEqual(await page.locator('#retranscription-start').inputValue(),before);
   const end = page.locator('[data-range-edge="end"]');
   await end.focus(); const oldEnd=await page.locator('#retranscription-end').inputValue();
   await page.keyboard.press('ArrowLeft');
   assert.notEqual(await page.locator('#retranscription-end').inputValue(),oldEnd);
   // Manual fields update the overlay and affected text, without changing cues.
   await page.locator('#retranscription-start').fill('00:00:03.000');
   await page.locator('#retranscription-end').fill('00:00:05.800');
   await page.waitForFunction(()=>document.querySelector('#repair-range-text').textContent.startsWith('#2 '));
   assert.equal(await page.locator('#karaoke-current-source').inputValue(),source);
   assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>c.cmd==='save_subtitle_timing_recorded'||c.cmd==='update_subtitle')),false);
   await page.locator('#retranscription-end').fill('00:00:03.250');
   await page.locator('#repair-range-audition').click();
   await page.waitForFunction(()=>{const media=document.querySelector('#karaoke-media-host audio');return media.currentTime>=3.25 && media.paused;});
   assert.equal(await page.locator('#karaoke-current-source').inputValue(),source);
   assert.equal(await page.locator('audio,video').count(),1);
   await page.locator('[data-range-pan="1"]').click();
   await page.waitForFunction(()=>document.querySelector('[data-range-edge="start"]').hidden);
   await page.locator('[data-editor-action="close-repair"]').click();
   assert.equal(await page.locator('#editor-repair-dialog').count(),0);
   await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio').currentTime<1);
   assert.equal(await page.locator('#karaoke-current-source').inputValue(),source);
   // Task chrome must retain a light surface even with the image-theme rule.
   await page.evaluate(()=>document.documentElement.setAttribute('data-image-theme','moonlight'));
   await page.locator('#karaoke-open-workspace').click();
   const colors=await page.locator('.editor-task-tools .workspace-heading').evaluate(el=>({bg:getComputedStyle(el).backgroundColor,ink:getComputedStyle(el.querySelector('h2')).color}));
   assert.equal(colors.bg,'rgb(255, 255, 255)');
   assert.equal(colors.ink,'rgb(32, 38, 48)');
   assert.equal(await page.locator('.editor-task-tools .workspace-section-tab.active').evaluate(el=>getComputedStyle(el).backgroundColor),'rgb(226, 236, 248)');
   assert.deepEqual(errors,[]);
   console.log(`PASS ${size.width}: graphical selection, explicit alignment, keyboard/manual times, pan and unchanged cue data`);
   await page.close();
  }
 } finally { await browser.close(); }
})().catch(error=>{ console.error(error); process.exitCode=1; });
