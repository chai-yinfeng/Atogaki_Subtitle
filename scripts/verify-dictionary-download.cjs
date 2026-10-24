const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke,state=window.__dictionary={calls:[],downloads:[],networkRelease:null,holdNetwork:true,holdStates:false,statesRelease:null,startStatus:'queued'};
   const download=(dictionaryId,status)=>({dictionaryId,status,downloadedBytes:1,totalBytes:2,path:null,version:'fixture',error:null,source:'fixture'});
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    state.calls.push({cmd,args});
    if(cmd==='dictionary_catalog')return ['a','b'].map(id=>({id,name:'Dictionary '+id,languagePair:'ja/zh',versionLabel:'fixture',sizeLabel:'1 KiB',description:'fixture',license:'fixture',attribution:'fixture',sourceUrl:'https://example.invalid'}));
    if(cmd==='dictionary_download_states'){const snapshot=state.downloads.map(s=>({...s}));if(state.holdStates)return new Promise(resolve=>state.statesRelease=()=>resolve(snapshot));return snapshot;}
    if(cmd==='save_download_network_settings')return state.holdNetwork?new Promise(resolve=>state.networkRelease=resolve):null;
    if(cmd==='start_dictionary_download'){state.downloads=[download(args.dictionaryId,state.startStatus)];return state.downloads[0];}
    return invoke(cmd,args);
   };
  });
  const count=cmd=>page.evaluate(cmd=>window.__dictionary.calls.filter(c=>c.cmd===cmd).length,cmd),button=id=>page.locator(`[data-download-dictionary="${id}"]`),notice=page.locator('#dictionary-download-message');
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('#open-settings').click();await page.locator('[data-settings-tab="network"]').click();await page.locator('#settings-model-mirror').fill('https://old.fixture.test');await page.locator('[data-settings-tab="dictionary"]').click();await button('a').click();await button('a').dispatchEvent('click');assert.equal(await count('save_download_network_settings'),1);assert.equal(await button('b').isDisabled(),true);
  await page.locator('[data-settings-tab="network"]').click();await page.locator('#settings-model-mirror').fill('https://new.fixture.test');await page.evaluate(()=>window.__dictionary.networkRelease());await page.waitForFunction(()=>window.__dictionary.calls.some(c=>c.cmd==='start_dictionary_download'));assert.equal(await page.locator('#settings-model-mirror').inputValue(),'https://new.fixture.test');assert.match(await notice.innerText(),/新的网络输入尚未保存/);assert.equal(await page.evaluate(()=>window.__dictionary.calls.find(c=>c.cmd==='save_download_network_settings').args.request.modelMirrorUrl),'https://old.fixture.test');
  const activeReads=await count('dictionary_download_states');await page.waitForFunction(before=>window.__dictionary.calls.filter(c=>c.cmd==='dictionary_download_states').length>before,activeReads);await page.locator('#close-settings').click();const hiddenReads=await count('dictionary_download_states');await page.waitForTimeout(1100);assert.equal(await count('dictionary_download_states'),hiddenReads);
  await page.locator('#open-settings').click();await page.locator('[data-settings-tab="dictionary"]').click();await page.waitForFunction(()=>document.querySelector('[data-download-dictionary="a"]').disabled);await page.evaluate(()=>window.__dictionary.downloads[0].status='done');await page.waitForFunction(()=>document.querySelector('#dictionary-download-message').textContent.includes('词典包已校验'));assert.equal(await button('a').isEnabled(),true);assert.equal(await button('a').innerText(),'检查更新');assert.equal(await count('start_dictionary_download'),1);
  // Cache-complete start must reload once and must not leave a polling timer.
  await page.evaluate(()=>{window.__dictionary.holdNetwork=false;window.__dictionary.startStatus='done';});await button('a').click();await page.waitForFunction(()=>window.__dictionary.calls.filter(c=>c.cmd==='start_dictionary_download').length===2&&!document.querySelector('[data-download-dictionary="a"]').disabled);const terminalReads=await count('dictionary_download_states');await page.waitForTimeout(1100);assert.equal(await count('dictionary_download_states'),terminalReads);
  // Closing a pending network preflight retains the explicit download intent, but no hidden observation.
  await page.evaluate(()=>{window.__dictionary.holdNetwork=true;window.__dictionary.startStatus='queued';window.__dictionary.networkRelease=null;});await button('b').click();await page.waitForFunction(()=>window.__dictionary.networkRelease);await page.locator('#close-settings').click();const closedReads=await count('dictionary_download_states');await page.evaluate(()=>window.__dictionary.networkRelease());await page.waitForFunction(()=>window.__dictionary.calls.filter(c=>c.cmd==='start_dictionary_download').length===3);await page.waitForTimeout(1100);assert.equal(await count('dictionary_download_states'),closedReads);
  // A status request from the old visit cannot replace a fresh completed state.
  await page.locator('#open-settings').click();await page.locator('[data-settings-tab="dictionary"]').click();await page.waitForFunction(()=>document.querySelector('[data-download-dictionary="b"]').disabled);await page.evaluate(()=>window.__dictionary.holdStates=true);await page.waitForFunction(()=>window.__dictionary.statesRelease);await page.locator('#close-settings').click();await page.evaluate(()=>{window.__dictionary.holdStates=false;window.__dictionary.downloads[0].status='done';});await page.locator('#open-settings').click();await page.waitForFunction(()=>document.querySelector('[data-download-dictionary="b"]').textContent==='检查更新');await page.evaluate(()=>window.__dictionary.statesRelease());await page.evaluate(()=>new Promise(requestAnimationFrame));assert.equal(await button('b').innerText(),'检查更新');assert.equal(await button('b').isEnabled(),true);
  assert.equal(await count('save_desktop_settings'),0);assert.equal(await count('save_dictionary_credential'),0);assert.equal(await count('start_model_download'),0);assert.deepEqual(errors,[]);
  console.log('PASS: dictionary download frozen network/new inputs, duplicate prevention, serial visible polling, hidden read stop, cache completion, retained explicit intent and late status isolation; mock downloads only');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
