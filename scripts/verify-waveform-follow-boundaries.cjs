const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  for(const duration of [20000,10800000]){
   const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
   await require('./editor-ui-fixture.cjs').installEditorFixture(page,30,duration);
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
   await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1);
   await page.waitForTimeout(200);
   const requests=()=>page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').length);
   const before=await requests();await page.locator('#karaoke-toggle-playback').click();await page.waitForTimeout(1100);
   const after=await requests();assert.ok(after-before<=1,`start edge request storm (${duration}): ${after-before}`);
   assert.ok(await page.locator('#karaoke-media-host audio').evaluate(a=>a.currentTime)>.5,'audio continues');
   if(duration===20000){
    await page.locator('#karaoke-media-host audio').evaluate(a=>{a.currentTime=19;a.dispatchEvent(new Event('timeupdate'));});
    await page.waitForTimeout(500);assert.ok(await requests()-after<=1,'short media tail no reload');
    assert.ok(await page.locator('#karaoke-media-host audio').evaluate(a=>a.currentTime)>19,'tail plays');
   }
   assert.deepEqual(errors,[]);await page.close();
  }
  console.log('PASS: short full-media viewport and long-timeline start edge remain drawable without repeated waveform requests during playback');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
