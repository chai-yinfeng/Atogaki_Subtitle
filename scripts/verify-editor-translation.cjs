const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;window.__translation={fail:false,hold:false,resolve:null,cloudUsage:false};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='translation_status')return {provider_id:'fixture',provider:'隔离翻译',configured:true,model:'fixture',endpoint_kind:'test'};
    if(cmd==='get_job_detail'){
     const detail=structuredClone(await invoke(cmd,args)),cloud=window.__translation.cloudUsage;
     detail.translation_runs=[{id:'usage',provider_id:'fixture',provider_name:'隔离翻译',model:'fixture',endpoint_kind:'test',segment_count:1,input_tokens:cloud?120:0,output_tokens:cloud?18:0,completed_at_unix:1,plan_json:JSON.stringify({execution_usage:cloud?{cached_input_tokens:96,uncached_input_tokens:24}:{local_cache_hit:true}})}];
     return detail;
    }
    if(!['translate_subtitle','translate_all_subtitles'].includes(cmd))return structuredClone(await invoke(cmd,args));
    window.__test.calls.push({cmd,args});
    if(cmd==='translate_subtitle'){
     if(window.__translation.hold)return new Promise(resolve=>{window.__translation.resolve=resolve;});
     const cue=window.__test.cues.find(c=>c.id===args.segmentId);Object.assign(cue,{translated_text:'隔离翻译结果',translation_stale:false});return {...cue};
    }
    Object.assign(window.__test.cues[0],{translated_text:'已保留的成功批次',translation_stale:false});
    if(window.__translation.fail)throw Error('fixture provider stopped');
    for(const cue of window.__test.cues)Object.assign(cue,{translated_text:'完整批次译文',translation_stale:false});
    return {segments:window.__test.cues.map(c=>({...c})),requested_count:30,translated_count:30,empty_segment_ids:[],resumed:args.onlyPending};
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  const row=page.locator('#editor-cue-list [data-cue-id]').first();await row.click();
  const more=page.locator('.editor-cue-toolbar details summary'),single=page.locator('[data-editor-action="translate"]');
  await more.click();await single.click();await page.waitForFunction(()=>document.querySelector('#karaoke-current-translation').value==='隔离翻译结果');
  assert.deepEqual(await page.evaluate(()=>window.__test.calls.find(r=>r.cmd==='translate_subtitle').args),{jobId:'editor-test',segmentId:'cue-0'});
  // Draft flush is the existing save queue, before the translation command.
  await page.locator('#karaoke-current-source').fill('待保存后翻译的原文');await more.click();await single.click();
  await page.waitForFunction(()=>window.__test.calls.filter(r=>r.cmd==='translate_subtitle').length===2);
  const sequence=await page.evaluate(()=>window.__test.calls.filter(r=>['update_subtitle','translate_subtitle'].includes(r.cmd)).map(r=>r.cmd));assert.deepEqual(sequence.slice(-2),['update_subtitle','translate_subtitle']);
  await page.locator('#karaoke-open-workspace').click();
  assert.match(await page.locator('#translation-run-status').innerText(),/本地缓存复用（本次无新增 API 消耗）/);
  await page.locator('#translate-all').click();await page.locator('#confirmation-dialog').waitFor();await page.locator('#cancel-confirmation').click();assert.equal(await page.evaluate(()=>window.__test.calls.filter(r=>r.cmd==='translate_all_subtitles').length),0);
  await page.evaluate(()=>{window.__translation.fail=true;window.__translation.cloudUsage=true;});await page.locator('#translate-pending').click();await page.locator('#accept-confirmation').click();
  await page.waitForFunction(()=>document.querySelector('#workspace-action-message').textContent.includes('之前成功的批次已经保留'));
  assert.match(await page.locator('#workspace-action-message').innerText(),/fixture provider stopped/);assert.equal(await page.locator('#translate-pending').isEnabled(),true);
  assert.match(await page.locator('#translation-run-status').innerText(),/本次 Token 120 入 \/ 18 出 · 云端输入缓存 96 命中 \/ 24 未命中/);
  await page.evaluate(()=>window.__translation.fail=false);await page.locator('#translate-pending').click();await page.locator('#accept-confirmation').click();
  await page.waitForFunction(()=>document.querySelector('#workspace-action-message').textContent.includes('续译完成：30'));
  assert.equal(await page.locator('#translate-pending').isDisabled(),true);assert.equal(await page.locator('#translate-all').isEnabled(),true);
  await page.locator('#back-to-jobs').click();assert.equal(await page.locator('#karaoke-current-source').inputValue(),'待保存后翻译的原文');
  assert.equal(await page.locator('#karaoke-current-translation').inputValue(),'完整批次译文');
  await page.evaluate(()=>window.__translation.hold=true);await more.click();await single.click();
  await page.waitForFunction(()=>typeof window.__translation.resolve==='function');
  await page.locator('[data-editor-action="back"]').click();await page.locator('.job-open').first().click();
  const beforeLate=await page.locator('#karaoke-current-translation').inputValue();
  await page.evaluate(()=>window.__translation.resolve({...window.__test.cues[0],translated_text:'迟到的旧视图译文'}));
  await page.waitForFunction(()=>!document.querySelector('[data-editor-action="translate"]').disabled);
  assert.equal(await page.locator('#karaoke-current-translation').inputValue(),beforeLate);assert.deepEqual(errors,[]);
  console.log('PASS: single translation, draft flush order, cancel without request, partial persistence refresh, retry, return to same cue and late same-task navigation isolation');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
