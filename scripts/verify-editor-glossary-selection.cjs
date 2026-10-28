const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,20,60000);
  await page.addInitScript(()=>{
   const base=window.__TAURI_INTERNALS__.invoke,test=window.__test;
   test.glossaryPending=[];test.holdInspection=false;test.appliedGlossaries=[];test.snapshotPending=[];test.holdSnapshot=false;
   const glossary=id=>({id,name:`词表 ${id}`,source_language:'ja',builtin_key:null,core_term_count:0,content_group_count:0,correction_only_count:1});
   const detail=id=>({glossary:glossary(id),terms:[{id:`term-${id}`,glossary_id:id,source_text:`错误 ${id}`,target_text:`修正 ${id}`,prompt_scope:'correction_only',content_group:null}]});
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='list_glossaries')return ['A','B'].map(glossary);
    if(cmd==='get_glossary')return test.holdInspection?new Promise((resolve,reject)=>test.glossaryPending.push({kind:'inspection',id:args.glossaryId,resolve:()=>resolve(detail(args.glossaryId)),reject})):detail(args.glossaryId);
    if(cmd==='preview_glossary_application')return new Promise((resolve,reject)=>test.glossaryPending.push({kind:'preview',id:args.glossaryId,resolve:()=>resolve({glossary_id:args.glossaryId,glossary_name:`词表 ${args.glossaryId}`,changes:[{segment_id:'cue-0',segment_index:0,before_text:'原文',after_text:`修正 ${args.glossaryId}`,translation_will_be_stale:true}]}),reject}));
    if(cmd==='apply_glossary_to_workspace'){test.appliedGlossaries.push(args);return {changed_segments:0,stale_translations:0,segments:structuredClone(test.cues)};}
    if(cmd==='get_job_glossary_snapshot'&&test.holdSnapshot)return new Promise((resolve,reject)=>test.snapshotPending.push({resolve,reject}));
    return base(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  await page.locator('.job-open').first().click();
  await page.getByRole('button',{name:'翻译与词表',exact:true}).click();
  const selection=page.locator('#workspace-glossary'),preview=page.locator('#glossary-preview');
  await selection.selectOption('A');await page.locator('#preview-glossary').click();
  await page.waitForFunction(()=>window.__test.glossaryPending.length===1);
  await selection.selectOption('B');
  await page.evaluate(()=>window.__test.glossaryPending[0].resolve());
  await page.waitForFunction(()=>!document.querySelector('#preview-glossary').disabled);
  assert.equal(await preview.isVisible(),false,'A preview must stay cleared after selecting B');
  assert.equal(await page.locator('#apply-previewed-glossary').count(),0);
  // Same-selection stale responses must also fail after A -> B -> A.
  await selection.selectOption('A');await page.locator('#preview-glossary').click();
  await page.waitForFunction(()=>window.__test.glossaryPending.length===2);
  await selection.selectOption('B');await selection.selectOption('A');
  await page.evaluate(()=>window.__test.glossaryPending[1].resolve());
  await page.waitForFunction(()=>!document.querySelector('#preview-glossary').disabled);
  assert.equal(await preview.isVisible(),false);
  // Current responses still render, while an obsolete error cannot replace them.
  await page.locator('#preview-glossary').click();
  await page.waitForFunction(()=>window.__test.glossaryPending.length===3);
  await page.evaluate(()=>window.__test.glossaryPending[2].resolve());
  await page.locator('#apply-previewed-glossary').waitFor();assert.match(await preview.textContent(),/词表 A/);
  await page.locator('#preview-glossary').click();
  await page.waitForFunction(()=>window.__test.glossaryPending.length===4);
  await selection.selectOption('B');
  await page.evaluate(()=>window.__test.glossaryPending[3].reject(Error('obsolete preview error')));
  await page.waitForFunction(()=>!document.querySelector('#preview-glossary').disabled);
  assert.equal(await preview.isVisible(),false);
  assert.doesNotMatch(await page.locator('#workspace-action-message').textContent(),/obsolete preview error/);
  // Rules inspection has an independent latest-request boundary, including ABA.
  await page.evaluate(()=>window.__test.holdInspection=true);
  await selection.selectOption('A');await selection.selectOption('B');await selection.selectOption('A');
  await page.waitForFunction(()=>window.__test.glossaryPending.filter(x=>x.kind==='inspection').length===3);
  await page.evaluate(()=>window.__test.glossaryPending.filter(x=>x.kind==='inspection')[2].resolve());
  await page.waitForFunction(()=>document.querySelector('#workspace-glossary-mappings').textContent.includes('修正 A'));
  await page.evaluate(()=>{const requests=window.__test.glossaryPending.filter(x=>x.kind==='inspection');requests[1].resolve();requests[0].reject(Error('obsolete rules error'));});
  await page.waitForTimeout(50);
  assert.match(await page.locator('#workspace-glossary-mappings').textContent(),/修正 A/);
  assert.equal(await page.evaluate(()=>window.__test.appliedGlossaries.length),0);
  // A valid current preview remains usable and applies the captured glossary.
  await page.locator('#preview-glossary').click();
  await page.waitForFunction(()=>window.__test.glossaryPending.filter(x=>x.kind==='preview').length===5);
  await page.evaluate(()=>window.__test.glossaryPending.filter(x=>x.kind==='preview')[4].resolve());
  await page.locator('#apply-previewed-glossary').click();
  await page.waitForFunction(()=>window.__test.appliedGlossaries.length===1);
  assert.deepEqual(await page.evaluate(()=>window.__test.appliedGlossaries),[{jobId:'editor-test',glossaryId:'A'}]);
  // Reopening tools creates a new snapshot read even for the same task.
  await page.evaluate(()=>window.__test.holdSnapshot=true);
  const reopen=async()=>{await page.locator('#back-to-jobs').click();await page.getByRole('button',{name:'翻译与词表',exact:true}).click();};
  await reopen();await page.waitForFunction(()=>window.__test.snapshotPending.length===1);
  await reopen();await page.waitForFunction(()=>window.__test.snapshotPending.length===2);
  await page.evaluate(()=>window.__test.snapshotPending[1].resolve('当前任务快照'));
  await page.waitForFunction(()=>document.querySelector('#job-glossary-snapshot').textContent==='当前任务快照');
  await page.evaluate(()=>window.__test.snapshotPending[0].reject(Error('obsolete snapshot error')));
  await page.waitForTimeout(50);assert.equal(await page.locator('#job-glossary-snapshot').textContent(),'当前任务快照');
  assert.deepEqual(errors,[]);
  console.log('PASS: glossary preview selection, ABA invalidation, obsolete errors and latest rules inspection');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
