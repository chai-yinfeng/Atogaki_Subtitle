const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;window.__picker={calls:[],resolve:null,reject:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(['pick_model_file','pick_vad_model_file','pick_hy_mt2_model_file'].includes(cmd)){window.__picker.calls.push(cmd);return new Promise((resolve,reject)=>{window.__picker.resolve=resolve;window.__picker.reject=reject;});}
    if(cmd==='desktop_settings')return {needsOnboarding:false,onboardingCompleted:true,whisperModelPath:'/fixture-original.bin',vadModelPath:null,hyMt2ModelPath:null,translationProviderId:'none',translationStyleInstruction:'fixture',credentialStore:'fixture',modelsDirectory:'/test/models',networkProxyMode:'environment'};
    return invoke(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('#open-settings').click();await page.waitForFunction(()=>document.querySelector('#settings-whisper-model').value==='/fixture-original.bin');
  const whisper=page.locator('#settings-choose-whisper'),field=page.locator('#settings-whisper-model'),notice=page.locator('#settings-message');
  await whisper.focus();await whisper.press('Enter');assert.equal(await page.locator('#settings-choose-vad').isDisabled(),true);assert.equal(await page.locator('#settings-choose-hy-mt2').isDisabled(),true);await whisper.dispatchEvent('click');assert.equal(await page.evaluate(()=>window.__picker.calls.length),1);
  await field.fill('/newer.bin');await page.evaluate(()=>window.__picker.resolve('/old-result.bin'));await page.waitForFunction(()=>!document.querySelector('#settings-choose-whisper').disabled);assert.equal(await field.inputValue(),'/newer.bin');assert.match(await notice.innerText(),/保留现有值/);assert.equal(await field.evaluate(e=>document.activeElement===e),true);
  await whisper.click();await field.evaluate(e=>e.dispatchEvent(new Event('input',{bubbles:true})));await page.evaluate(()=>window.__picker.resolve('/same-value-race.bin'));await page.waitForFunction(()=>!document.querySelector('#settings-choose-whisper').disabled);assert.equal(await field.inputValue(),'/newer.bin');
  await whisper.focus();await whisper.press('Enter');await page.evaluate(()=>window.__picker.resolve(null));await page.waitForFunction(()=>document.querySelector('#settings-message').textContent.includes('已取消选择'));assert.equal(await whisper.evaluate(e=>document.activeElement===e),true);
  await whisper.press('Enter');await page.evaluate(()=>window.__picker.reject(Error('fixture chooser failure')));await page.waitForFunction(()=>document.querySelector('#settings-message').textContent.includes('无法打开模型选择器'));assert.equal(await whisper.isEnabled(),true);await whisper.click();await page.evaluate(()=>window.__picker.resolve('/fixture/日本語 <model>.bin'));await page.waitForFunction(()=>document.querySelector('#settings-whisper-model').value.includes('日本語'));assert.equal(await field.inputValue(),'/fixture/日本語 <model>.bin');assert.match(await notice.innerText(),/尚未保存配置/);
  await page.locator('#settings-choose-vad').click();await page.locator('[data-settings-tab="models"]').click();await page.evaluate(()=>window.__picker.resolve('/vad-choice.bin'));await page.waitForFunction(()=>!document.querySelector('#settings-choose-vad').disabled);assert.equal(await page.locator('#settings-vad-model').inputValue(),'/vad-choice.bin');assert.equal(await page.locator('#settings-hy-mt2-model').inputValue(),'');
  await page.locator('#settings-choose-hy-mt2').click();await page.evaluate(()=>window.__picker.resolve('/custom-choice.gguf'));await page.waitForFunction(()=>document.querySelector('#settings-hy-mt2-model').value==='/custom-choice.gguf');
  await page.locator('#settings-choose-hy-mt2').click();await page.locator('#close-settings').click();await page.locator('#open-settings').click();await page.locator('#settings-hy-mt2-model').fill('/new-visit.gguf');await page.evaluate(()=>window.__picker.resolve('/old-visit.gguf'));await page.waitForFunction(()=>!document.querySelector('#settings-choose-hy-mt2').disabled);assert.equal(await page.locator('#settings-hy-mt2-model').inputValue(),'/new-visit.gguf');assert.equal(await notice.innerText(),'');
  assert.equal(await page.evaluate(()=>window.__picker.calls.includes('pick_vad_model_file')&&window.__picker.calls.includes('pick_hy_mt2_model_file')),true);assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>['save_desktop_settings','start_model_download','select_model','uninstall_model','test_translation_connection'].includes(c.cmd))),false);assert.deepEqual(errors,[]);
  console.log('PASS: model picker duplicate prevention, newer/same-value input, cancel/error/retry, keyboard focus, captured target across tabs and late visit isolation; no configuration writes');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
