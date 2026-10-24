const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   const state=(modelId,status)=>({modelId,status,downloadedBytes:1,totalBytes:2,path:status==='done'?'/installed-'+modelId:null,error:null,source:'fixture',active:false});
   const settings={onboardingCompleted:true,needsOnboarding:false,whisperModelPath:'/fixture-current',vadModelPath:null,hyMt2ModelPath:null,translationProviderId:'none',translationStyleInstruction:'fixture',modelsDirectory:'/fixture-models',networkProxyMode:'direct',networkProxyUrl:null,modelMirrorUrl:null,credentialStore:'fixture'};
   window.__models={states:[state('b','done')],settings,calls:[],reads:0,holdNetwork:true,holdSelect:true,releaseNetwork:null,releaseSelect:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const data=window.__models;data.calls.push({cmd,args});
    if(cmd==='desktop_settings')return structuredClone(data.settings);
    if(cmd==='model_catalog')return ['a','b'].map(id=>({id,kind:'whisper',name:'Fixture model '+id,fileName:id+'.bin',sizeLabel:'1 KiB',recommendedFor:'fixture',sourceUrl:'https://example.invalid/model'}));
    if(cmd==='model_download_states'){data.reads++;return structuredClone(data.states);}
    if(cmd==='save_download_network_settings'){
     const request=structuredClone(args.request),commit=()=>{data.settings.networkProxyMode=request.proxyMode;data.settings.networkProxyUrl=request.proxyUrl;data.settings.modelMirrorUrl=request.modelMirrorUrl;};
     if(data.holdNetwork)return new Promise(resolve=>data.releaseNetwork=()=>{commit();resolve();});commit();return;
    }
    if(cmd==='start_model_download'){const value=state(args.modelId,'queued');data.states=data.states.filter(item=>item.modelId!==args.modelId).concat(value);return value;}
    if(cmd==='select_model'){
     const commit=()=>{data.states=data.states.map(item=>({...item,active:item.modelId===args.modelId}));data.settings.whisperModelPath='/installed-'+args.modelId;return data.states.find(item=>item.modelId===args.modelId);};
     return data.holdSelect?new Promise(resolve=>data.releaseSelect=()=>resolve(commit())):commit();
    }
    if(cmd==='uninstall_model'){data.states=data.states.filter(item=>item.modelId!==args.modelId);return;}
    if(cmd==='cancel_model_download'){const value=state(args.modelId,'cancelled');data.states=data.states.filter(item=>item.modelId!==args.modelId).concat(value);return value;}
    return invoke(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('#open-settings').click();await page.locator('[data-settings-tab="network"]').click();await page.locator('#settings-model-mirror').fill('https://first.invalid');await page.locator('[data-settings-tab="models"]').click();
  await page.locator('[data-model-action="download"][data-model-id="a"]').click();assert.equal(await page.locator('[data-model-action="select"][data-model-id="b"]').isDisabled(),true);
  await page.locator('[data-settings-tab="network"]').click();await page.locator('#settings-model-mirror').fill('https://second.invalid');await page.evaluate(()=>window.__models.releaseNetwork());await page.waitForFunction(()=>document.querySelector('#model-download-message').textContent.includes('新的网络输入尚未保存'));
  assert.equal(await page.evaluate(()=>window.__models.calls.find(c=>c.cmd==='save_download_network_settings').args.request.modelMirrorUrl),'https://first.invalid');
  await page.evaluate(()=>{const data=window.__models;data.states=data.states.map(item=>item.modelId==='a'?{...item,status:'done',path:'/installed-a',active:true}:item);data.settings.whisperModelPath='/installed-a';});await page.waitForFunction(()=>document.querySelector('#model-download-message').textContent.includes('模型已校验并安装'));assert.equal(await page.locator('#settings-model-mirror').inputValue(),'https://second.invalid');
  await page.locator('[data-settings-tab="models"]').click();await page.locator('[data-model-action="select"][data-model-id="b"]').click();await page.locator('[data-settings-tab="transcription"]').click();await page.locator('#settings-whisper-model').fill('/typed-during-select');await page.evaluate(()=>window.__models.releaseSelect());await page.waitForFunction(()=>document.querySelector('#model-download-message').textContent.includes('新的路径输入尚未保存'));assert.equal(await page.locator('#settings-whisper-model').inputValue(),'/typed-during-select');
  await page.locator('[data-settings-tab="models"]').click();await page.locator('[data-model-action="uninstall"][data-model-id="a"]').click();await page.waitForFunction(()=>document.querySelector('#model-download-message').textContent.includes('模型及其未完成下载已删除'));assert.equal(await page.evaluate(()=>window.__models.calls.filter(c=>c.cmd==='uninstall_model').length),1);
  await page.evaluate(()=>window.__models.holdNetwork=false);await page.locator('[data-model-action="download"][data-model-id="a"]').click();await page.waitForFunction(()=>document.querySelector('[data-model-action="cancel"][data-model-id="a"]')!==null);await page.locator('#close-settings').click();await page.evaluate(()=>new Promise(requestAnimationFrame));const reads=await page.evaluate(()=>window.__models.reads);await page.waitForTimeout(1100);assert.equal(await page.evaluate(()=>window.__models.reads),reads);
  await page.locator('#open-settings').click();await page.locator('[data-settings-tab="models"]').click();await page.locator('[data-model-action="cancel"][data-model-id="a"]').click();await page.waitForFunction(()=>window.__models.states.find(item=>item.modelId==='a')?.status==='cancelled');assert.equal(await page.evaluate(()=>window.__models.calls.filter(c=>c.cmd==='cancel_model_download').length),1);await page.waitForFunction(()=>{const button=document.querySelector('[data-model-action="download"][data-model-id="a"]');return button&&!button.disabled;});
  await page.evaluate(()=>{window.__models.holdNetwork=true;window.__models.releaseNetwork=null;});const starts=await page.evaluate(()=>window.__models.calls.filter(c=>c.cmd==='start_model_download').length);await page.locator('[data-model-action="download"][data-model-id="a"]').click();await page.waitForFunction(()=>typeof window.__models.releaseNetwork==='function');await page.locator('#close-settings').click();await page.evaluate(()=>window.__models.releaseNetwork());await page.waitForFunction(starts=>window.__models.calls.filter(c=>c.cmd==='start_model_download').length===starts+1,starts);const closedReads=await page.evaluate(()=>window.__models.reads);await page.waitForTimeout(1100);assert.equal(await page.evaluate(()=>window.__models.reads),closedReads);assert.deepEqual(errors,[]);
  console.log('PASS: model network/path draft protection, selection/uninstall/cancel, visible serial polling and no hidden reads; mock model operations only');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
