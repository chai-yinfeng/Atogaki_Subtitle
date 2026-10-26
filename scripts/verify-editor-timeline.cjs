const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const b=await webkit.launch({headless:true}),p=await b.newPage({viewport:{width:960,height:640}}),errors=[];
 p.on('pageerror',e=>errors.push(String(e)));
 await require('./editor-ui-fixture.cjs').installEditorFixture(p);
 await p.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1420');await p.locator('.job-open').first().click();
 const canvas=p.locator('#karaoke-waveform');await canvas.waitFor();
 await p.waitForFunction(()=>window.__test.calls.some(c=>c.cmd==='get_waveform_window'));
 const colors=await canvas.evaluate(c=>{
  const px=c.getContext('2d').getImageData(c.width-2,2,1,1).data;return [...px].slice(0,3);
 });
 assert.deepEqual(colors,[238,241,245]);
 // In the compact view the bottom subtitle track remains visible and hittable.
 const box=await canvas.boundingBox();
 const height=box.height, trackTop=28+Math.max(12,(height-56)*.5)+(height<100?6:14);
 const y=box.y+(trackTop+height-4)/2;
 const before=await p.evaluate(()=>window.__test.cues[0].end_ms);
 const window=await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request);
 const endX=box.x+(before-window.startMs)/(window.endMs-window.startMs)*box.width;
 await p.mouse.move(endX-2,y);await p.mouse.down();await p.mouse.move(endX-14,y,{steps:3});await p.mouse.up();
 await p.waitForFunction(before=>window.__test.cues[0].end_ms<before,before,{timeout:5000});
 assert.equal(await p.locator('[data-editor-action="follow"]').getAttribute('aria-pressed'),'false');
 assert.match(await p.locator('[data-cue-id="cue-0"]').getAttribute('class'),/selected/);
 assert.ok(await p.evaluate(()=>window.__test.calls.some(c=>c.cmd==='save_subtitle_timing_recorded')));
 // Refusing a selection must also refuse its timeline drag, not just the list click.
 const writes=await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='save_subtitle_timing_recorded').length);
 const dragOther=async()=>{
  const x=box.x+(4400-window.startMs)/(window.endMs-window.startMs)*box.width;
  await p.mouse.move(x,y);await p.mouse.down();await p.mouse.move(x+12,y,{steps:2});await p.mouse.up();
 };
 await p.locator('#editor-cue-start').fill('00:00:00.100');await dragOther();
 assert.match(await p.locator('[data-cue-id="cue-0"]').getAttribute('class'),/selected/);
 assert.equal(await p.locator('#editor-cue-start').inputValue(),'00:00:00.100');
 await p.locator('#editor-cue-start').focus();await p.keyboard.press('Escape');
 await p.locator('#karaoke-current-source').evaluate(input=>input.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true})));
 await dragOther();
 assert.match(await p.locator('[data-cue-id="cue-0"]').getAttribute('class'),/selected/);
 assert.equal(await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='save_subtitle_timing_recorded').length),writes);
 await p.locator('#karaoke-current-source').evaluate(input=>input.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true})));
 await p.locator('[data-editor-toggle="waveform"]').click();
 await p.waitForTimeout(100);
 const requests=await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').length);
 await p.locator('#karaoke-media-host audio').evaluate(async audio=>{
  if(audio.readyState<1)await new Promise(r=>audio.addEventListener('loadedmetadata',r,{once:true}));
  await new Promise(r=>{audio.addEventListener('seeked',r,{once:true});audio.currentTime=19;});audio.dispatchEvent(new Event('timeupdate'));
 });
 await p.waitForTimeout(100);
 assert.equal(await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').length),requests);
 await p.locator('[data-editor-toggle="waveform"]').click();
 assert.equal(await canvas.isVisible(),true);
 assert.deepEqual(errors,[]);console.log('PASS: editor palette, compact track drag, timing persistence, folded waveform request boundary');
 await b.close();
})().catch(e=>{console.error(e);process.exit(1)});
