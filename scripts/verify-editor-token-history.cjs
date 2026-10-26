const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  const open=async()=>{await page.locator('.job-open').first().click();await page.locator('#karaoke-current-source').waitFor();};
  await open();
  await page.waitForFunction(()=>window.__test.calls.filter(c=>c.cmd==='get_structure_edit_state').length===1);
  const undo=page.getByRole('button',{name:'撤销打轴',exact:true}),start=page.locator('#editor-cue-start'),save=page.locator('[data-editor-action="save-time"]');
  const saved=async()=>{await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('时间已保存'));};
  await start.fill('00:00:00.100');await save.click();await saved();assert.equal(await undo.isDisabled(),false);
  await page.locator('[data-editor-action="back"]').click();await open();
  await page.waitForFunction(()=>!document.querySelector('#undo-subtitle-structure')?.disabled||document.querySelector('button[title="撤销调整时间"]'));
  assert.equal(await undo.isDisabled(),false,'reopening must read the backend session token');
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='get_structure_edit_state').length),2,'read state once per editor entry; never for the intermediate task view');
  await undo.click();await page.waitForFunction(()=>window.__test.cues[0].start_ms===0);
  assert.equal(await start.inputValue(),'00:00:00.000');assert.equal(await undo.isDisabled(),true);
  await start.fill('00:00:00.100');await save.click();await saved();
  await page.locator('#karaoke-current-source').fill('token invalidation after automatic text save');
  await page.waitForFunction(()=>window.__test.cues[0].source_text==='token invalidation after automatic text save');
  assert.equal(await undo.isDisabled(),true,'text save must invalidate the old structure descriptor');
  await page.evaluate(()=>window.__test.structureWarnings=['辅助候选标记失败，字幕已保存']);
  await start.fill('00:00:00.200');await save.click();await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('辅助候选标记失败'));
  assert.equal(await undo.isDisabled(),false);
  await page.evaluate(()=>{window.__test.structureWarnings=[];window.__test.structureOversized=true;});
  await start.fill('00:00:00.300');await save.click();await page.waitForFunction(()=>document.querySelector('#karaoke-timing-message').textContent.includes('超过撤销内存预算'));
  assert.equal(await undo.isDisabled(),true);assert.equal(await page.evaluate(()=>window.__test.cues[0].start_ms),300);
  assert.match(await page.locator('#karaoke-timing-message').innerText(),/修改已保存.*无法撤销/);
  const calls=await page.evaluate(()=>window.__test.calls);
  assert.equal(calls.filter(c=>['restore_subtitle_structure','save_subtitle_timing','split_subtitle','merge_subtitles','edit_subtitle_block','confirm_retranscription'].includes(c.cmd)).length,0);
  const restores=calls.filter(c=>c.cmd==='undo_subtitle_structure');assert.equal(restores.length,1);assert.deepEqual(Object.keys(restores[0].args.request).sort(),['jobId','token']);
  assert.deepEqual(errors,[]);
  console.log('PASS: backend token reload, token-only undo, text-save invalidation, committed warnings and oversized saved-without-undo presentation');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
