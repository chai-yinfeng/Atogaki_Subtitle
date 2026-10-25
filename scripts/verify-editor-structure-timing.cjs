const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const base=window.__TAURI_INTERNALS__.invoke,test=window.__test;
   test.cues[0].end_ms=3000;
   window.__structure={calls:[],hold:false,release:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const s=window.__structure;
    if(cmd==='update_subtitle'&&s.hold){s.hold=false;await new Promise(resolve=>s.release=resolve);}
    if(['split_subtitle','merge_subtitles','edit_subtitle_block','restore_subtitle_structure'].includes(cmd)){
     s.calls.push(cmd);const r=args.request;
     if(cmd==='split_subtitle')test.cues.splice(0,1,{...test.cues[0],end_ms:r.boundaryMs},{...test.cues[0],id:'split-right',start_ms:r.boundaryMs});
     if(cmd==='restore_subtitle_structure')test.cues.splice(0,test.cues.length,...r.beforeSegments);
     return test.cues.map(c=>({...c}));
    }
    return base(cmd,args);
   };
  });
  const url=process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430';
  const start=page.locator('#editor-cue-start'),notice=page.locator('#karaoke-timing-message');
  const open=async()=>{
   await page.goto(url);await page.locator('.job-open').first().click();
   await page.locator('#karaoke-media-host audio').evaluate(async a=>{if(a.readyState<1)await new Promise(r=>a.addEventListener('loadedmetadata',r,{once:true}));await new Promise(r=>{a.addEventListener('seeked',r,{once:true});a.currentTime=1;});a.dispatchEvent(new Event('timeupdate'));});
  };
  const action=async name=>{
   if(name==='undo'){await page.getByRole('button',{name:'撤销打轴',exact:true}).click();return;}
   if(name!=='cut')await page.locator('.editor-menu > summary').click();
   await page.locator('#karaoke-'+({cut:'cut-segment',join:'join-segment',insert:'insert-block',delete:'delete-block'}[name])).click();
  };
  for(const name of ['cut','join','insert','delete','undo']){
   await open();
   if(name==='undo'){await start.fill('00:00:00.050');await page.locator('[data-editor-action="save-time"]').click();await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('时间已保存'));}
   await start.fill('00:00:00.100');await action(name);
   await page.waitForFunction(()=>/先保存时间/.test(document.querySelector('#karaoke-timing-message').textContent)||window.__structure.calls.length||document.querySelector('dialog[open]'));
   assert.equal(await page.evaluate(()=>window.__structure.calls.length),0,`${name} must preserve unsaved timing`);
   assert.equal(await page.locator('dialog[open]').count(),0,`${name} must not open a destructive dialog over pending time`);
   assert.equal(await start.inputValue(),'00:00:00.100');assert.match(await notice.innerText(),/先保存时间/);
  }
  await open();await start.fill('00:00:00.100');
  const canvas=page.locator('#karaoke-waveform'),box=await canvas.boundingBox();
  const view=await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_waveform_window').at(-1).args.request);
  const edge=box.x+(3000-view.startMs)/(view.endMs-view.startMs)*box.width;
  const trackTop=28+Math.max(12,(box.height-56)*.5)+(box.height<100?6:14),trackY=box.y+(trackTop+box.height-4)/2;
  await page.mouse.move(edge-2,trackY);await page.mouse.down();await page.mouse.move(edge-17,trackY,{steps:2});await page.mouse.up();
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='save_subtitle_timing').length),0);
  assert.equal(await start.inputValue(),'00:00:00.100');assert.match(await notice.innerText(),/先保存时间/);
  await open();await page.evaluate(()=>window.__structure.hold=true);
  await page.locator('#karaoke-current-source').fill('structure flush text');await action('cut');
  await page.waitForFunction(()=>window.__structure.release);await start.fill('00:00:00.200');await page.evaluate(()=>window.__structure.release());
  await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('先保存时间'));
  assert.equal(await page.evaluate(()=>window.__structure.calls.length),0);assert.equal(await start.inputValue(),'00:00:00.200');
  await start.press('Escape');await action('cut');await page.waitForFunction(()=>window.__structure.calls.length===1);
  assert.equal(await page.evaluate(()=>window.__structure.calls[0]),'split_subtitle');
  const candidate=await browser.newPage({viewport:{width:1180,height:760}});candidate.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-processing-fixture.cjs').installProcessingFixture(candidate);
  await candidate.goto(url);await candidate.locator('.job-open').first().click();
  await candidate.locator('#editor-cue-start').fill('00:00:00.100');
  await candidate.locator('[data-editor-action="repair"]').click();await candidate.locator('#preview-retranscription').click();await candidate.locator('#retranscription-dialog[open]').waitFor();
  await candidate.locator('#confirm-retranscription').click();
  await candidate.waitForFunction(()=>document.querySelector('#retranscription-confirm-message').textContent.includes('先保存时间'));
  assert.equal(await candidate.evaluate(()=>window.__test.adopted.length),0);
  await candidate.locator('#close-retranscription-preview').click();await candidate.locator('[data-editor-action="close-repair"]').click();
  assert.equal(await candidate.locator('#editor-cue-start').inputValue(),'00:00:00.100');
  await candidate.locator('#editor-cue-start').press('Escape');
  await candidate.locator('[data-editor-action="repair"]').click();await candidate.locator('#preview-retranscription').click();await candidate.locator('#retranscription-dialog[open]').waitFor();
  await candidate.locator('#confirm-retranscription').click();await candidate.locator('#retranscription-dialog').waitFor({state:'hidden'});
  assert.equal(await candidate.evaluate(()=>window.__test.adopted.length),1);
  assert.deepEqual(errors,[]);
  console.log('PASS: pending timing blocks Cut/Join/insert/delete/undo, timeline drag and candidate adoption; late input during flush survives, Escape permits Cut and adoption retries');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
