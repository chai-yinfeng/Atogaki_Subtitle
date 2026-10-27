const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const base=window.__TAURI_INTERNALS__.invoke,test=window.__test;
   test.contextStaleIds=[];
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='get_translation_context_state'){
     test.calls.push({cmd,args});
     return {workspace_revision:42,translation_context_stale_ids:[...test.contextStaleIds]};
    }
    const result=await base(cmd,args);
    if(cmd==='update_subtitle')test.contextStaleIds=test.nextContextStaleIds??['cue-2'];
    return result;
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  const consumer=page.locator('[data-cue-id="cue-2"]');
  assert.equal(await consumer.locator('.cue-status').innerText(),'已保存');
  await page.evaluate(()=>{window.__consumerNode=document.querySelector('[data-cue-id="cue-2"]');window.__editorNode=document.querySelector('#karaoke-current-source');});
  await page.locator('#karaoke-current-source').fill('changed context source');
  await page.waitForFunction(()=>document.querySelector('[data-cue-id="cue-2"] .cue-status')?.textContent==='待重译',null,{timeout:5000});
  assert.equal(await page.evaluate(()=>document.querySelector('[data-cue-id="cue-2"]')===window.__consumerNode),true,'preserve virtual row node');
  assert.equal(await page.evaluate(()=>document.querySelector('#karaoke-current-source')===window.__editorNode),true,'preserve editor and its focus');
  assert.equal(await page.locator('#karaoke-current-source').inputValue(),'changed context source');
  const calls=await page.evaluate(()=>window.__test.calls);
  assert.equal(calls.filter(c=>c.cmd==='get_job_detail').length,1,'never reload full task on save');
  assert.equal(calls.filter(c=>c.cmd==='get_translation_context_state').length,1,'one on-demand lightweight query');
  assert.equal(await page.evaluate(()=>window.__test.cues[2].translation_stale),false,'consumer is stale through context metadata only');
  await page.locator('#editor-filter').selectOption('stale');
  assert.equal(await page.locator('[data-cue-id="cue-2"]').count(),1,'context-stale consumer participates in current filter');
  await page.evaluate(()=>window.__test.nextContextStaleIds=[]);
  await page.locator('#karaoke-current-source').fill('changed context source again');
  await page.waitForFunction(()=>window.__test.calls.filter(c=>c.cmd==='get_translation_context_state').length===2&&!document.querySelector('[data-cue-id="cue-2"]'));
  assert.equal(await page.locator('#karaoke-current-source').inputValue(),'changed context source again','active stale filter refresh preserves current input');
  assert.equal(await page.evaluate(()=>document.querySelector('#karaoke-current-source')===window.__editorNode),true);
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_job_detail').length),1);

  assert.deepEqual(errors,[]);
  console.log('PASS: single-row save immediately updates context consumer badge/filter with one lightweight query, preserving row/editor nodes and no full task reload');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
