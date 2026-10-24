const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),{stripTypeScriptTypes}=require('node:module');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke,status={providerId:'gemini-transcribe',providerName:'Gemini',configured:true,credentialStore:'fixture'};
   window.__asr={calls:[],pending:{},failProfiles:false};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const state=window.__asr;state.calls.push({cmd,args});
    if(['save_asr_credential','check_asr_credential','save_translation_credential','save_desktop_settings'].includes(cmd))return new Promise((resolve,reject)=>state.pending[cmd]={resolve:()=>resolve(cmd==='save_desktop_settings'?{needsOnboarding:false,translationProviderId:'gemini',credentialStore:'fixture'}:status),reject});
    if(cmd==='translation_profiles'){if(state.failProfiles)throw Error('fixture profile refresh');return ['deepseek','gemini'].map(provider_id=>({provider_id,model:'fixture',base_url:'https://example.invalid',style:'fixture',readiness:'unconfigured'}));}
    if(cmd==='asr_credential_status')return status;
    return invoke(cmd,args);
   };
  });
  const count=cmd=>page.evaluate(cmd=>window.__asr.calls.filter(c=>c.cmd===cmd).length,cmd);
  const finish=cmd=>page.evaluate(cmd=>{window.__asr.pending[cmd].resolve();delete window.__asr.pending[cmd];},cmd);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('#open-settings').click();
  const asr=page.locator('#settings-gemini-api-key'),notice=page.locator('#gemini-api-key-status'),save=page.locator('#save-gemini-api-key');
  const transcription=async()=>{await page.locator('[data-settings-tab="transcription"]').click();if(!await asr.isVisible())await page.getByText('可选：Gemini 局部修复',{exact:true}).click();};
  const translation=async()=>{await page.locator('[data-settings-tab="translation"]').click();await page.locator('[data-provider-card="gemini"] [data-configure-provider]').click();};
  await transcription();await asr.fill('fixture-unsaved');await page.locator('#check-gemini-api-key').focus();await page.locator('#check-gemini-api-key').press('Enter');assert.equal(await save.isDisabled(),true);await finish('check_asr_credential');await page.waitForFunction(()=>!document.querySelector('#save-gemini-api-key').disabled);assert.equal(await asr.inputValue(),'fixture-unsaved');assert.equal(await page.locator('#check-gemini-api-key').evaluate(e=>document.activeElement===e),true);
  await translation();await page.locator('#settings-api-key').fill('fixture-translation-before');await transcription();await save.click();await save.dispatchEvent('click');assert.equal(await count('save_asr_credential'),1);await asr.fill('fixture-asr-new');await page.locator('#save-settings').click();assert.equal(await count('save_desktop_settings'),0);await translation();await page.locator('#settings-api-key').fill('fixture-translation-new');await page.locator('[data-save-provider-key]').click();assert.equal(await count('save_translation_credential'),0);await finish('save_asr_credential');await page.waitForFunction(()=>!document.querySelector('#save-gemini-api-key').disabled);assert.equal(await asr.inputValue(),'fixture-asr-new');assert.equal(await page.locator('#settings-api-key').inputValue(),'fixture-translation-new');
  await transcription();await save.click();await asr.evaluate(e=>e.dispatchEvent(new Event('input',{bubbles:true})));await finish('save_asr_credential');await page.waitForFunction(()=>!document.querySelector('#save-gemini-api-key').disabled);assert.equal(await asr.inputValue(),'fixture-asr-new');
  await save.click();await page.evaluate(()=>window.__asr.failProfiles=true);await finish('save_asr_credential');await page.waitForFunction(()=>document.querySelector('#gemini-api-key-status').textContent.includes('状态刷新失败'));assert.equal(await asr.inputValue(),'');await page.evaluate(()=>window.__asr.failProfiles=false);
  await asr.fill('fixture-failure');await save.click();await page.evaluate(()=>window.__asr.pending.save_asr_credential.reject(Error('fixture write failed')));await page.waitForFunction(()=>!document.querySelector('#save-gemini-api-key').disabled);assert.equal(await asr.inputValue(),'fixture-failure');assert.match(await notice.innerText(),/操作失败/);
  await save.click();await page.locator('#close-settings').click();await page.locator('#open-settings').click();await transcription();await asr.fill('fixture-new-visit');await finish('save_asr_credential');await page.waitForFunction(()=>!document.querySelector('#save-gemini-api-key').disabled);assert.equal(await asr.inputValue(),'fixture-new-visit');assert.equal(await notice.innerText(),'');
  // A pending check must not publish pre-write state after a shared credential mutation.
  await page.locator('#check-gemini-api-key').click();await translation();await page.locator('#settings-api-key').fill('fixture-provider');await page.locator('[data-save-provider-key]').click();await asr.evaluate(e=>e.dispatchEvent(new Event('input',{bubbles:true})));await finish('save_translation_credential');await page.waitForFunction(()=>!document.querySelector('[data-save-provider-key]').disabled);await finish('check_asr_credential');await page.waitForFunction(()=>!document.querySelector('#check-gemini-api-key').disabled);assert.equal(await asr.inputValue(),'fixture-new-visit');
  // Verify reverse ownership from a pending global save to ASR persistence.
  await page.locator('#save-settings').click();await page.waitForFunction(()=>window.__asr.pending.save_desktop_settings);const before=await count('save_asr_credential');await transcription();await save.click();assert.equal(await count('save_asr_credential'),before);await finish('save_desktop_settings');await page.waitForFunction(()=>!document.querySelector('#save-settings').disabled);
  const isolated=await browser.newPage();isolated.on('pageerror',e=>errors.push(String(e)));await isolated.setContent('<dialog open><button id="save-gemini-api-key">Save</button><button id="check-gemini-api-key">Check</button><button id="clear-gemini-api-key">Delete</button></dialog>');
  await isolated.evaluate(async source=>{const url=URL.createObjectURL(new Blob([source],{type:'text/javascript'}));const {mountAsrCredentialView}=await import(url);URL.revokeObjectURL(url);let dead=false;const guard=()=>{if(dead)throw Error('released view context');};const state=window.__view={calls:0,release:null,view:null};state.view=mountAsrCredentialView(document.querySelector('dialog'),{get busy(){guard();return false;},run:()=>{state.calls++;return new Promise(resolve=>state.release=resolve);}},()=>{guard();return 1;});state.dispose=()=>{state.view.dispose();dead=true;};},stripTypeScriptTypes(fs.readFileSync('ui/src/asr-credential-view.ts','utf8')));
  await isolated.locator('#save-gemini-api-key').focus();await isolated.locator('#save-gemini-api-key').press('Enter');await isolated.evaluate(()=>{window.__view.dispose();window.__view.release();window.__view.view.render();});await isolated.locator('#check-gemini-api-key').click();assert.equal(await isolated.evaluate(()=>window.__view.calls),1);assert.deepEqual(errors,[]);
  console.log('PASS: ASR check preserves input, duplicate prevention, shared write ownership, newer/same-value drafts, precise refresh failure, retry, visit isolation, stale checks and view disposal; fixture credentials only');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
