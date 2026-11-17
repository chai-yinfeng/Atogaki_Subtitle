const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{const browser=await webkit.launch({headless:true});try{
 const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',error=>errors.push(String(error)));
 await require('./editor-ui-fixture.cjs').installEditorFixture(page);
 await page.addInitScript(()=>{const invoke=window.__TAURI_INTERNALS__.invoke;window.__offline={submitted:[]};window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
  if(cmd==='desktop_settings')return {onboardingCompleted:true,needsOnboarding:false,whisperModelPath:'/qa/medium.bin',whisperModelReady:true,vadModelPath:'/qa/vad.bin',vadModelReady:true,qwenOfflineReady:true,translationProviderId:'none',translationStyleInstruction:'',modelsDirectory:'/qa/models',networkProxyMode:'environment',credentialStore:'fixture'};
  if(cmd==='offline_recognition_choices')return [{id:'configured-whisper',name:'Whisper Medium',path:'/qa/medium.bin',provider:'whisper.cpp',ready:true,reason:''},{id:'qwen-1.7b-8bit',name:'Qwen3-ASR 1.7B 8-bit',path:'/qa/qwen',provider:'mlx-qwen3-asr',ready:true,reason:''},{id:'qwen-0.6b',name:'Qwen 0.6B',path:'/qa/qwen-small',provider:'mlx-qwen3-asr',ready:false,reason:'需下载字幕对齐模型'}];
  if(cmd==='submit_transcription'){window.__offline.submitted.push(args.request);return 'fixture';}
  return invoke(cmd,args);
 };});
 await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
 const model=page.locator('#recognition-model');await page.waitForFunction(()=>document.querySelector('#recognition-model').options.length===3);
 assert.equal(await model.isVisible(),true);assert.equal(await page.locator('#task-advanced').evaluate(e=>e.open),false);
 assert.equal(await page.locator('#model-path').isVisible(),false);assert.equal(await page.locator('#choose-model').count(),0);
 assert.equal(await model.locator('[value="qwen-0.6b"]').evaluate(e=>e.disabled),true);
 await page.locator('#task-advanced > summary').click();assert.equal(await page.locator('#vad-model').isVisible(),true);
 await model.selectOption('qwen-1.7b-8bit');assert.equal(await page.locator('#vad-enabled').isDisabled(),true);assert.equal(await page.locator('#vad-model').isVisible(),false);
 await page.locator('#media-path').fill('/qa/input.wav');await page.locator('#task-translation-provider').selectOption('none');await page.locator('#submit-task').click();
 await page.waitForFunction(()=>window.__offline.submitted.length===1);
 const submitted=await page.evaluate(()=>window.__offline.submitted[0]);assert.equal(submitted.recognitionModelId,'qwen-1.7b-8bit');assert.equal(submitted.vadModelPath,null);assert.equal(submitted.modelPath,'/qa/qwen');
 await page.reload();await page.waitForFunction(()=>document.querySelector('#recognition-model').value==='qwen-1.7b-8bit');
 assert.match(await page.locator('#task-pipeline-summary').innerText(),/Qwen3-ASR/);
 await model.selectOption('configured-whisper');await page.locator('#task-advanced > summary').click();assert.equal(await page.locator('#vad-enabled').isDisabled(),false);assert.equal(await page.locator('#vad-model').isVisible(),true);
 await page.screenshot({path:'/tmp/atogaki-offline-models-20261117.png'});assert.deepEqual(errors,[]);
 console.log('PASS: visible model selector, hidden paths, unavailable Qwen guard, engine-specific VAD, Qwen wire ID, persisted selection and Whisper return');
}finally{await browser.close();}})().catch(error=>{console.error(error);process.exitCode=1});
