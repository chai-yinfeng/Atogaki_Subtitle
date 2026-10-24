const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const {stripTypeScriptTypes}=require('node:module');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__cards={calls:[],hold:null,release:null,fail:false};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const state=window.__cards;state.calls.push({cmd,args});
    if(cmd==='model_catalog')return [{id:'local',kind:'hy-mt2',name:'Local <fixture>',fileName:'local.gguf',sizeLabel:'1 KiB',recommendedFor:'fixture',sourceUrl:'https://example.invalid/model'}];
    if(cmd==='model_download_states')return [{modelId:'local',status:'done',path:'/fixture/local.gguf',downloadedBytes:1,totalBytes:1,active:false}];
    if(cmd==='translation_profiles')return ['deepseek','gemini','deepl','hy-mt2-local','openai-compatible'].map(provider_id=>({provider_id,model:'fixture-model',base_url:'https://example.invalid',style:'fixture-style',readiness:provider_id==='hy-mt2-local'?'unverified':'unconfigured'}));
    if(['save_translation_credential','check_translation_api_key','test_translation_connection'].includes(cmd)){
     if(state.fail)throw Error('fixture credential write failed');
     const result=cmd==='check_translation_api_key'?{providerId:args.providerId,providerName:'Mock '+args.providerId,storedInSystem:true,availableFromEnvironment:false,credentialStore:'fixture'}:null;
     if(state.hold===cmd)return new Promise(resolve=>state.release=()=>resolve(result));return result;
    }
    if(cmd==='asr_credential_status')return {providerId:'gemini-transcribe',providerName:'Gemini',configured:true,credentialStore:'fixture'};
    return invoke(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('#open-settings').click();await page.locator('[data-settings-tab="translation"]').click();
  const configure=id=>page.locator(`[data-provider-card="${id}"] [data-configure-provider]`).click();
  await configure('deepseek');await page.locator('#settings-provider-model').fill('draft-a');await page.locator('#settings-api-key').fill('fixture-only-a');
  await page.evaluate(()=>window.__editorNode=document.querySelector('.provider-editor'));
  await configure('gemini');assert.equal(await page.locator('#settings-api-key').inputValue(),'');await page.locator('#settings-provider-model').fill('draft-b');await configure('deepseek');assert.equal(await page.locator('#settings-provider-model').inputValue(),'draft-a');assert.equal(await page.evaluate(()=>window.__editorNode===document.querySelector('.provider-editor')),true);
  await page.locator('[data-translation-settings] .provider-cards > details > summary').click();await configure('openai-compatible');assert.equal(await page.locator('[data-provider-card="openai-compatible"]').evaluate(e=>e.closest('details').open),true);assert.equal(await page.locator('#settings-provider-base-url').isVisible(),true);await configure('hy-mt2-local');assert.equal(await page.locator('.provider-editor').isVisible(),false);
  await page.locator('#local-translation-model-choice').selectOption('/fixture/local.gguf');assert.equal(await page.locator('#settings-hy-mt2-model').inputValue(),'/fixture/local.gguf');
  assert.equal(await page.locator('#local-translation-model-choice option').nth(1).textContent(),'Local <fixture>');assert.equal(await page.locator('#settings-clear-api-key').count(),0);assert.equal(await page.locator('#check-api-key').count(),0);
  assert.equal(await page.evaluate(()=>window.__cards.calls.some(c=>['save_translation_credential','test_translation_connection'].includes(c.cmd))),false);
  await configure('deepseek');await page.locator('#settings-api-key').fill('fixture-only-a');await page.evaluate(()=>window.__cards.hold='save_translation_credential');await page.locator('[data-save-provider-key]').click();assert.equal(await page.locator('[data-delete-provider-key]').isDisabled(),true);
  await configure('gemini');await page.locator('#settings-api-key').fill('fixture-only-b');await page.evaluate(()=>window.__cards.release());await page.waitForFunction(()=>!document.querySelector('[data-save-provider-key]').disabled);assert.equal(await page.locator('#settings-api-key').inputValue(),'fixture-only-b');
  const first=await page.evaluate(()=>window.__cards.calls.find(c=>c.cmd==='save_translation_credential').args);assert.deepEqual(first,{providerId:'deepseek',apiKey:'fixture-only-a',clear:false});
  await page.locator('[data-save-provider-key]').click();await page.locator('[data-settings-tab="transcription"]').click();await page.locator('.onboarding-optional > summary').click();await page.locator('#settings-gemini-api-key').fill('new-asr-fixture');await page.evaluate(()=>window.__cards.release());await page.waitForFunction(()=>!document.querySelector('[data-save-provider-key]').disabled);assert.equal(await page.locator('#settings-gemini-api-key').inputValue(),'new-asr-fixture');
  await page.locator('[data-settings-tab="translation"]').click();await configure('deepseek');await page.evaluate(()=>{window.__cards.hold='check_translation_api_key';window.__cards.release=null;});await page.locator('[data-provider-card="deepseek"] [data-check-provider]').click();await page.waitForFunction(()=>typeof window.__cards.release==='function');const before=await page.locator('#settings-message').innerText();
  await page.locator('#close-settings').click();await page.locator('#open-settings').click();await page.locator('[data-settings-tab="translation"]').click();await page.evaluate(()=>window.__cards.release());await page.waitForFunction(()=>!document.querySelector('[data-provider-card="deepseek"] [data-check-provider]').disabled);assert.equal(await page.locator('#settings-message').innerText(),before);
  await configure('deepl');await page.locator('#settings-api-key').fill('retry-fixture');await page.evaluate(()=>{window.__cards.hold=null;window.__cards.fail=true;});await page.locator('[data-save-provider-key]').click();await page.waitForFunction(()=>document.querySelector('#settings-message').textContent.includes('fixture credential write failed'));assert.equal(await page.locator('#settings-api-key').inputValue(),'retry-fixture');assert.equal(await page.locator('[data-delete-provider-key]').isEnabled(),true);
  await page.evaluate(()=>window.__cards.fail=false);await page.locator('[data-delete-provider-key]').click();await page.waitForFunction(()=>document.querySelector('#settings-message').textContent.includes('deepl 凭据已删除'));assert.deepEqual(await page.evaluate(()=>window.__cards.calls.filter(c=>c.cmd==='save_translation_credential').at(-1).args),{providerId:'deepl',apiKey:null,clear:true});assert.deepEqual(errors,[]);
  await page.locator('[data-provider-card="deepl"] [data-test-provider]').click();await page.waitForFunction(()=>!document.querySelector('[data-provider-card="deepl"] [data-test-provider]').disabled);assert.equal(await page.evaluate(()=>window.__cards.calls.filter(c=>c.cmd==='test_translation_connection').length),1);
  await page.evaluate(()=>window.__cards.hold='save_translation_credential');await page.locator('#settings-api-key').fill('same-value-fixture');await page.locator('[data-save-provider-key]').click();await page.locator('#settings-api-key').evaluate(e=>e.dispatchEvent(new Event('input',{bubbles:true})));await page.evaluate(()=>window.__cards.release());await page.waitForFunction(()=>!document.querySelector('[data-save-provider-key]').disabled);assert.equal(await page.locator('#settings-api-key').inputValue(),'same-value-fixture');
  await page.locator('[data-save-provider-key]').click();await page.locator('#close-settings').click();await page.locator('#open-settings').click();await page.locator('[data-settings-tab="translation"]').click();const profilesBefore=await page.evaluate(()=>window.__cards.calls.filter(c=>c.cmd==='translation_profiles').length);await page.evaluate(()=>window.__cards.release());await page.waitForFunction(()=>!document.querySelector('[data-save-provider-key]').disabled);assert.equal(await page.evaluate(()=>window.__cards.calls.filter(c=>c.cmd==='translation_profiles').length),profilesBefore+1);
  // Independently verify that the actual view's AbortController and late-action guard survive dispose.
  const isolated=await browser.newPage();await isolated.goto('about:blank');
  const transpile=path=>stripTypeScriptTypes(fs.readFileSync(path,'utf8'));
  await isolated.evaluate(async ({template,view})=>{
   const load=async source=>{const url=URL.createObjectURL(new Blob([source],{type:'text/javascript'}));try{return await import(url);}finally{URL.revokeObjectURL(url);}};
   const t=await load(template),v=await load(view);document.body.innerHTML=t.translationProviderTemplate();window.__isolated={calls:0,accepted:0,release:null};
   const state=window.__isolated;state.view=v.mountTranslationProviderView(document.querySelector('[data-translation-settings]'),{visit:()=>1,state:()=>({readiness:new Map(),dirty:new Set(),catalog:[],downloads:[]}),localModel:()=>'',chooseLocalModel:()=>{},action:async(action,id,current)=>{state.calls++;await new Promise(resolve=>state.release=resolve);if(current())state.accepted++;},error:()=>{throw Error('unexpected error');}});
  },{template:transpile('ui/src/translation-provider-template.ts'),view:transpile('ui/src/translation-provider-view.ts')});
  await isolated.locator('[data-provider-card="deepseek"] [data-check-provider]').click();await isolated.evaluate(()=>{window.__isolated.view.dispose();window.__isolated.release();});await isolated.evaluate(()=>new Promise(requestAnimationFrame));await isolated.locator('[data-provider-card="gemini"] [data-check-provider]').click();assert.deepEqual(await isolated.evaluate(()=>[window.__isolated.calls,window.__isolated.accepted]),[1,0]);
  console.log('PASS: static provider cards, shared editor/drafts, model choices, credential intent isolation, retry, late visits and dispose; mock credentials only');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
