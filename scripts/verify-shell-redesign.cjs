// Production WebKit, synthetic IPC/audio: no user files, capture or paid API.
const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,60);
  await page.addInitScript(()=>{
   localStorage.setItem('atogaki.appearance.theme','letter');
   const invoke=window.__TAURI_INTERNALS__.invoke;
   const q=window.__onlineShell={status:{sessionId:null,phase:'idle',error:null},events:[],visible:false};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='online_choices')return {languages:[{id:'ja',name:'日语',ready:true}],drafts:[{id:'sensevoice',language:'ja',name:'SenseVoice',ready:true}],accelerationReady:true,finals:[{id:'qwen-1.7b-8bit',name:'Qwen3-ASR 1.7B 8-bit',ready:true}],runtimeReady:true,translationReady:true,translators:[{id:'hy-mt2',name:'Hy-MT2',ready:true}]};
    if(cmd==='online_snapshot')return {status:q.status,events:q.events,captionVisible:q.visible,captionLocked:false};
    if(cmd==='start_online_session'){q.status={sessionId:'online',phase:'running',error:null,translationEnabled:true};return q.status;}
    if(cmd==='stop_online_session'){q.status={...q.status,phase:'stopped'};return q.status;}
    if(cmd==='open_online_captions'||cmd==='toggle_online_captions'){q.visible=!q.visible;return;}
    if(cmd==='hide_online_captions'){q.visible=false;return;}
    const result=await invoke(cmd,args);
    if(cmd==='list_job_summaries')return result.map(job=>({...job,translation_status:'translated',translated_segment_count:job.segment_count}));
    return result;
   };
  });
  const base=process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430';
  await page.goto(base);await page.waitForFunction(()=>document.querySelectorAll('.job-open').length>0);
  assert.equal(await page.locator('#recent-jobs-drawer').evaluate(e=>e.open),false);
  assert.equal(await page.locator('.job-open').first().isVisible(),false);
  assert.equal(await page.locator('#refresh').count(),0);
  assert.equal(await page.locator('html').getAttribute('data-theme'),'neutral');
  await page.screenshot({path:'/tmp/atogaki-shell-workbench.png'});
  await page.locator('#open-recent-jobs').click();
  assert.equal(await page.locator('#recent-jobs-drawer').evaluate(e=>e.open),true);
  const search=await page.locator('#job-title-search').boundingBox(),list=await page.locator('#job-list').boundingBox();
  assert.ok(list.y>=search.y+search.height+12);
  await page.locator('#job-title-search').fill('不存在的任务');assert.equal(await page.locator('.job-open').count(),0);
  await page.locator('#job-title-search').fill('');await page.screenshot({path:'/tmp/atogaki-shell-tasks.png'});
  await page.keyboard.press('Escape');assert.equal(await page.locator('#recent-jobs-drawer').evaluate(e=>e.open),false);
  assert.equal(await page.locator('#open-recent-jobs').evaluate(e=>e===document.activeElement),true);
  await page.locator('#show-listening').click();await page.locator('.listening-job').first().click();
  await page.locator('#listening-subtitle-list .listening-subtitle').first().waitFor();
  for(const size of [{width:1180,height:760},{width:960,height:640},{width:1440,height:900}]){
   await page.setViewportSize(size);
   const geometry=await page.evaluate(()=>{
    const rect=id=>{const r=document.querySelector(id).getBoundingClientRect();return {left:r.left,top:r.top,right:r.right,bottom:r.bottom,width:r.width,height:r.height};};
    return {library:rect('.listening-library'),player:rect('.listening-player'),reading:rect('.listening-reading'),playback:rect('.listening-playback'),overflow:document.documentElement.scrollWidth>innerWidth};
   });
   assert.equal(geometry.overflow,false,JSON.stringify(geometry));
   assert.ok(geometry.player.left>geometry.library.left,JSON.stringify(geometry));
   assert.ok(geometry.player.width>400&&geometry.player.bottom<=size.height+1,JSON.stringify(geometry));
   assert.ok(geometry.reading.width>150&&geometry.reading.height>150&&geometry.reading.right<=size.width+1,JSON.stringify(geometry));
   await page.screenshot({path:'/tmp/atogaki-shell-listening-'+size.width+'.png'});
  }
  await page.locator('#listening-layout-placement').selectOption('bottom');
  const bottom=await page.locator('.listening-reading').boundingBox(),play=await page.locator('.listening-playback').boundingBox();assert.ok(bottom.y>play.y+play.height);
  await page.locator('#listening-layout-placement').selectOption('right');
  await page.setViewportSize({width:1180,height:760});await page.locator('#open-online').click();await page.locator('#online-start:enabled').waitFor();
  assert.equal(await page.locator('#online-view').isVisible(),true);
  assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>c.cmd==='open_online_control')),false);
  await page.locator('#online-start').click();await page.locator('#online-stop:enabled').waitFor();
  await page.locator('#show-workbench').click();assert.equal(await page.locator('#online-running-indicator').isVisible(),true);
  assert.equal(await page.evaluate(()=>window.__onlineShell.status.phase),'running');
  await page.locator('#open-online').click();await page.locator('#online-stop:enabled').waitFor();
  await page.evaluate(()=>window.__onlineShell.events=[{sessionId:'online',sequence:1,event:{kind:'source_final',segment_id:'a',start_s:0,end_s:3,text:'确定的原文'}},{sessionId:'online',sequence:2,event:{kind:'translation_final',group_id:'A',start_s:0,end_s:3,text:'确定的译文',source_revisions:{a:1}}}]);
  await page.waitForFunction(()=>document.querySelector('#translation-text').textContent.includes('确定的译文'));
  await page.screenshot({path:'/tmp/atogaki-shell-online.png'});
  await page.locator('#online-stop').click();await page.locator('#online-start:enabled').waitFor();
  await page.locator('#show-workbench').click();await page.locator('#open-recent-jobs').click();await page.locator('.job-open').first().click();
  await page.locator('#karaoke-view:not(.hidden)').waitFor();
  assert.equal(await page.locator('#recent-jobs-drawer').evaluate(e=>e.open),false);
  assert.ok((await page.locator('#karaoke-view').boundingBox()).height>500);
  await page.screenshot({path:'/tmp/atogaki-shell-editor.png'});
  assert.deepEqual(errors,[]);
  console.log('PASS: neutral migration, full task page, accessible hidden task sheet, search spacing, listening at 960/1180/1440, embedded online lifecycle and editor entry.');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
