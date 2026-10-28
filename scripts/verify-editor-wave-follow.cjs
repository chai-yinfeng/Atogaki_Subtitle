const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright'),assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const p=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];p.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(p,300);
  const open=async()=>{await p.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await p.locator('.job-open').first().click();await p.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1);await p.locator('#karaoke-zoom').fill('0');await p.locator('#karaoke-zoom').dispatchEvent('input');await p.locator('[data-editor-action="follow"]').click();await p.waitForFunction(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1)?.args.request.endMs===2000);};
  const wave=p.locator('#karaoke-waveform'),state=p.locator('#editor-wave-follow-state');
  const request=()=>p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request);
  await open();let box=await wave.boundingBox();
  await p.mouse.click(box.x+box.width*.3,box.y+38);
  assert.match(await state.textContent(),/视窗跟随中/,'plain waveform seek must retain viewport follow');
  await p.locator('#karaoke-toggle-playback').click();await p.waitForFunction(()=>document.querySelector('#karaoke-media-host audio').currentTime>2.2);await p.waitForFunction(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request.startMs>0);
  await p.locator('#karaoke-toggle-playback').click();
  // Clicking a cue without changing its timing also preserves follow.
  await open();box=await wave.boundingBox();const trackTop=28+Math.max(12,(box.height-56)*.5)+(box.height<100?6:14);
  await p.mouse.click(box.x+box.width*.35,box.y+(trackTop+box.height-4)/2);assert.match(await state.textContent(),/视窗跟随中/,'cue selection is not viewport browsing');
  assert.equal(await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='save_subtitle_timing_recorded').length),0);
  // Scrubbing the playhead keeps the intent, but does not recenter underneath the drag.
  await open();box=await wave.boundingBox();const before=(await request()).startMs;
  await p.mouse.move(box.x+2,box.y+28);await p.mouse.down();await p.mouse.move(box.x+box.width*.95,box.y+28,{steps:3});
  assert.equal((await request()).startMs,before,'no recenter during captured playhead drag');await p.mouse.up();
  await p.waitForFunction(()=>Math.abs(document.querySelector('#karaoke-media-host audio').currentTime-1.9)<.12);assert.match(await state.textContent(),/视窗跟随中/);
  // Actual manual pan pauses; waveform-only resume preserves locked cue and unsaved time.
  await open();box=await wave.boundingBox();await p.mouse.move(box.x+box.width*.7,box.y+38);await p.mouse.down();await p.mouse.move(box.x+box.width*.4,box.y+38,{steps:3});await p.mouse.up();
  await p.waitForFunction(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request.startMs>0);assert.match(await state.textContent(),/恢复视窗跟随/);assert.equal(await state.getAttribute('aria-pressed'),'false');
  await p.locator('.editor-cue-row').first().click();await p.locator('#editor-cue-start').fill('00:00:00.010');const source=await p.locator('#karaoke-current-source').inputValue();
  await p.locator('#karaoke-media-host audio').evaluate(a=>{a.currentTime=10;a.dispatchEvent(new Event('timeupdate'));});
  await state.click();assert.equal(await state.getAttribute('aria-pressed'),'true');await p.waitForFunction(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request.startMs>8000);
  assert.equal(await p.locator('#karaoke-current-source').inputValue(),source);assert.equal(await p.locator('#editor-cue-start').inputValue(),'00:00:00.010');assert.equal(await p.locator('[data-editor-action="follow"]').getAttribute('aria-pressed'),'false');
  assert.equal(await p.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='save_subtitle_timing_recorded').length),0);assert.deepEqual(errors,[]);
  console.log('PASS: seek/cue/playhead preserve waveform follow, playback advances viewport, active drag stable, manual pan pauses, independent resume preserves edit/time draft');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
