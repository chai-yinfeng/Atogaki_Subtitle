const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),{stripTypeScriptTypes}=require('node:module');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke,state=window.__relink={calls:[],path:null,resolve:null,reject:null,holdRead:false,readRelease:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    state.calls.push({cmd,args});
    if(cmd==='relink_job_media')return new Promise((resolve,reject)=>{state.resolve=path=>{if(path!==null)state.path=path;resolve(path===null?null:{...window.__test.job,input_path:path});};state.reject=reject;});
    if(cmd==='list_jobs'||cmd==='list_job_summaries')return [{...window.__test.job,input_path:state.path||window.__test.job.input_path},{...window.__test.job,job_id:'job-b',display_name:'Task B',input_path:'/fixture/b.wav'}];
    if(cmd==='get_job_detail'){
     const detail=await invoke(cmd,args);
     if(args.jobId==='job-b')return {...detail,job:{...detail.job,job_id:'job-b',display_name:'Task B',input_path:'/fixture/b.wav'},segments:detail.segments.map(c=>({...c,id:'b-'+c.id,job_id:'job-b',source_text:'B '+c.source_text}))};
     const result={...detail,job:{...detail.job,input_path:state.path||detail.job.input_path},playback_path:state.path?'/fixture.wav':null,audio_fallback_path:'/fixture.wav'};
     if(state.holdRead)return new Promise(resolve=>state.readRelease=()=>resolve(result));return result;
    }
    return invoke(cmd,args);
   };
  });
  const button=page.locator('#relink-job-media'),start=page.locator('#editor-cue-start'),source=page.locator('#karaoke-current-source'),notice=page.locator('#karaoke-timing-message');
  const count=()=>page.evaluate(()=>window.__relink.calls.filter(c=>c.cmd==='relink_job_media').length);
  const finish=path=>page.evaluate(path=>window.__relink.resolve(path),path);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();await button.waitFor();await page.locator('[data-cue-id="cue-1"]').click();assert.match(await source.inputValue(),/第 2 句/);
  await start.fill('00:00:03.100');await button.click();assert.equal(await count(),0);assert.match(await notice.innerText(),/文字和时间修改/);await start.press('Escape');
  await button.focus();await button.press('Enter');await button.dispatchEvent('click');assert.equal(await count(),1);await finish(null);await page.waitForFunction(()=>!document.querySelector('#relink-job-media').disabled);assert.match(await notice.innerText(),/已取消/);assert.equal(await button.evaluate(e=>document.activeElement===e),true);
  await button.click();await page.evaluate(()=>window.__relink.reject(Error('fixture wrong media')));await page.waitForFunction(()=>!document.querySelector('#relink-job-media').disabled);assert.match(await notice.innerText(),/重新定位原媒体失败/);
  await button.click();await start.fill('00:00:03.200');await finish('/fixture/saved-with-new-input.mp4');await page.waitForFunction(()=>!document.querySelector('#relink-job-media').disabled);assert.match(await notice.innerText(),/路径已保存；保留当前新修改/);assert.equal(await start.inputValue(),'00:00:03.200');await start.press('Escape');
  // Once the navigation lock is released for reload, moving to B must invalidate A's refresh.
  await page.evaluate(()=>window.__relink.holdRead=true);await button.click();await finish('/fixture/old-a.mp4');await page.waitForFunction(()=>window.__relink.readRelease);assert.equal(await button.isDisabled(),true);await page.locator('[data-editor-action="back"]').click();await page.locator('.job-open').nth(1).click();await page.waitForFunction(()=>document.querySelector('#karaoke-job-title').textContent==='Task B');await page.evaluate(()=>window.__relink.readRelease());await page.evaluate(()=>new Promise(requestAnimationFrame));assert.match(await source.inputValue(),/^B /);assert.doesNotMatch(await notice.innerText(),/old-a.mp4/);
  await page.locator('[data-editor-action="back"]').click();await page.evaluate(()=>{window.__relink.holdRead=false;window.__relink.path=null;});await page.locator('.job-open').first().click();await button.waitFor();await page.locator('[data-cue-id="cue-1"]').click();await button.click();await finish('/fixture/final.mp4');await page.waitForFunction(()=>document.querySelector('#relink-job-media').classList.contains('hidden'));await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('final.mp4'));assert.match(await source.inputValue(),/第 2 句/);assert.equal(await page.evaluate(()=>window.__relink.calls.filter(c=>c.cmd==='relink_job_media').every(c=>c.args.jobId==='editor-test')),true);
  const isolated=await browser.newPage();isolated.on('pageerror',e=>errors.push(String(e)));await isolated.setContent('<button id="relink">Relink</button>');
  await isolated.evaluate(async code=>{const url=URL.createObjectURL(new Blob([code],{type:'text/javascript'}));const {mountMediaRelinkView}=await import(url);URL.revokeObjectURL(url);let dead=false;const state=window.__view={calls:0,release:null};state.view=mountMediaRelinkView(document.querySelector('button'),{run:()=>{state.calls++;return new Promise(resolve=>state.release=resolve);}},()=>{if(dead)throw Error('released context');return 1;});state.dispose=()=>{state.view.dispose();dead=true;};},stripTypeScriptTypes(fs.readFileSync('ui/src/media-relink-view.ts','utf8')));await isolated.locator('button').focus();await isolated.locator('button').press('Enter');await isolated.evaluate(()=>{window.__view.dispose();window.__view.release();});await isolated.locator('button').click();assert.equal(await isolated.evaluate(()=>window.__view.calls),1);assert.deepEqual(errors,[]);
  console.log('PASS: media relink task ID, dirty timing and new-input protection, cancel/focus, duplicate/error/retry, superseded reload to B, selected cue preservation and view disposal; fixture paths only');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
