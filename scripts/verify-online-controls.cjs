// Production WebKit UI with synthetic IPC; no capture, models or user data.
const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:760,height:700}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await page.addInitScript(()=>{
   window.__onlineQA={calls:[],status:{sessionId:null,phase:'idle',error:null},events:[],visible:false,locked:false};
   window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'online-control'},currentWebview:{label:'online-control'}},transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(command,args={})=>{
    const q=window.__onlineQA;q.calls.push({command,args});
    if(command==='online_choices')return {languages:[{id:'ja',name:'日语',ready:true}],finals:[{id:'qwen-1.7b-8bit',name:'Qwen3-ASR 1.7B 8bit',ready:true},{id:'qwen-1.7b',name:'Qwen3-ASR 1.7B',ready:true},{id:'qwen-0.6b',name:'Qwen3-ASR 0.6B',ready:false}],runtimeReady:true,translationReady:true,translators:[{id:'hy-mt2',name:'Hy-MT2 1.8B',ready:true}]};
    if(command==='list_glossaries')return [{id:'ja',name:'日语词表',source_language:'ja'},{id:'en',name:'英语词表',source_language:'en'}];
    if(command==='online_snapshot')return {status:q.status,events:q.events,captionVisible:q.visible,captionLocked:q.locked};
    if(command==='start_online_session'){q.status={sessionId:'s',phase:'running',error:null,translationEnabled:args.options.translate};return q.status;}
    if(command==='stop_online_session'){q.status={...q.status,phase:'stopped'};return q.status;}
    if(command==='open_online_captions')q.visible=true;
    if(command==='hide_online_captions')q.visible=false;
    if(command==='toggle_online_captions')q.visible=!q.visible;
    if(command==='set_online_caption_locked')q.locked=args.locked;
    return 1;
   }};
   window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
  });
  const base=process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430';
  await page.goto(base+'/online.html');
  await page.locator('#online-start:enabled').waitFor();
  assert.equal(await page.locator('#online-settings').evaluate(e=>e.open),false);
  assert.equal(await page.locator('#online-start').isVisible(),true);
  assert.equal(await page.locator('#online-model').inputValue(),'qwen-1.7b-8bit');
  await page.locator('#online-settings summary').click();
  await page.locator('#online-model').selectOption('qwen-1.7b');
  await page.locator('#online-refresh').click();
  await page.evaluate(()=>new Promise(requestAnimationFrame));
  assert.equal(await page.locator('#online-model').inputValue(),'qwen-1.7b');
  await page.locator('#online-model').selectOption('qwen-1.7b-8bit');
  await page.locator('#online-translate').uncheck();
  assert.equal(await page.locator('#online-drafts').isDisabled(),true);
  await page.locator('#online-translate').check();
  assert.equal(await page.locator('#online-drafts').isEnabled(),true);
  await page.locator('#online-drafts').uncheck();
  await page.locator('#online-settings summary').click();
  await page.locator('#online-start').click();
  await page.locator('#online-stop:enabled').waitFor();
  const options=await page.evaluate(()=>window.__onlineQA.calls.find(c=>c.command==='start_online_session').args.options);
  assert.equal(options.finalModel,'qwen-1.7b-8bit');
  assert.equal(options.translate,true);assert.equal(options.translationDrafts,false);
  assert.equal(await page.locator('#online-language').isDisabled(),true);
  await page.evaluate(()=>{window.__onlineQA.events=[{sessionId:'s',sequence:1,event:{kind:'source_final',segment_id:'a',start_s:0,end_s:3,text:'已固定原文'}},{sessionId:'s',sequence:2,event:{kind:'translation_draft',group_id:'g',start_s:0,end_s:3,text:'可回改译文',source_revisions:{a:1}}}];});
  await page.waitForFunction(()=>document.getElementById('translation-state').textContent==='含可回改草稿');
  assert.equal(await page.locator('#source-state').textContent(),'已锁定');
  await page.evaluate(()=>window.__onlineQA.events.push({sessionId:'s',sequence:3,event:{kind:'translation_final',group_id:'g',start_s:0,end_s:3,text:'已固定译文',source_revisions:{a:1}}}));
  await page.waitForFunction(()=>document.getElementById('translation-state').textContent==='已锁定');
  assert.equal(await page.locator('#translation-text').textContent(),'已固定译文 ');
  await page.locator('#online-hide').click();
  assert.equal(await page.locator('#online-stop').isEnabled(),true,'hiding captions must not stop capture');
  await page.locator('#online-stop').click();await page.locator('#online-start:enabled').waitFor();
  await page.setViewportSize({width:440,height:700});
  await page.locator('#online-settings summary').click();
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
  await page.screenshot({path:'/tmp/atogaki-online-controls-small.png',fullPage:true});
  await page.locator('#online-settings summary').click();
  await page.setViewportSize({width:760,height:700});
  await page.screenshot({path:'/tmp/atogaki-online-controls.png',fullPage:true});
  await page.goto(base+'/online-captions.html');
  await page.waitForFunction(()=>document.getElementById('online-status').textContent==='未开始');
  await page.evaluate(()=>{window.__onlineQA.status={sessionId:'s',phase:'running',error:null};window.__onlineQA.events=[{sessionId:'s',sequence:1,event:{kind:'source_final',segment_id:'a',start_s:0,end_s:3,text:'字幕窗原文'}},{sessionId:'s',sequence:2,event:{kind:'translation_draft',group_id:'g',start_s:0,end_s:3,text:'字幕窗译文草稿',source_revisions:{a:1}}}];});
  await page.waitForFunction(()=>document.getElementById('translation-state').textContent==='含可回改草稿');
  await page.setViewportSize({width:900,height:260});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollHeight<=innerHeight&&document.documentElement.scrollWidth<=innerWidth),true);
  await page.screenshot({path:'/tmp/atogaki-online-caption.png'});
  assert.deepEqual(errors,[]);
  console.log('PASS: folded controls preserve pretranslation intent, disabled settings follow the session, caption states match displayed revisions, and small controls/fixed overlay fit.');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
