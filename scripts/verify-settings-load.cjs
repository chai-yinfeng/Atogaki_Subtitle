const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   const settings={onboardingCompleted:true,needsOnboarding:false,whisperModelPath:'/fresh-model',vadModelPath:null,hyMt2ModelPath:null,translationProviderId:'none',translationModel:null,translationBaseUrl:null,translationStyleInstruction:'fixture-style',networkProxyMode:'environment',networkProxyUrl:null,modelMirrorUrl:null,credentialStore:'test',modelsDirectory:'/test/models'};
   window.__load={reads:0,models:0,directories:0,releaseStartup:null,releaseDictionary:null,rejectSettings:null,failNextRead:false};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const state=window.__load;
    if(cmd==='desktop_settings'){
     state.reads++;
     if(state.reads===1)return new Promise(resolve=>state.releaseStartup=()=>resolve({...settings,needsOnboarding:true,whisperModelPath:'/stale-model'}));
     if(state.failNextRead){state.failNextRead=false;return new Promise((resolve,reject)=>state.rejectSettings=()=>reject(Error('fixture current read failed')));}
     return structuredClone(settings);
    }
    if(cmd==='model_catalog'){state.models++;return [{id:'fixture-model',kind:'whisper',name:'Fixture model',fileName:'fixture.bin',sizeLabel:'1 KiB',recommendedFor:'fixture',sourceUrl:'https://example.invalid/fixture'}];}
    if(cmd==='dictionary_directory'){
     state.directories++;
     if(state.directories===1)return new Promise(resolve=>state.releaseDictionary=()=>resolve('/stale-dictionary'));
     return '/fresh-dictionary-'+state.directories;
    }
    return invoke(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.waitForFunction(()=>typeof window.__load.releaseStartup==='function');await page.locator('#open-settings').click();
  await page.waitForFunction(()=>typeof window.__load.releaseDictionary==='function');assert.equal(await page.locator('#settings-whisper-model').inputValue(),'/fresh-model');
  await page.locator('#close-settings').click();await page.locator('#open-settings').click();await page.waitForFunction(()=>document.querySelector('#dictionary-directory').textContent.includes('/fresh-dictionary-2'));
  await page.evaluate(()=>window.__load.releaseDictionary());await page.evaluate(()=>new Promise(requestAnimationFrame));assert.match(await page.locator('#dictionary-directory').textContent(),/fresh-dictionary-2/);
  await page.locator('#close-settings').click();await page.evaluate(()=>window.__load.releaseStartup());await page.evaluate(()=>new Promise(requestAnimationFrame));assert.equal(await page.locator('#settings-dialog').evaluate(e=>e.open),false);assert.equal(await page.locator('#settings-whisper-model').inputValue(),'/fresh-model');
  assert.equal(await page.evaluate(()=>window.__load.models),2);
  await page.evaluate(()=>window.__load.failNextRead=true);await page.locator('#open-settings').click();await page.waitForFunction(()=>typeof window.__load.rejectSettings==='function');await page.evaluate(()=>window.__load.rejectSettings());await page.waitForFunction(()=>document.querySelector('#settings-message').textContent.includes('无法读取启动配置'));
  await page.locator('#close-settings').click();await page.locator('#open-settings').click();await page.waitForFunction(()=>document.querySelector('#dictionary-directory').textContent.includes('/fresh-dictionary-3'));assert.equal(await page.locator('#settings-whisper-model').inputValue(),'/fresh-model');assert.equal(await page.locator('#settings-message').innerText(),'');
  assert.equal(await page.evaluate(()=>window.__test.calls.some(c=>['save_desktop_settings','save_translation_credential','translate_subtitle','translate_all_subtitles'].includes(c.cmd))),false);assert.deepEqual(errors,[]);
  console.log('PASS: settings startup/open/close generations, late dictionary isolation, catalog reuse and failed-read recovery; read-only fixture');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
