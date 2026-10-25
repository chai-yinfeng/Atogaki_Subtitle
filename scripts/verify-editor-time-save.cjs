const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__timeSave={hold:false,release:null,timing:[],fail:false};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const state=window.__timeSave;
    if(cmd==='update_subtitle'&&state.hold){state.hold=false;await new Promise(resolve=>state.release=resolve);if(state.fail)throw Error('fixture text write failed');}
    if(cmd==='save_subtitle_timing')state.timing.push(args.request);
    return invoke(cmd,args);
   };
  });
  const source=page.locator('#karaoke-current-source'),start=page.locator('#editor-cue-start'),end=page.locator('#editor-cue-end'),save=page.locator('[data-editor-action="save-time"]');
  const prepare=async(startTime='00:00:00.100')=>{
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
   await page.locator('.job-open').first().click();
   await page.evaluate(()=>window.__timeSave.hold=true);
   await source.fill('time-save race fixture');
   await start.fill(startTime);await save.click();
   await page.waitForFunction(()=>window.__timeSave.release);
  };
  const finish=async()=>{
   await page.evaluate(()=>window.__timeSave.release());
   await page.waitForFunction(()=>window.__timeSave.timing.length||/时间输入或编辑段已变化|文字保存失败/.test(document.querySelector('#karaoke-timing-message').textContent));
  };
  for(const scenario of ['start','end','same-value','escape']){
   await prepare();
   if(scenario==='end')await end.fill('00:00:02.700');
   else if(scenario==='escape'){await start.press('Escape');await start.fill('00:00:00.100');}
   else {await start.fill('00:00:00.200');if(scenario==='same-value')await start.fill('00:00:00.100');}
   const expectedStart=await start.inputValue(),expectedEnd=await end.inputValue();
   await finish();
   assert.equal(await page.evaluate(()=>window.__timeSave.timing.length),0,`${scenario}: edited time must cancel the old save intent after text flush`);
   assert.equal(await start.inputValue(),expectedStart);assert.equal(await end.inputValue(),expectedEnd);
  }
  await save.click();
  await page.waitForFunction(()=>window.__timeSave.timing.length===1);
  assert.equal(await page.evaluate(()=>window.__timeSave.timing[0].afterSegments[0].start_ms),100);
  assert.equal(await page.evaluate(()=>window.__test.cues[0].source_text),'time-save race fixture');
  await prepare();await save.dispatchEvent('click');await finish();
  await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('时间已保存'));
  assert.equal(await page.evaluate(()=>window.__timeSave.timing.length),1,'duplicate pending clicks must not submit two timing transactions');
  await prepare('00:00:00.000');await page.locator('[data-editor-action="back"]').click();
  await page.evaluate(()=>window.__timeSave.release());
  await page.locator('.job-open').first().waitFor();
  assert.equal(await page.evaluate(()=>window.__timeSave.timing.length),0,'leaving while text flush is pending must cancel the old timing save');
  await page.locator('.job-open').first().click();assert.equal(await start.inputValue(),'00:00:00.000');assert.equal(await source.inputValue(),'time-save race fixture');
  await prepare();await page.evaluate(()=>window.__timeSave.fail=true);await finish();
  assert.equal(await page.evaluate(()=>window.__timeSave.timing.length),0);assert.equal(await start.inputValue(),'00:00:00.100');assert.equal(await source.inputValue(),'time-save race fixture');
  assert.deepEqual(errors,[]);
  console.log('PASS: changed start/end, same-value retyping, Escape and navigation cancel pending time saves; retry keeps flushed text, duplicate clicks submit once, text failures preserve inputs');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
