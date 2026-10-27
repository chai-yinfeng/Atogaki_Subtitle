const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__listeningQA={hold:null,release:null,resume:4500};
   window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
    const qa=window.__listeningQA,resume=qa.resume;
    if(qa.hold===command){qa.hold=null;await new Promise(resolve=>qa.release=resolve);}
    const result=await invoke(command,args);
    if(command==='list_job_summaries')return result.map(job=>({...job,translation_status:'translated',translated_segment_count:job.segment_count}));
    if(command==='get_playback_position')return resume;
    if(command==='save_playback_position'){qa.resume=args.request.positionMs;return null;}
    return result;
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  const audio=page.locator('#listening-media-host audio');
  const listen=async()=>{await page.locator('#show-listening').click();await page.locator('[data-listen-job]').first().click();};
  const home=async()=>{await page.locator('#show-workbench').click();await page.locator('.job-open').first().waitFor({state:'visible'});};
  await page.locator('.job-open').first().waitFor({state:'visible'});
  for(const held of ['get_playback_position','list_learning_items']) {
   await page.evaluate(command=>{window.__listeningQA.hold=command;window.__listeningQA.resume=4500;},held);
   await listen();await page.waitForFunction(()=>window.__listeningQA.release);
   assert.equal(await audio.count(),0,'pending visit cannot mount media before all required reads complete');
   await home();await page.evaluate(()=>window.__listeningQA.resume=9000);await listen();
   await page.waitForFunction(()=>{const a=document.querySelector('#listening-media-host audio');return a?.readyState>=1&&a.currentTime>=8.9;});
   const current=await audio.elementHandle();
   await page.evaluate(()=>{window.__listeningQA.release();window.__listeningQA.release=null;});
   await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
   assert.equal(await current.evaluate(a=>a===document.querySelector('#listening-media-host audio')),true,'late prior visit must not replace the shared player');
   assert.ok(await audio.evaluate(a=>a.currentTime>=8.9&&a.currentTime<9.2));
   assert.equal(await audio.count(),1);await home();assert.equal(await audio.count(),0);
   assert.equal(await page.evaluate(()=>window.__listeningQA.resume),9000,'leaving persists the current position');
  }
  assert.deepEqual(errors,[]);
  console.log('PASS: held listening position/learning reads cannot replace a reopened same-job player; new position persists and exactly one media element is mounted');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
