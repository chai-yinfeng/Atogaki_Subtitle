const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const b=await webkit.launch({headless:true}),p=await b.newPage({viewport:{width:1180,height:760}}),errors=[];
 p.on('pageerror',e=>errors.push(String(e)));
 await require('./editor-ui-fixture.cjs').installEditorFixture(p);
 const url=process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1420';
 const open=async()=>{
  await p.goto(url);await p.locator('.job-open').first().click();await p.locator('.editor-cue-row').first().waitFor();
  await p.locator('#karaoke-media-host audio').evaluate(async a=>{if(a.readyState<1)await new Promise(r=>a.addEventListener('loadedmetadata',r,{once:true}));});
  await p.waitForFunction(()=>window.__test.calls.some(c=>c.cmd==='get_waveform_window'));
  await p.locator('#karaoke-waveform').evaluate(c=>c.addEventListener('pointerdown',e=>window.__gesturePointer=e.pointerId));
 };
 await open();const canvas=p.locator('#karaoke-waveform');
 const calls=()=>p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='save_subtitle_timing_recorded').length);
 const box=await canvas.boundingBox(),height=box.height;
 const trackTop=28+Math.max(12,(height-56)*.5)+(height<100?6:14),y=box.y+(trackTop+height-4)/2;
 const state=await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request);
 const endX=box.x+2800/(state.endMs-state.startMs)*box.width;
 const startDrag=async()=>{await p.mouse.move(endX-2,y);await p.mouse.down();await p.mouse.move(endX-17,y,{steps:2});};
 const original=await p.evaluate(()=>window.__test.cues[0].end_ms);
 await startDrag();await p.keyboard.press('Escape');await p.mouse.up();
 assert.equal(await calls(),0);assert.equal(await p.evaluate(()=>window.__test.cues[0].end_ms),original);
 // Loss of capture cancels every gesture state and must not block another drag.
 await startDrag();await canvas.evaluate(c=>c.releasePointerCapture(window.__gesturePointer));await p.mouse.move(endX-19,y);await p.mouse.up();
 assert.equal(await calls(),0);
 await startDrag();await p.mouse.up();await p.waitForFunction(()=>window.__test.calls.some(c=>c.cmd==='save_subtitle_timing_recorded'));
 assert.ok(await p.evaluate(original=>window.__test.cues[0].end_ms<original,original));
 const savedWrites=await calls();
 // Cancelling a later gesture never sends another transaction.
 await p.mouse.move(box.x+box.width*.35,box.y+35);await p.mouse.down();
 await canvas.dispatchEvent('pointercancel',{pointerId:await p.evaluate(()=>window.__gesturePointer)});await p.mouse.up();
 assert.equal(await calls(),savedWrites);
 // Dragging the audio background pans but never edits timing.
 await p.mouse.move(box.x+box.width*.6,box.y+35);await p.mouse.down();await p.mouse.move(box.x+box.width*.6-45,box.y+35,{steps:3});await p.mouse.up();
 await p.waitForFunction(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request.startMs>0);
 assert.equal(await calls(),savedWrites);assert.match(await p.locator('#editor-wave-follow-state').innerText(),/已暂停/);
 const zoomBefore=Number(await p.locator('#karaoke-zoom').inputValue());
 await p.mouse.move(box.x+box.width*.5,box.y+35);await p.keyboard.down('Control');await p.mouse.wheel(0,-120);await p.keyboard.up('Control');
 assert.ok(Number(await p.locator('#karaoke-zoom').inputValue())<zoomBefore);
 // Safari pinch and ctrl-wheel cannot both apply one zoom gesture.
 const pinch=async(type,scale)=>canvas.evaluate((c,{type,scale})=>{
  const e=new Event(type,{bubbles:true,cancelable:true});Object.defineProperties(e,{scale:{value:scale},clientX:{value:c.getBoundingClientRect().x+c.clientWidth/2}});c.dispatchEvent(e);
 },{type,scale});
 await pinch('gesturestart',1);const pinchZoom=await p.locator('#karaoke-zoom').inputValue();
 await canvas.dispatchEvent('wheel',{ctrlKey:true,deltaY:-100,clientX:box.x+100});assert.equal(await p.locator('#karaoke-zoom').inputValue(),pinchZoom);
 await pinch('gesturechange',2);assert.ok(Number(await p.locator('#karaoke-zoom').inputValue())<Number(pinchZoom));await pinch('gestureend',2);
 // Navigation cancels an active preview and releases capture before another task mount.
 await open();const fresh=await canvas.boundingBox(),freshHeight=fresh.height;
 const freshTop=28+Math.max(12,(freshHeight-56)*.5)+(freshHeight<100?6:14),freshY=fresh.y+(freshTop+freshHeight-4)/2;
 await p.mouse.move(fresh.x+30,freshY);await p.mouse.down();await p.mouse.move(fresh.x+36,freshY);
 await p.locator('[data-editor-action="back"]').evaluate(button=>button.click());await p.locator('.job-open').first().waitFor();await p.mouse.up();
 assert.equal(await calls(),0);await p.locator('.job-open').first().click();await canvas.waitFor();
 await p.locator('[data-editor-action="follow"]').click();
 const headBox=await canvas.boundingBox();
 await p.locator("#karaoke-media-host audio").evaluate(a=>a.addEventListener("seeked",()=>window.__headSeeked=a.currentTime));
 await p.mouse.move(headBox.x+2,headBox.y+28);await p.mouse.down();await p.mouse.move(headBox.x+headBox.width*.25,headBox.y+28,{steps:3});await p.mouse.up();
 // WebKit applies currentTime asynchronously during successive pointer moves.
 const headTarget=await p.evaluate(()=>{const r=window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request;return (r.startMs+(r.endMs-r.startMs)*.25)/1000;});
 await p.waitForFunction(target=>{const a=document.querySelector('#karaoke-media-host audio');return !a.seeking && a.readyState>=2 && Math.abs(a.currentTime-target)<.15 && Math.abs(window.__headSeeked-target)<.15;},headTarget,{polling:50,timeout:10000}).catch(async error=>{console.error('final seek state',await p.locator('#karaoke-media-host audio').evaluate(a=>({time:a.currentTime,seeking:a.seeking,ready:a.readyState,seeked:window.__headSeeked})),{headTarget});throw error;});
 assert.equal(await p.locator('[data-editor-action="follow"]').getAttribute('aria-pressed'),'true');
 assert.deepEqual(errors,[]);console.log('PASS: Escape, capture loss, fresh drag, cancel, pan, ctrl-wheel, pinch, task navigation and playhead seek');
 await b.close();
})().catch(e=>{console.error(e);process.exit(1)});
