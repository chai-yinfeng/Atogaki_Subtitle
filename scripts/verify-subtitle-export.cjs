const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page);
  await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1430');
  await page.locator('.job-open').first().click();
  await page.evaluate(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__exportTest={existing:[],fail:false,held:false,pending:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args)=>{
    if(!['pick_subtitle_export_directory','preview_workspace_subtitle_export','export_workspace_subtitles','reveal_exported_subtitle'].includes(cmd))return invoke(cmd,args);
    window.__test.calls.push({cmd,args});
    const state=window.__exportTest;
    if(cmd==='pick_subtitle_export_directory')return state.held ? new Promise(resolve=>state.pending=resolve) : '/qa/export';
    if(cmd==='preview_workspace_subtitle_export')return {existing_files:state.existing};
    if(cmd==='export_workspace_subtitles'){
     if(state.fail)throw Error('isolated export failure');
     return {source_srt:'/qa/export/source.srt',translated_srt:'/qa/export/translated.srt',bilingual_srt:'/qa/export/bi.srt',bilingual_ass:'/qa/export/bi.ass',missing_translation_count:0};
    }
   };
  });
  assert.equal(await page.locator('#subtitle-export-dialog').count(),0);
  await page.locator('[data-editor-action="export"]').click();
  const start=page.locator('#export-subtitles'),dialog=page.locator('#subtitle-export-dialog');
  await start.click();await dialog.waitFor();
  assert.equal(await dialog.getAttribute('role'),'region');assert.equal(await dialog.evaluate(e=>e.inert),false);
  assert.equal(await page.locator('#workspace-view').evaluate(e=>e.inert),true);
  for(const checkbox of await dialog.locator('input').all())await checkbox.uncheck();
  await dialog.locator('[type="submit"]').click();assert.match(await page.locator('#subtitle-export-message').innerText(),/至少选择/);
  await page.locator('#cancel-subtitle-export').focus();await page.keyboard.press('Shift+Tab');assert.equal(await page.evaluate(()=>document.activeElement.type),'submit');
  await page.keyboard.press('Tab');assert.equal(await page.evaluate(()=>document.activeElement.id),'cancel-subtitle-export');
  await page.keyboard.press('Escape');assert.equal(await dialog.count(),0);
  assert.equal(await page.evaluate(()=>document.activeElement.id),'export-subtitles');
  assert.equal(await page.locator('#workspace-view').evaluate(e=>e.inert),false);
  await start.click();await dialog.locator('[value="bilingual_srt"]').check();await dialog.locator('[type="submit"]').click();
  assert.match(await page.locator('#workspace-action-message').innerText(),/缺失/);
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(r=>r.cmd==='pick_subtitle_export_directory').length),0);
  const sourceOnly=async()=>{await start.click();for(const input of await dialog.locator('input').all())await input.uncheck();await dialog.locator('[value="source_srt"]').check();await dialog.locator('[type="submit"]').click();};
  await sourceOnly();await page.waitForFunction(()=>document.querySelector('#workspace-action-message').textContent.includes('已导出 1'));
  assert.equal(await page.locator('#reveal-export').isVisible(),true);await page.locator('#reveal-export').click();
  assert.deepEqual(await page.evaluate(()=>window.__test.calls.find(r=>r.cmd==='export_workspace_subtitles').args.request),{jobId:'editor-test',outputDirectory:'/qa/export',overwriteExisting:false,artifacts:['source_srt']});
  assert.equal(await page.evaluate(()=>window.__test.calls.find(r=>r.cmd==='reveal_exported_subtitle').args.path),'/qa/export/source.srt');
  await page.evaluate(()=>window.__exportTest.existing=['/qa/export/<unsafe>.srt']);
  const writes=await page.evaluate(()=>window.__test.calls.filter(r=>r.cmd==='export_workspace_subtitles').length);
  await sourceOnly();const overwrite=page.locator('#subtitle-export-overwrite-dialog');await overwrite.waitFor();
  assert.equal(await overwrite.getAttribute('role'),'region');assert.equal(await overwrite.locator('[data-overwrite-files]').textContent(),'<unsafe>.srt');assert.equal(await overwrite.locator('unsafe').count(),0);
  await overwrite.locator('[data-cancel-overwrite]').click();await start.waitFor({state:'visible'});await page.waitForFunction(()=>!document.querySelector('#export-subtitles').disabled);
  assert.equal(await page.evaluate(()=>document.activeElement.id),'export-subtitles');
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(r=>r.cmd==='export_workspace_subtitles').length),writes);
  await sourceOnly();await overwrite.waitFor();await overwrite.locator('[data-accept-overwrite]').click();
  await page.waitForFunction(n=>window.__test.calls.filter(r=>r.cmd==='export_workspace_subtitles').length===n+1,writes);
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(r=>r.cmd==='export_workspace_subtitles').at(-1).args.request.overwriteExisting),true);
  await page.evaluate(()=>{window.__exportTest.existing=[];window.__exportTest.fail=true;});
  await sourceOnly();await page.waitForFunction(()=>document.querySelector('#workspace-action-message').textContent.includes('isolated export failure'));assert.equal(await start.isEnabled(),true);
  for(let i=0;i<3;i++){await start.click();await page.locator('#cancel-subtitle-export').click();assert.equal(await dialog.count(),0);}
  await page.locator('#back-to-jobs').click();
  assert.equal(await page.locator('#karaoke-current-source').isVisible(),true);
  assert.equal(await page.locator('#subtitle-export-overwrite-dialog').count(),0);
  assert.deepEqual(errors,[]);console.log('PASS: export mount/focus, formats, translation guard, source-only wire values, overwrite/cancel, safe filenames, failures and repeated close');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
