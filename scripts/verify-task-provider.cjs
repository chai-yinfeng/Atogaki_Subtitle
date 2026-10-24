const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__provider={values:{'editor-test':'deepseek',other:'deepl'},writes:[],hold:false,fail:false,resolve:null,completed:0};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='translation_status'){
     const id=window.__provider.values[args.jobId]||'none';return {provider_id:id,provider:id,configured:id!=='none',model:null,endpoint_kind:'fixture',configuration_hint:null};
    }
    if(cmd==='set_task_translation'){
     window.__provider.writes.push(args);
     if(window.__provider.fail)throw Error('fixture selection rejected');
     const commit=()=>{window.__provider.values[args.jobId]=args.providerId;window.__provider.completed++;};
     if(window.__provider.hold)return new Promise(resolve=>{window.__provider.resolve=()=>{commit();resolve();};});
     commit();return;
    }
    const result=structuredClone(await invoke(cmd,args));
    if(cmd==='list_jobs'||cmd==='list_job_summaries')return [...result,{...result[0],job_id:'other',display_name:'另一任务 B'}];
    if(cmd==='get_job_detail'&&args.jobId==='other')return {...result,job:{...result.job,job_id:'other',display_name:'另一任务 B'},segments:result.segments.map(c=>({...c,id:'other-'+c.id,job_id:'other'}))};
    return result;
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();await page.locator('#karaoke-open-workspace').click();
  const choice=page.locator('#task-provider-choice'),apply=page.locator('[data-apply-task-provider]'),notice=page.locator('[data-task-provider-message]');
  await page.waitForFunction(()=>document.querySelector('#task-provider-choice').value==='deepseek');
  await choice.selectOption('gemini');await page.evaluate(()=>window.__provider.hold=true);await apply.focus();await apply.click();assert.equal(await apply.isDisabled(),true);
  await choice.focus();await choice.selectOption('deepl');await page.evaluate(()=>window.__provider.resolve());await page.waitForFunction(()=>!document.querySelector('[data-apply-task-provider]').disabled);
  assert.equal(await choice.inputValue(),'deepl');assert.equal(await choice.evaluate(e=>document.activeElement===e),true);assert.deepEqual(await page.evaluate(()=>window.__provider.writes),[{jobId:'editor-test',providerId:'gemini'}]);
  assert.match(await notice.innerText(),/本任务后端已更新/);assert.match(await page.locator('#translation-status').innerText(),/gemini/);
  await page.evaluate(()=>{window.__provider.hold=false;window.__provider.fail=true;});await choice.selectOption('openai-compatible');await apply.click();
  await page.waitForFunction(()=>document.querySelector('[data-task-provider-message]').textContent.includes('设置任务后端失败'));
  assert.equal(await choice.inputValue(),'openai-compatible');assert.equal(await apply.isEnabled(),true);
  await page.evaluate(()=>{window.__provider.fail=false;window.__provider.hold=true;window.__provider.resolve=null;});await choice.selectOption('gemini');await apply.click();
  await page.waitForFunction(()=>typeof window.__provider.resolve==='function');await page.locator('#back-to-jobs').click();await page.locator('[data-editor-action="back"]').click();await page.locator('.job-open').nth(1).click();await page.locator('#karaoke-open-workspace').click();
  await page.waitForFunction(()=>document.querySelector('#task-provider-choice').value==='deepl');await page.evaluate(()=>window.__provider.resolve());await page.waitForFunction(()=>window.__provider.completed===2);
  assert.equal(await choice.inputValue(),'deepl');assert.equal(await notice.innerText(),'');assert.match(await page.locator('#translation-status').innerText(),/deepl/);
  assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>['translate_subtitle','translate_all_subtitles'].includes(c.cmd))),false);assert.deepEqual(errors,[]);
  console.log('PASS: task provider selection, busy/draft preservation, focus, failure retry, current-task isolation and no implicit translation');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
