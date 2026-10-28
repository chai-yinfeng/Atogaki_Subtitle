const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright'),assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,20,60000);
  await page.addInitScript(()=>{
   const base=window.__TAURI_INTERNALS__.invoke,test=window.__test;
   const glossary={id:'A',name:'修正测试',source_language:'ja',core_term_count:0,content_group_count:0,correction_only_count:0};
   Object.assign(test,{correctionReads:[],correctionWrites:[],holdCorrectionRead:false,holdCorrectionWrite:false});
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='list_glossaries')return [glossary];
    if(cmd==='get_glossary')return test.holdCorrectionRead?new Promise((resolve,reject)=>test.correctionReads.push({resolve:()=>resolve({glossary,terms:[]}),reject})):{glossary,terms:[]};
    if(cmd==='save_glossary'){
     const result={glossary,terms:args.request.terms};
     const request={args,resolve:null,reject:null};test.correctionWrites.push(request);
     return test.holdCorrectionWrite?new Promise((resolve,reject)=>{request.resolve=()=>resolve(result);request.reject=reject;}):result;
    }
    return base(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  await page.getByRole('button',{name:'翻译与词表',exact:true}).click();await page.locator('#workspace-glossary').selectOption('A');await page.locator('#back-to-jobs').click();
  const dialog=page.locator('#glossary-correction-dialog'),submit=page.locator('#glossary-correction-form button[type=submit]');
  const open=async(target)=>{
   if(!await page.locator('.editor-menu').evaluate(x=>x.open))await page.locator('.editor-menu > summary').click();
   await page.locator('[data-editor-action="glossary"]').click();await dialog.waitFor({state:'visible'});
   assert.equal(await dialog.getAttribute('role'),'region');
   assert.equal(await page.locator('#karaoke-view').evaluate(node=>node.inert),true);
   assert.equal(await page.evaluate(()=>document.activeElement.id),'glossary-correction-source');
   await page.locator('#glossary-correction-source').fill('误识别');await page.locator('#glossary-correction-target').fill(target);
  };
  // Cancelling during the preparatory read must not start a write or close a new dialog.
  await open('旧请求');
  await submit.focus();await page.keyboard.press('Tab');assert.equal(await page.evaluate(()=>document.activeElement.id),'cancel-glossary-correction');
  await page.keyboard.press('Shift+Tab');assert.equal(await page.evaluate(()=>document.activeElement.closest('form')?.id),'glossary-correction-form');assert.equal(await submit.evaluate(node=>node===document.activeElement),true);
  await page.evaluate(()=>window.__test.holdCorrectionRead=true);await submit.click();
  await page.waitForFunction(()=>window.__test.correctionReads.length===1);await page.keyboard.press('Escape');
  assert.equal(await page.evaluate(()=>document.activeElement.matches('.editor-menu > summary')),true);await open('新输入');
  await page.evaluate(()=>window.__test.correctionReads[0].resolve());await page.waitForTimeout(70);
  assert.equal(await page.evaluate(()=>window.__test.correctionWrites.length),0,'cancelled read must not write glossary');
  assert.equal(await dialog.isVisible(),true);assert.equal(await page.locator('#glossary-correction-target').inputValue(),'新输入');
  // A write already sent may complete; its old UI callbacks cannot affect a new visit.
  await page.evaluate(()=>{window.__test.holdCorrectionRead=false;window.__test.holdCorrectionWrite=true;});await submit.click();
  await page.waitForFunction(()=>window.__test.correctionWrites.length===1);
  await page.locator('#glossary-correction-form').evaluate(form=>form.requestSubmit());await page.waitForTimeout(30);
  assert.equal(await page.evaluate(()=>window.__test.correctionWrites.length),1,'single writer per dialog visit');
  await page.locator('#cancel-glossary-correction').click();await open('保留这个新输入');
  await page.evaluate(()=>window.__test.correctionWrites[0].resolve());await page.waitForTimeout(70);
  assert.equal(await dialog.isVisible(),true);assert.equal(await page.locator('#glossary-correction-target').inputValue(),'保留这个新输入');
  assert.equal(await page.locator('#glossary-correction-message').textContent(),'');
  // Current failure remains retryable, and normal success still closes the dialog.
  await submit.click();await page.waitForFunction(()=>window.__test.correctionWrites.length===2);
  await page.evaluate(()=>window.__test.correctionWrites[1].reject(Error('test save failure')));
  await page.waitForFunction(()=>document.querySelector('#glossary-correction-message').textContent.includes('test save failure'));
  assert.equal(await submit.isDisabled(),false);assert.equal(await page.locator('#glossary-correction-target').inputValue(),'保留这个新输入');
  await page.evaluate(()=>window.__test.holdCorrectionWrite=false);await submit.click();await page.waitForFunction(()=>!document.querySelector('#glossary-correction-dialog')?.open);
  assert.equal(await page.locator('#glossary-correction-dialog').count(),0);
  assert.equal(await page.locator('#karaoke-view').evaluate(node=>node.inert),false);
  assert.equal(await page.evaluate(()=>window.__test.correctionWrites.length),3);assert.deepEqual(errors,[]);
  console.log('PASS: cancelled correction read, old write isolation, duplicate submit, failure retry and normal save');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
