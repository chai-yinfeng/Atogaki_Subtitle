const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__navigationQA={hold:true,release:null,incomplete:false};
   window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
    const qa=window.__navigationQA;
    if(command==='get_waveform_window'&&qa.hold){qa.hold=false;await new Promise(resolve=>qa.release=resolve);}
    const result=await invoke(command,args);
    if(command==='get_job_detail'&&qa.incomplete) result.job={...result.job,status:'transcribing',message:'隔离：仍在识别'};
    return result;
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  await page.locator('.job-open').first().click();
  await page.waitForFunction(()=>window.__navigationQA.release);
  await page.locator('[data-editor-action="back"]').click();
  await page.locator('.job-open').first().waitFor({state:'visible'});
  const counts=()=>page.evaluate(()=>Object.fromEntries(['list_asr_quality_signals','translation_status'].map(command=>[command,window.__test.calls.filter(call=>call.cmd===command).length])));
  const before=await counts();
  await page.evaluate(()=>{window.__navigationQA.release();window.__navigationQA.release=null;});
  // Cross task queues/RAF: wait for the released open workflow to drain.
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  assert.deepEqual(await counts(),before,'a departed editor visit must not initialize review or translation after a late waveform');
  assert.equal(await page.locator('#karaoke-current-source').isVisible(),false);
  await page.locator('.job-open').first().click();
  await page.waitForFunction(()=>window.__test.calls.some(call=>call.cmd==='list_asr_quality_signals'));
  assert.equal(await page.locator('#karaoke-current-source').isVisible(),true);
  assert.match(await page.locator('#karaoke-current-source').inputValue(),/第 1 句/);
  await page.locator('[data-editor-action="back"]').click();
  await page.locator('.job-open').first().waitFor({state:'visible'});
  const reads=await page.evaluate(()=>{window.__navigationQA.incomplete=true;return window.__test.calls.filter(call=>call.cmd==='get_job_detail').length;});
  await page.locator('.job-open').first().click();
  await page.waitForFunction(()=>document.querySelector('#workspace-title')?.textContent.includes('任务进度'));
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  assert.equal(await page.locator('#karaoke-current-source').isVisible(),false);
  assert.equal(await page.locator('#workspace-message').isVisible(),true);
  assert.match(await page.locator('#workspace-message').innerText(),/仍在识别/);
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(call=>call.cmd==='get_job_detail').length),reads+1,'incomplete detail must settle in progress without repeatedly following the stale completed summary');
  await page.locator('#back-to-jobs').click();
  await page.locator('.job-open').first().waitFor({state:'visible'});
  await page.evaluate(()=>window.__navigationQA.incomplete=false);
  await page.locator('.job-open').first().click();
  assert.equal(await page.locator('#karaoke-current-source').isVisible(),true);
  assert.deepEqual(errors,[]);
  console.log('PASS: late waveform cannot initialize a departed visit; stale completed summary with incomplete detail settles in progress after one read; reopening returns to editor');
 } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
