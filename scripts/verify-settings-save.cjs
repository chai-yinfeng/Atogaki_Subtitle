const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   const initial={onboardingCompleted:true,needsOnboarding:false,whisperModelPath:'/fixture-model',vadModelPath:null,hyMt2ModelPath:null,translationProviderId:'none',translationModel:null,translationBaseUrl:null,translationStyleInstruction:'fixture-style',networkProxyMode:'environment',networkProxyUrl:null,modelMirrorUrl:null,credentialStore:'test',modelsDirectory:'/test/models'};
   window.__settings={stored:initial,writes:[],hold:true,fail:false,profileFail:false,release:null,reads:0};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const state=window.__settings;
    if(cmd==='desktop_settings'){state.reads++;const value=structuredClone(state.stored);if(state.holdRead){state.holdRead=false;return new Promise(resolve=>state.releaseRead=()=>resolve(value));}return value;}
    if(cmd==='save_desktop_settings'){
     state.writes.push(structuredClone(args.request));if(state.fail)throw Error('fixture write failed');
     const commit=()=>{state.stored={...initial,...args.request};return structuredClone(state.stored);};
     return state.hold?new Promise(resolve=>state.release=()=>resolve(commit())):commit();
    }
    if(cmd==='translation_profiles'){
     if(state.profileFail)throw Error('fixture profiles unavailable');
     return [{provider_id:state.stored.translationProviderId,model:state.stored.translationModel,base_url:state.stored.translationBaseUrl,style:state.stored.translationStyleInstruction,readiness:'unverified'}];
    }
    return invoke(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('#open-settings').click();await page.waitForFunction(()=>document.querySelector('#settings-whisper-model').value==='/fixture-model');
  await page.locator('[data-settings-tab="translation"]').click();await page.locator('[data-provider-card="deepseek"] [data-configure-provider]').click();
  await page.locator('#settings-provider-model').fill('before-save');await page.locator('#save-settings').click();assert.equal(await page.locator('#save-settings').isDisabled(),true);assert.equal(await page.locator('#finish-settings').isDisabled(),true);
  await page.locator('#settings-provider-model').fill('after-save');await page.evaluate(()=>window.__settings.release());await page.waitForFunction(()=>!document.querySelector('#save-settings').disabled);
  assert.equal(await page.locator('#settings-provider-model').inputValue(),'after-save');assert.equal(await page.locator('#settings-provider-model').evaluate(e=>document.activeElement===e),true);assert.match(await page.locator('#settings-message').innerText(),/新输入尚未保存/);
  assert.equal(await page.evaluate(()=>window.__settings.writes[0].translationModel),'before-save');assert.match(await page.locator('[data-provider-card="deepseek"] [data-provider-readiness]').innerText(),/草稿未保存/);
  await page.locator('#save-settings').click();await page.locator('[data-provider-card="gemini"] [data-configure-provider]').click();await page.locator('#settings-provider-model').fill('gemini-draft');await page.evaluate(()=>window.__settings.release());await page.waitForFunction(()=>!document.querySelector('#save-settings').disabled);
  assert.equal(await page.locator('#settings-provider').inputValue(),'gemini');assert.equal(await page.locator('#settings-provider-model').inputValue(),'gemini-draft');assert.equal(await page.evaluate(()=>window.__settings.writes[1].translationProviderId),'deepseek');
  await page.evaluate(()=>{window.__settings.hold=false;window.__settings.profileFail=true;});await page.locator('#save-settings').click();await page.waitForFunction(()=>document.querySelector('#settings-message').textContent.includes('已保存，但状态刷新未完成'));assert.equal(await page.locator('#settings-provider-model').inputValue(),'gemini-draft');
  await page.evaluate(()=>{window.__settings.profileFail=false;window.__settings.fail=true;});await page.locator('#settings-provider-model').fill('retry-draft');await page.locator('#save-settings').focus();await page.locator('#save-settings').press('Enter');await page.waitForFunction(()=>document.querySelector('#settings-message').textContent.startsWith('保存失败'));assert.equal(await page.locator('#settings-provider-model').inputValue(),'retry-draft');assert.equal(await page.locator('#save-settings').evaluate(e=>document.activeElement===e),true,JSON.stringify(await page.evaluate(()=>({id:document.activeElement?.id,tag:document.activeElement?.tagName}))));
  await page.evaluate(()=>{window.__settings.fail=false;window.__settings.hold=true;});await page.locator('#save-settings').click();await page.locator('#close-settings').click();await page.locator('#open-settings').click();await page.locator('[data-settings-tab="translation"]').click();
  await page.locator('#settings-provider-model').fill('new-visit-draft');await page.evaluate(()=>window.__settings.release());await page.waitForFunction(()=>!document.querySelector('#save-settings').disabled);await page.waitForFunction(()=>window.__settings.reads>=4);
  assert.equal(await page.locator('#settings-provider-model').inputValue(),'new-visit-draft');assert.equal(await page.locator('#settings-dialog').evaluate(e=>e.open),true);
  await page.locator('#close-settings').click();await page.evaluate(()=>{window.__settings.holdRead=true;window.__settings.hold=false;});await page.locator('#open-settings').click();await page.waitForFunction(()=>typeof window.__settings.releaseRead==='function');
  await page.locator('[data-settings-tab="translation"]').click();await page.locator('#settings-provider-model').fill('final-save');await page.locator('#save-settings').click();await page.waitForFunction(()=>!document.querySelector('#save-settings').disabled);await page.evaluate(()=>window.__settings.releaseRead());await page.evaluate(()=>new Promise(requestAnimationFrame));
  assert.equal(await page.locator('#settings-provider-model').inputValue(),'final-save');
  const writes=await page.evaluate(()=>window.__settings.writes);assert.equal(writes.length,6);assert.equal(writes.every(r=>r.apiKey===null&&!r.clearApiKey),true);
  assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>['translate_subtitle','translate_all_subtitles','save_translation_credential'].includes(c.cmd))),false);assert.deepEqual(errors,[]);
  console.log('PASS: settings frozen save, duplicate prevention, new input/provider drafts, partial refresh, failure retry and close/reopen isolation; no real credentials or translation');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
