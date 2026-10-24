const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),{stripTypeScriptTypes}=require('node:module');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;window.__fields={profiles:[],hold:false,release:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const state=window.__fields;
    if(cmd==='desktop_settings'){
     const value={needsOnboarding:false,onboardingCompleted:true,whisperModelPath:'/fixture',translationProviderId:'deepseek',translationModel:'saved-a',translationBaseUrl:'https://a.invalid',translationStyleInstruction:'style-a',credentialStore:'fixture',translationApiKeySource:'saved',modelsDirectory:'/test/models',networkProxyMode:'environment'};
     return state.hold?new Promise(resolve=>state.release=()=>resolve(value)):value;
    }
    if(cmd==='translation_profiles')return state.profiles;
    if(cmd==='check_translation_api_key')return {providerId:args.providerId,providerName:args.providerId,storedInSystem:false,availableFromEnvironment:false,credentialStore:'fixture'};
    return invoke(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('#open-settings').click();await page.locator('[data-settings-tab="translation"]').click();await page.waitForFunction(()=>document.querySelector('#settings-provider-model').value==='saved-a');
  const choose=id=>page.locator(`[data-provider-card="${id}"] [data-configure-provider]`).click();
  await page.locator('#settings-provider-model').fill('draft-a');await choose('gemini');
  await page.evaluate(()=>window.__fields.profiles=['deepseek','gemini'].map(id=>({provider_id:id,model:id==='gemini'?'saved-b':'saved-a',base_url:null,style:id+' style',readiness:'unconfigured'})));
  await page.locator('[data-provider-card="gemini"] [data-check-provider]').click();await page.waitForFunction(()=>document.querySelector('#settings-provider-model').value==='saved-b');assert.equal(await page.locator('#settings-provider-base-url').inputValue(),'https://generativelanguage.googleapis.com/v1beta/openai');
  await choose('deepseek');assert.equal(await page.locator('#settings-provider-model').inputValue(),'draft-a');await choose('gemini');assert.equal(await page.locator('#settings-provider-model').inputValue(),'saved-b');
  await page.locator('#close-settings').click();await page.evaluate(()=>window.__fields.hold=true);await page.locator('#open-settings').click();await page.waitForFunction(()=>typeof window.__fields.release==='function');await page.locator('#settings-api-key').fill('fixture-only-pending');await page.evaluate(()=>window.__fields.release());await page.evaluate(()=>new Promise(requestAnimationFrame));assert.equal(await page.locator('#settings-provider').inputValue(),'gemini');assert.equal(await page.locator('#settings-api-key').inputValue(),'fixture-only-pending');assert.match(await page.locator('#api-key-status').innerText(),/检查凭据是否存在/);
  await choose('deepseek');assert.equal(await page.locator('#settings-api-key').inputValue(),'');assert.equal(await page.locator('#settings-provider-model').inputValue(),'draft-a');
  assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>['save_translation_credential','save_desktop_settings','test_translation_connection','translate_all_subtitles'].includes(c.cmd))),false);assert.deepEqual(errors,[]);
  const html=await page.locator('[data-translation-settings]').evaluate(e=>e.outerHTML),isolated=await browser.newPage();await isolated.setContent(html);
  await isolated.evaluate(async sources=>{
   const load=async source=>{const url=URL.createObjectURL(new Blob([source],{type:'text/javascript'}));try{return await import(url);}finally{URL.revokeObjectURL(url);}};
   const {createTranslationProviderDrafts}=await load(sources.drafts),{mountTranslationProviderFields}=await load(sources.fields);const state=window.__isolated={drafts:createTranslationProviderDrafts(),changes:0,dead:false};state.view=mountTranslationProviderFields(document.querySelector('[data-translation-settings]'),state.drafts,{dirty:()=>false,clearSecretDirty:()=>{},changed:()=>{if(state.dead)throw Error('disposed callback');state.changes++;}});document.querySelector('#settings-provider').value='deepseek';document.querySelector('#settings-provider').dispatchEvent(new Event('change',{bubbles:true}));
  },{drafts:stripTypeScriptTypes(fs.readFileSync('ui/src/translation-provider-drafts.ts','utf8')),fields:stripTypeScriptTypes(fs.readFileSync('ui/src/translation-provider-fields.ts','utf8'))});
  assert.equal(await isolated.evaluate(()=>{const state=window.__isolated;state.drafts.edit({model:'pending',endpoint:'pending',style:'pending'});state.view.load({translationProviderId:'gemini',translationModel:'server',translationBaseUrl:null,translationStyleInstruction:'server'});return state.drafts.selected;}),'deepseek');
  const before=await isolated.evaluate(()=>{window.__isolated.view.dispose();window.__isolated.dead=true;return window.__isolated.changes;});await isolated.evaluate(()=>{const field=document.querySelector('#settings-provider-model');field.value='late';field.dispatchEvent(new Event('input',{bubbles:true}));document.querySelector('#settings-provider').dispatchEvent(new Event('change',{bubbles:true}));window.__isolated.view.render();window.__isolated.view.load({translationProviderId:'gemini'});});assert.equal(await isolated.evaluate(()=>window.__isolated.changes),before);
  console.log('PASS: per-provider late profiles, independent drafts/endpoints, pending-key identity, accurate hint, switch clearing and dispose; no credential writes');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
