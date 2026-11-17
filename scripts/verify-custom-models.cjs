const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const p=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];p.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(p,30);
  await p.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke,qa=window.__customQA={models:JSON.parse(localStorage.getItem('__customFixtureModels')||'[]'),picks:0,release:null};
   const save=()=>localStorage.setItem('__customFixtureModels',JSON.stringify(qa.models));
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='registered_models')return structuredClone(qa.models);
    if(cmd==='pick_registered_model'){qa.picks++;return new Promise(resolve=>qa.release=resolve);}
    if(cmd==='register_model'){if(!args.name.trim())throw Error('名称不能为空');const model={id:'custom-'+(qa.models.length+1),...args,ready:true};qa.models.push(model);save();return model;}
    if(cmd==='rename_registered_model'){qa.models.find(m=>m.id===args.id).name=args.name;save();return;}
    if(cmd==='forget_registered_model'){qa.models=qa.models.filter(m=>m.id!==args.id);save();return;}
    if(cmd==='desktop_settings')return {onboardingCompleted:true,needsOnboarding:false,whisperModelPath:'/fixture-original.bin',vadModelPath:null,translationProviderId:'none',translationStyleInstruction:'',modelsDirectory:'/test/models',networkProxyMode:'environment',credentialStore:'fixture'};
    if(cmd==='offline_recognition_choices')return [{id:'configured-whisper',name:'Whisper Medium',path:'/fixture-original.bin',provider:'whisper.cpp',ready:true,reason:''},...qa.models.filter(m=>m.kind!=='vad').map(m=>({...m,provider:m.kind==='qwen-asr'?'mlx-qwen3-asr':'whisper.cpp',reason:''}))];
    return invoke(cmd,args);
   };
  });
  await p.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await p.locator('#open-settings').click();
  assert.equal(await p.locator('#settings-whisper-model').getAttribute('type'),'hidden');
  assert.equal(await p.locator('#settings-choose-whisper,#settings-choose-vad').count(),0);
  await p.locator('.custom-model-import summary').click();await p.locator('[data-custom-pick]').click();
  await p.locator('[data-custom-pick]').dispatchEvent('click');assert.equal(await p.evaluate(()=>window.__customQA.picks),1);
  await p.locator('[data-custom-path]').fill('/newer.bin');await p.evaluate(()=>window.__customQA.release('/stale.bin'));
  await p.locator('[data-custom-pick]:enabled').waitFor();assert.equal(await p.locator('[data-custom-path]').inputValue(),'/newer.bin');
  await p.locator('[data-custom-pick]').click();await p.locator('#close-settings').click();await p.locator('#open-settings').click();
  await p.evaluate(()=>window.__customQA.release('/old-visit.bin'));await p.locator('[data-custom-pick]:enabled').waitFor();
  assert.equal(await p.locator('[data-custom-path]').inputValue(),'/newer.bin');
  await p.locator('[data-custom-name]').fill('我的 Medium <local>');await p.locator('[data-custom-add]').click();
  await p.locator('[data-registered-model="custom-1"]').waitFor();assert.equal(await p.locator('[data-registered-model="custom-1"] strong').innerText(),'我的 Medium <local>');
  await p.locator('[data-registered-model="custom-1"] button').filter({hasText:'改名'}).click();await p.getByLabel('新的模型名称').fill('电台专用');await p.getByLabel('新的模型名称').press('Enter');
  await p.waitForFunction(()=>document.querySelector('[data-registered-model="custom-1"] strong')?.textContent==='电台专用');
  await p.locator('[data-custom-kind]').selectOption('vad');await p.locator('[data-custom-path]').fill('/vad.bin');await p.locator('[data-custom-name]').fill('我的人声检测');await p.locator('[data-custom-add]').click();await p.locator('[data-registered-model="custom-2"]').waitFor();
  await p.screenshot({path:'/tmp/atogaki-custom-models.png'});
  await p.locator('#close-settings').click();await p.locator('#recognition-model').selectOption('custom-1');
  await p.locator('#task-advanced > summary').click();await p.locator('#vad-model').selectOption('/vad.bin');
  await p.locator('#media-path').fill('/fixture.wav');await p.locator('#task-translation-provider').selectOption('none');await p.locator('#submit-task').click();
  await p.waitForFunction(()=>window.__test.calls.some(call=>call.cmd==='submit_transcription'));
  const frozen=await p.evaluate(()=>window.__test.calls.find(call=>call.cmd==='submit_transcription').args.request);
  assert.equal(frozen.recognitionModelId,'custom-1');assert.equal(frozen.vadModelPath,'/vad.bin');
  await p.reload();await p.waitForFunction(()=>document.querySelector('#recognition-model').value==='custom-1');
  assert.equal(await p.locator('#vad-model').inputValue(),'/vad.bin');

  await p.locator('#open-settings').click();await p.locator('[data-registered-model="custom-1"] button').filter({hasText:'移除配置'}).click();
  await p.waitForFunction(()=>!document.querySelector('[data-registered-model="custom-1"]'));
  await p.locator('#close-settings').click();await p.waitForFunction(()=>document.querySelector('#recognition-model').value==='configured-whisper');
  assert.equal(frozen.recognitionModelId,'custom-1','existing submitted request is not rewritten');
  assert.deepEqual(errors,[]);console.log('PASS: named model import/rename, safe labels, picker duplicate and stale visit protection, ASR/VAD selectors, submitted IDs and removal fallback.');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
