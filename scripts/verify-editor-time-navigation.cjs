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
   window.__timeNavigation={hold:false,release:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const state=window.__timeNavigation;
    if(cmd==='update_subtitle'&&state.hold){state.hold=false;await new Promise(resolve=>state.release=resolve);}
    return invoke(cmd,args);
   };
  });
  const open=async()=>{await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();};
  const start=page.locator('#editor-cue-start'),back=page.locator('[data-editor-action="back"]'),notice=page.locator('#karaoke-timing-message');
  await open();await start.fill('00:00:00.100');await back.click();
  await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('时间')||!document.querySelector('.job-open').closest('.hidden'));
  assert.equal(await start.isVisible(),true,'unsaved timing must keep the editor open');
  assert.equal(await start.inputValue(),'00:00:00.100');assert.match(await notice.innerText(),/保存时间|时间.*保存/);
  await page.locator('[data-editor-action="export"]').click();await page.locator('#back-to-jobs').click();
  assert.equal(await start.inputValue(),'00:00:00.100','opening and closing tools must retain pending timing');
  await start.press('Escape');await back.click();await page.locator('.job-open').first().waitFor();
  await page.locator('.job-open').first().click();assert.equal(await start.inputValue(),'00:00:00.000');
  await start.fill('00:00:00.100');await page.locator('[data-editor-action="save-time"]').click();
  await page.waitForFunction(()=>window.__test.cues[0].start_ms===100);
  await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('时间已保存'));
  await back.click();
  await page.locator('.job-open').first().waitFor();assert.equal(await start.isVisible(),false);
  await page.locator('.job-open').first().click();
  await page.waitForFunction(()=>document.querySelector('#editor-cue-start').value==='00:00:00.100');
  assert.equal(await start.inputValue(),'00:00:00.100');
  // A new input during the asynchronous leave preflight must also block teardown.
  await open();
  await page.evaluate(()=>window.__timeNavigation.hold=true);
  await page.locator('#karaoke-current-source').fill('leave preflight text');await back.click();
  await page.waitForFunction(()=>window.__timeNavigation.release);
  await start.fill('00:00:00.200');await page.evaluate(()=>window.__timeNavigation.release());
  await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('时间'));
  assert.equal(await start.isVisible(),true);assert.equal(await start.inputValue(),'00:00:00.200');
  assert.equal(await page.locator('#karaoke-current-source').inputValue(),'leave preflight text');
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='save_subtitle_timing').length),0);
  await start.press('Escape');await back.click();await page.locator('.job-open').first().waitFor();
  assert.deepEqual(errors,[]);
  console.log('PASS: pending time blocks navigation before and after text flush; tools retain input, Escape permits leaving, same-task reopen shows saved time');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
