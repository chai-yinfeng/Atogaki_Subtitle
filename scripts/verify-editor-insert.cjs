const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),{stripTypeScriptTypes}=require('node:module');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:960,height:640}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke,state=window.__insert={calls:[],hold:true,release:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd!=='edit_subtitle_block_recorded')return invoke(cmd,args);
    state.calls.push(args.request);
    if(state.hold)await new Promise((resolve,reject)=>state.release=fail=>fail?reject(Error('fixture disk full')):resolve());
    const cues=window.__test.cues,r=args.request,before=structuredClone(window.__test.cues);
    if(r.insertion)cues.push(r.insertion);
    if(r.removeId){const index=cues.findIndex(cue=>cue.id===r.removeId);if(index<0)throw Error('fixture missing delete target');cues.splice(index,1);}
    cues.sort((a,b)=>a.start_ms-b.start_ms);cues.forEach((cue,i)=>cue.segment_index=i);
    return window.__test.recordStructure(before,r.removeId?'删除字幕块':'插入字幕块',r.jobId);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  const open=async()=>{await page.locator('.editor-menu > summary').click();await page.locator('#karaoke-insert-block').click();};
  const dialog=page.locator('#insert-subtitle-block-dialog'),start=dialog.locator('[name="start"]'),end=dialog.locator('[name="end"]'),source=dialog.locator('[name="source"]'),translation=dialog.locator('[name="translation"]'),submit=dialog.locator('[type="submit"]'),status=dialog.locator('[role="status"]');
  await open();assert.equal(await dialog.getAttribute('role'),'region');assert.equal(await dialog.evaluate(d=>d.matches(':modal')),false);
  assert.equal(await start.evaluate(e=>e===document.activeElement),true);assert.equal(await start.inputValue(),'00:00:02.800');assert.equal(await end.inputValue(),'00:00:03.000');
  await start.press('Shift+Tab');assert.equal(await submit.evaluate(e=>e===document.activeElement),true);await submit.press('Tab');assert.equal(await start.evaluate(e=>e===document.activeElement),true);
  const geometry=await dialog.boundingBox();assert.ok(geometry.y>=0&&geometry.y+geometry.height<=640);
  await source.fill('插入回归');await translation.fill('translation');await start.fill('00:00:00.100');await submit.click();assert.match(await status.innerText(),/不能与已有字幕重叠/);assert.equal(await page.evaluate(()=>window.__insert.calls.length),0);
  await start.fill('00:00:02.800');await source.dispatchEvent('compositionstart');await dialog.locator('form').dispatchEvent('submit');assert.match(await status.innerText(),/输入法/);await source.dispatchEvent('compositionend');
  await submit.click();await page.waitForFunction(()=>window.__insert.calls.length===1);assert.equal(await start.isDisabled(),true);await dialog.press('Escape');assert.equal(await dialog.isVisible(),true);await dialog.locator('form').dispatchEvent('submit');assert.equal(await page.evaluate(()=>window.__insert.calls.length),1);
  await page.evaluate(()=>window.__insert.release(true));await page.waitForFunction(()=>!document.querySelector('#insert-subtitle-block-dialog fieldset').disabled);assert.match(await status.innerText(),/disk full/);assert.equal(await source.inputValue(),'插入回归');assert.equal(await translation.inputValue(),'translation');
  await page.evaluate(()=>window.__insert.hold=false);await submit.click();await dialog.waitFor({state:'detached'});
  assert.equal(await page.evaluate(()=>window.__test.cues.length),31);assert.equal(await page.locator('#karaoke-current-source').inputValue(),'插入回归');assert.equal(await page.locator('.editor-menu > summary').evaluate(e=>e===document.activeElement),true,await page.evaluate(()=>({tag:document.activeElement?.tagName,id:document.activeElement?.id})));
  const insertedId=await page.evaluate(()=>window.__insert.calls.at(-1).insertion.id);
  const remove=async()=>{await page.locator('.editor-menu > summary').click();await page.locator('#karaoke-delete-block').click();await page.locator('#confirmation-dialog[open]').waitFor();};
  await remove();const confirmation=page.locator('#confirmation-dialog');assert.equal(await confirmation.getAttribute('role'),'region');assert.equal(await confirmation.getAttribute('aria-label'),'删除当前字幕块？');assert.equal(await confirmation.evaluate(d=>d.matches(':modal')),false);
  assert.equal(await page.locator('#cancel-confirmation').evaluate(e=>e===document.activeElement),true);
  await page.locator('#cancel-confirmation').press('Shift+Tab');assert.equal(await page.locator('#accept-confirmation').evaluate(e=>e===document.activeElement),true);
  await page.locator('#accept-confirmation').press('Tab');assert.equal(await page.locator('#cancel-confirmation').evaluate(e=>e===document.activeElement),true);
  await confirmation.press('Escape');assert.equal(await page.evaluate(()=>window.__test.cues.length),31);assert.equal(await confirmation.count(),0);assert.equal(await page.locator('.editor-menu > summary').evaluate(e=>e===document.activeElement),true);
  await remove();await page.locator('#accept-confirmation').click();await page.waitForFunction(()=>window.__test.cues.length===30);
  assert.equal(await page.evaluate(id=>window.__test.cues.some(cue=>cue.id===id),insertedId),false);
  await page.getByRole('button',{name:'撤销打轴',exact:true}).click();await page.waitForFunction(()=>window.__test.cues.length===31);
  assert.equal(await page.evaluate(id=>window.__test.cues.find(cue=>cue.id===id)?.source_text,insertedId),'插入回归');
  await open();await dialog.press('Escape');await dialog.waitFor({state:'detached'});assert.equal(await page.locator('.editor-menu > summary').evaluate(e=>e===document.activeElement),true);
  await open();await page.locator('[data-editor-action="back"]').evaluate(b=>b.click());await page.locator('.job-open').first().waitFor();assert.equal(await dialog.count(),0);
  const isolated=await browser.newPage();isolated.on('pageerror',e=>errors.push(String(e)));await isolated.setContent('<main><button id="trigger">open</button></main>');
  await isolated.evaluate(async sources=>{
   const load=source=>URL.createObjectURL(new Blob([source],{type:'text/javascript'}));const tools=load(sources.tools),view=load(sources.view.replace('"./task-tools"',JSON.stringify(tools)));
   const {mountInsertSubtitleBlock}=await import(view);URL.revokeObjectURL(view);URL.revokeObjectURL(tools);
   const state=window.__isolated={context:{jobId:'a',navigation:1},dead:false,release:null};
   state.view=mountInsertSubtitleBlock(document.querySelector('main'),{context(){if(state.dead)throw Error('disposed context');return state.context;},time:String,parse:v=>Number(v),overlaps:()=>false,submit:()=>new Promise(resolve=>state.release=resolve)});
   state.open=()=>state.view.open({startMs:1,endMs:2,durationMs:10});
  },{tools:stripTypeScriptTypes(fs.readFileSync('ui/src/task-tools.ts','utf8')),view:stripTypeScriptTypes(fs.readFileSync('ui/src/insert-subtitle-block.ts','utf8'))});
  const begin=async()=>{await isolated.evaluate(()=>window.__isolated.open());await isolated.locator('[name="source"]').fill('isolated');await isolated.locator('[type="submit"]').click();};
  await begin();await isolated.evaluate(()=>{const s=window.__isolated;s.old=s.release;s.view.cancel();s.open();});await isolated.locator('[name="source"]').fill('new visit');await isolated.evaluate(()=>window.__isolated.old(null));assert.equal(await isolated.locator('[name="source"]').inputValue(),'new visit');
  await isolated.evaluate(()=>window.__isolated.view.cancel());await begin();await isolated.evaluate(()=>{window.__isolated.context={jobId:'b',navigation:2};window.__isolated.release(null);});await isolated.locator('dialog').waitFor({state:'detached'});
  await begin();await isolated.evaluate(()=>{const s=window.__isolated;s.view.dispose();s.dead=true;s.release(null);});assert.equal(await isolated.locator('dialog').count(),0);assert.equal(await isolated.locator('#trigger').evaluate(e=>!!e.closest('[inert]')),false);
  assert.deepEqual(errors,[]);console.log('PASS: insert region/focus, validation/IME, busy/duplicate protection, failure retention/retry, delete cancel/confirm/undo, navigation, late results and disposal');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
