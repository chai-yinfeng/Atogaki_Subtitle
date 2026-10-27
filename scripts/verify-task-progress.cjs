const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.setDefaultTimeout(10000);
  page.on('pageerror',error=>errors.push(String(error)));
  await page.clock.install();
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__progressQA={status:'transcribing',hold:false,release:null,rejectOld:false,reads:0,active:0,maxActive:0};
   window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
    if(command!=='get_job_detail')return invoke(command,args);
    const qa=window.__progressQA,status=qa.status,held=qa.hold;qa.hold=false;qa.reads++;qa.active++;qa.maxActive=Math.max(qa.maxActive,qa.active);
    try {
     if(held){await new Promise(resolve=>qa.release=resolve);if(qa.rejectOld)throw Error('迟到旧刷新错误');}
     const result=await invoke(command,args);
     return {...result,job:{...result.job,status,message:held?'迟到旧刷新内容':'隔离进度：'+status}};
    } finally {qa.active--;}
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  const open=async()=>{await page.locator('.job-open').first().click();};
  const home=async()=>{
   const editor=await page.locator('#karaoke-current-source').isVisible();
   await page.locator(editor?'[data-editor-action="back"]':'#back-to-jobs').click();
   await page.locator('.job-open').first().waitFor({state:'visible'});
  };
  const drain=()=>page.evaluate(()=>new Promise(resolve=>queueMicrotask(()=>queueMicrotask(resolve))));
  const reads=()=>page.evaluate(()=>window.__progressQA.reads);
  await open();await page.locator('#workspace-message').waitFor({state:'visible'});
  assert.match(await page.locator('#workspace-message').innerText(),/transcribing/);
  const initial=await reads();
  await page.evaluate(()=>window.__progressQA.hold=true);
  await page.clock.fastForward(2100);await page.waitForFunction(()=>window.__progressQA.release);
  await page.clock.fastForward(10000);assert.equal(await reads(),initial+1,'slow progress reads must not overlap');
  assert.equal(await page.evaluate(()=>window.__progressQA.maxActive),1);
  const oldMessage=await page.locator('#workspace-message').elementHandle();
  await home();await page.evaluate(()=>window.__progressQA.status='done');await open();
  await page.locator('#karaoke-current-source').waitFor({state:'visible'});
  const before=await oldMessage.textContent();
  await page.evaluate(()=>{window.__progressQA.release();window.__progressQA.release=null;});await drain();
  assert.equal(await oldMessage.textContent(),before,'old same-job progress must not rewrite the reopened visit');
  const completed=await reads();await page.clock.fastForward(8000);assert.equal(await reads(),completed,'completed editor must not poll full detail');
  // Reopening the same job does not authorize errors from its previous visit.
  await home();await page.evaluate(()=>window.__progressQA.status='transcribing');await open();
  await page.locator('#workspace-message').waitFor({state:'visible'});
  await page.evaluate(()=>window.__progressQA.hold=true);await page.clock.fastForward(2100);await page.waitForFunction(()=>window.__progressQA.release);
  const oldErrorMessage=await page.locator('#workspace-message').elementHandle();
  await home();await page.evaluate(()=>{window.__progressQA.status='done';window.__progressQA.rejectOld=true;});await open();
  await page.locator('#karaoke-current-source').waitFor({state:'visible'});
  const beforeError=await oldErrorMessage.textContent();
  await page.evaluate(()=>{window.__progressQA.release();window.__progressQA.release=null;});await drain();
  assert.equal(await oldErrorMessage.textContent(),beforeError);
  // Completion while remaining on the progress page releases its timer too.
  await home();await page.evaluate(()=>{window.__progressQA.status='transcribing';window.__progressQA.rejectOld=false;});await open();
  await page.locator('#workspace-message').waitFor({state:'visible'});
  await page.evaluate(()=>window.__progressQA.status='done');await page.clock.fastForward(2100);
  await page.waitForFunction(()=>document.querySelector('#workspace-message').textContent.includes('隔离进度：done'));
  const finished=await reads();await page.clock.fastForward(8000);assert.equal(await reads(),finished,'completed progress page must stop reading');
  assert.deepEqual(errors,[]);
  console.log('PASS: single-flight mounted progress, old same-job result/error isolation, no completed-editor polling and timer release on completion');
 } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
