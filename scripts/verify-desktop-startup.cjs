const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  for(const mode of ['normal','dispose','presets','runtime']) {
   const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
   page.on('pageerror',e=>errors.push(String(e)));
   await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
   await page.addInitScript(mode=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    const qa=window.__startupQA={held:{},presetReads:0,holdRuntime:false};
    const preset=name=>[{name,configuration:{modelPath:'',vadModelPath:null,sourceLanguage:'ja',glossaryId:null,selectedContentGroups:[],translationProviderId:'none'}}];
    window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
     if(command==='task_presets')qa.presetReads++;
     const hold=mode==='dispose'&&['data_directory','list_glossaries','task_presets','list_job_summaries'].includes(command)||mode==='presets'&&command==='task_presets'&&qa.presetReads===1;
     const runtimeHeld=mode==='runtime'&&qa.holdRuntime&&command==='list_job_summaries';
     if(hold||runtimeHeld){qa.holdRuntime=false;await new Promise(resolve=>qa.held[command]=resolve);}
     const result=await invoke(command,args);
     if(runtimeHeld)return result.map(job=>({...job,display_name:'迟到任务更名'}));
     if(command==='task_presets'&&mode==='presets')return preset(hold?'旧预设':'新预设');
     if(command==='task_presets'&&mode==='dispose')return preset('迟到预设');
     return result;
    };
   },mode);
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
   if(mode==='normal') {
    await page.locator('.job-open').first().waitFor();await page.waitForFunction(()=>document.querySelector('#data-path').textContent.includes('/test/isolated'));
    const counts=await page.evaluate(()=>Object.fromEntries(['data_directory','list_glossaries','task_presets','list_job_summaries'].map(cmd=>[cmd,window.__test.calls.filter(call=>call.cmd===cmd).length])));
    assert.deepEqual(counts,{data_directory:1,list_glossaries:1,task_presets:1,list_job_summaries:1});
   } else if(mode==='dispose') {
    await page.waitForFunction(()=>Object.keys(window.__startupQA.held).length===4);
    const selectors=['#data-path','#job-list','#task-glossary','[aria-label="任务预设"]'];
    const before=await page.evaluate(selectors=>selectors.map(selector=>document.querySelector(selector).innerHTML),selectors);
    await page.evaluate(()=>{window.dispatchEvent(new Event('pagehide'));Object.values(window.__startupQA.held).forEach(resolve=>resolve());});
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const after=await page.evaluate(selectors=>selectors.map(selector=>document.querySelector(selector).innerHTML),selectors);
    assert.deepEqual(after,before,'late startup reads must not update a disposed page');
   } else if(mode==='runtime') {
    await page.locator('.job-open').first().waitFor();await page.evaluate(()=>window.__startupQA.holdRuntime=true);
    await page.waitForFunction(()=>window.__startupQA.held.list_job_summaries);
    const before=await page.locator('#job-list').innerHTML();
    await page.evaluate(()=>{window.dispatchEvent(new Event('pagehide'));window.__startupQA.held.list_job_summaries();});
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    assert.equal(await page.locator('#job-list').innerHTML(),before);
   } else {
    await page.waitForFunction(()=>window.__startupQA.held.task_presets);
    await page.getByRole('textbox',{name:'预设名称',exact:true}).fill('新预设');await page.getByRole('button',{name:'保存当前配置为预设',exact:true}).click();
    await page.waitForFunction(()=>document.querySelector('[aria-label="任务预设"]').textContent.includes('新预设'));
    await page.evaluate(()=>window.__startupQA.held.task_presets());
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const presets=await page.locator('[aria-label="任务预设"]').textContent();assert.match(presets,/新预设/);assert.doesNotMatch(presets,/旧预设/);
   }
   assert.deepEqual(errors,[]);await page.close();
  }
  console.log('PASS: initial reads start once, disposed-page directory/tasks/glossaries/presets remain unchanged, late runtime refresh cannot render, and stale startup presets cannot replace a newer save');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
