const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright'),assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[],requests=[];
  page.on('pageerror',e=>errors.push(String(e)));page.on('request',r=>{if(r.url().includes('/qa-demand.wav'))requests.push(r.url());});
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,6,18000);
  await page.addInitScript(()=>{window.__qaDemandData=window.__qaAudio;window.__qaAudio=new URL('/qa-demand.wav',location.href).href;});
  await page.route('**/qa-demand.wav',async route=>{const data=await page.evaluate(()=>window.__qaDemandData);await route.fulfill({status:200,contentType:'audio/wav',body:Buffer.from(data.split(',')[1],'base64')});});
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  await page.locator('#karaoke-current-source').waitFor();await page.waitForTimeout(500);
  const audio=()=>page.locator('#karaoke-media-host audio');
  assert.equal(await audio().getAttribute('preload'),'none');assert.equal(requests.length,0,'opening editor must not fetch audio');
  await page.locator('[data-editor-action="follow"]').click();await page.locator('#editor-search').fill('第 3 句');
  await page.locator('[data-cue-id="cue-2"]').click();assert.equal(requests.length,0,'selecting text and following must not load media');
  await page.locator('#editor-search').fill('');
  await page.locator('.editor-menu > summary').click();await page.locator('#karaoke-insert-block').click();
  await page.locator('#insert-subtitle-block-dialog [name="start"]').waitFor();
  assert.equal(requests.length,0,'opening insertion form uses waveform duration without audio preload');
  await page.locator('#insert-subtitle-block-dialog [data-close]').click();
  await page.locator('#karaoke-step-forward').click();
  await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio').readyState>=1&&document.querySelector('#karaoke-media-host audio').currentTime>=.099);
  assert.ok(requests.length>0);assert.equal(await audio().evaluate(a=>a.paused),true,'paused seek must stay paused');
  await page.locator('[data-cue-id="cue-2"] [data-cue-time]').click();
  await page.waitForFunction(()=>{const a=document.querySelector('#karaoke-media-host audio');return !a.paused&&a.currentTime>=6});
  await page.locator('#karaoke-toggle-playback').click();const time=await audio().evaluate(a=>a.currentTime);assert.ok(time>=6&&time<9);
  await page.locator('[data-editor-action="back"]').click();const before=requests.length;await page.locator('.job-open').first().click();
  await page.waitForTimeout(500);assert.equal(requests.length,before,'same-task resume must remain deferred');
  assert.match(await page.locator('#karaoke-current-source').inputValue(),/第 3 句/);
  await page.locator('#karaoke-toggle-playback').click();await page.waitForFunction(t=>{const a=document.querySelector('#karaoke-media-host audio');return a.readyState>=1&&!a.paused&&a.currentTime>=t},time);
  assert.deepEqual(errors,[]);console.log('PASS: HTTP audio deferred on open/select/resume, paused seek loads, native play and resumed position preserved');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
