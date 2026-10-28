const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1);
  await page.locator('[data-editor-action="repair"]').click();
  const provider=page.locator('#retranscription-provider'),auth=page.locator('#authorize-gemini-upload');
  const local=await provider.inputValue();
  await provider.selectOption('gemini-transcribe');assert.equal(await page.locator('#gemini-upload-consent').isVisible(),true);
  for(const id of ['retranscription-start','retranscription-end']){await auth.check();await page.locator('#'+id).fill('00:00:01.000');assert.equal(await auth.isChecked(),false);}
  await auth.check();await provider.selectOption(local);assert.equal(await auth.isChecked(),false);assert.equal(await page.locator('#gemini-upload-consent').isVisible(),false);
  await provider.selectOption('gemini-transcribe');await auth.check();await page.locator('#retranscription-use-current').click();
  assert.equal(await auth.isChecked(),false);assert.equal(await page.locator('#retranscription-start').inputValue(),'00:00:00.000');assert.equal(await page.locator('#retranscription-end').inputValue(),'00:00:02.800');
  await page.locator('#retranscription-start').fill('00:00:00.100');await page.locator('#retranscription-end').fill('00:00:01.000');await auth.check();await page.locator('#retranscription-expand-segments').click();
  assert.equal(await auth.isChecked(),false);assert.equal(await page.locator('#retranscription-start').inputValue(),'00:00:00.000');assert.equal(await page.locator('#retranscription-end').inputValue(),'00:00:02.800');
  await auth.check();await page.locator('#retranscription-use-window').click();assert.equal(await auth.isChecked(),false);assert.match(await page.locator('#retranscription-message').textContent(),/当前可视范围/);
  await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));
  await page.evaluate(async()=>{
   const check=(ok,message)=>{if(!ok)throw Error(message);};
   const {mountEditorRepairControls}=await import('/src/editor-repair-controls.ts');
   const repair=document.createElement('section'),candidate=document.createElement('dialog');
   repair.innerHTML='<select id="retranscription-provider"><option value="local">local</option><option value="gemini-transcribe">cloud</option></select><div id="gemini-upload-consent" class="hidden"><input id="authorize-gemini-upload" type="checkbox"></div><input id="retranscription-start"><input id="retranscription-end">';
   const bindings={'retranscription-use-current':'useCurrent','retranscription-use-window':'useWindow','retranscription-expand-segments':'expand','preview-retranscription':'preview'},candidateBindings={'close-retranscription-preview':'closeCandidate','cancel-retranscription':'closeCandidate','confirm-retranscription':'confirm'};
   for(const [root,entries] of [[repair,bindings],[candidate,candidateBindings]])for(const id of Object.keys(entries)){const b=document.createElement('button');b.id=id;root.append(b);}
   document.body.append(repair,candidate);const calls=[];let adopting=false;
   const options=Object.fromEntries([...new Set([...Object.values(bindings),...Object.values(candidateBindings)])].map(name=>[name,()=>calls.push(name)]));options.adopting=()=>adopting;
   let view=mountEditorRepairControls(repair,candidate,options);
   for(const [root,entries] of [[repair,bindings],[candidate,candidateBindings]])for(const id of Object.keys(entries))root.querySelector('#'+id).click();
   check(JSON.stringify(calls)===JSON.stringify([...Object.values(bindings),...Object.values(candidateBindings)]),'each callback exactly once');
   const cancel=()=>{const e=new Event('cancel',{cancelable:true});candidate.dispatchEvent(e);return e.defaultPrevented;};
   check(!cancel(),'idle cancel allowed');adopting=true;check(cancel(),'adopting cancel blocked');
   view.dispose();view.dispose();const before=calls.length;
   for(const [root,entries] of [[repair,bindings],[candidate,candidateBindings]])for(const id of Object.keys(entries))root.querySelector('#'+id).click();
   const consent=repair.querySelector('#gemini-upload-consent'),auth=repair.querySelector('#authorize-gemini-upload'),provider=repair.querySelector('select');
   auth.checked=true;provider.value='gemini-transcribe';provider.dispatchEvent(new Event('change'));repair.querySelector('#retranscription-start').dispatchEvent(new Event('input'));repair.querySelector('#retranscription-end').dispatchEvent(new Event('input'));
   check(calls.length===before&&auth.checked&&consent.classList.contains('hidden')&&!cancel(),'disposed buttons, fields, presentation and cancel silent');
   view=mountEditorRepairControls(repair,candidate,options);repair.querySelector('#preview-retranscription').click();check(calls.length===before+1,'remount has one callback');check(cancel(),'remount cancel guard');view.dispose();repair.remove();candidate.remove();
  });
  assert.deepEqual(errors,[]);console.log('PASS: real repair range/provider consent, current/window/expand, all candidate callbacks, adoption cancel guard, dispose and remount');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
