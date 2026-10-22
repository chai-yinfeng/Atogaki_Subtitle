const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE),assert=require('node:assert/strict');
const {installProcessingFixture}=require('./editor-processing-fixture.cjs');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await installProcessingFixture(page);await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  await page.evaluate(()=>{
   const signal={id:'signal',job_id:'editor-test',run_id:'original',signal_kind:'boundary_repetition',severity:'warning',start_ms:3000,end_ms:5800,message:'提示 <button>文本</button>',latest_decision:null,latest_decision_note:null};
   window.__test.signals=[signal,{...signal,id:'resolved',message:'已解决提示',latest_decision:'resolved'}];
  });
  const open=()=>page.locator('[data-editor-action="repair"]').click(),close=()=>page.locator('[data-editor-action="close-repair"]').click();
  await open();await page.locator('#asr-quality-review > summary').click();await page.locator('[data-repair-signal="signal"]').waitFor();
  assert.equal(await page.locator('.asr-quality-signal').count(),1);assert.equal(await page.locator('.asr-quality-message').textContent(),'提示 <button>文本</button>');
  const reads=await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='list_asr_quality_signals').length);
  await page.locator('[data-show-processed]').check();assert.equal(await page.locator('.asr-quality-signal').count(),2);
  assert.equal(await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='list_asr_quality_signals').length),reads);
  await page.locator('[data-repair-signal="signal"]').click();assert.equal(await page.locator('#retranscription-start').inputValue(),'00:00:03.000');
  await page.locator('[data-asr-review="false_positive"][data-signal-id="signal"]').click();await page.waitForFunction(()=>document.querySelector('#asr-quality-summary').textContent.includes('0 项待复核'));
  await page.locator('[data-show-processed]').uncheck();assert.equal(await page.locator('.asr-quality-signal').count(),0);
  await page.locator('[data-show-processed]').check();await page.locator('.reviewed-signal').first().locator('summary').click();await page.locator('[data-locate-signal="signal"]').click();
  assert.equal(await page.locator('#editor-repair-dialog').count(),0);assert.ok(await page.locator('#karaoke-current-source').inputValue());
  await open();assert.equal(await page.locator('[data-full-candidate]').count(),1);
  await page.locator('#preview-retranscription').click();await page.locator('#retranscription-dialog[open]').waitFor();await page.locator('#close-retranscription-preview').click();
  await page.locator('[data-candidate-history]').click();await page.locator('[data-release-run]').waitFor();
  await page.locator('[data-release-run]').click();await page.locator('#confirmation-dialog[open]').waitFor();assert.equal(await page.locator('#confirmation-dialog').evaluate(d=>d.inert),false);
  await page.locator('#cancel-confirmation').click();assert.equal(await page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='release_asr_candidate').length),0);
  await page.locator('[data-release-run]').click();await page.locator('#accept-confirmation').click();await page.waitForFunction(()=>document.querySelector('.candidate-history')?.textContent.includes('产物已清理'));
  assert.equal(await page.locator('[data-release-run]').count(),0);assert.match(await page.locator('#retranscription-message').textContent(),/1 KiB/);
  // Out-of-order history reads must keep the latest list.
  await page.evaluate(()=>{window.__test.held.history=true;const r=Object.values(window.__test.previews)[0];window.__test.historyRows=[{id:'old',provider_name:'旧返回',model_identity:'test',scope_start_ms:0,scope_end_ms:2800,created_at_unix:1,status:'failed'}]});
  await page.locator('[data-candidate-history]').click();await page.waitForFunction(()=>window.__test.pending.filter(r=>r.kind==='history').length===1);
  await page.evaluate(()=>window.__test.historyRows[0].provider_name='新返回');await page.locator('[data-candidate-history]').click();await page.waitForFunction(()=>window.__test.pending.filter(r=>r.kind==='history').length===2);
  await page.evaluate(()=>window.__test.pending.filter(r=>r.kind==='history')[1].resolve());await page.waitForFunction(()=>document.querySelector('.candidate-history').textContent.includes('新返回'));
  await page.evaluate(()=>window.__test.pending.filter(r=>r.kind==='history')[0].resolve());await page.waitForTimeout(50);assert.match(await page.locator('.candidate-history').textContent(),/新返回/);
  // Closing clears all row DOM; a late read must not resurrect it after reopen.
  await page.locator('[data-candidate-history]').click();await page.waitForFunction(()=>window.__test.pending.filter(r=>r.kind==='history').length===3);await close();
  await page.evaluate(()=>window.__test.pending.filter(r=>r.kind==='history')[2].resolve());await open();assert.equal(await page.locator('.candidate-history').textContent(),'');assert.equal(await page.locator('[data-full-candidate]').count(),1);
  // Full-candidate completion while parked must re-enable retained controls.
  await page.evaluate(()=>window.__test.held.preview=true);await page.locator('[data-full-candidate]').click();await page.waitForFunction(()=>window.__test.pending.some(r=>r.kind==='preview'));await close();
  await page.evaluate(()=>window.__test.pending.find(r=>r.kind==='preview').resolve());await page.waitForTimeout(50);await open();
  assert.equal(await page.locator('[data-full-candidate]').isDisabled(),false);assert.equal(await page.locator('#retranscription-dialog').count(),0);
  // A completed release belongs to the originating task even after navigation.
  await page.evaluate(()=>{window.__test.held.history=false;window.__test.held.release=true;window.__test.historyRows=[{id:'late',provider_name:'迟到清理',model_identity:'test',scope_start_ms:0,scope_end_ms:2800,created_at_unix:1,status:'succeeded',cleanable:true,reopenable:true}]});
  await page.locator('[data-candidate-history]').click();await page.locator('[data-release-run="late"]').click();await page.locator('#accept-confirmation').click();await page.waitForFunction(()=>window.__test.pending.some(r=>r.kind==='release'));
  assert.equal(await page.locator('[data-reopen-run="late"]').isDisabled(),true);await close();await page.locator('[data-editor-action="back"]').click();await page.locator('.job-open').nth(1).click();
  await page.waitForFunction(()=>document.querySelector('#karaoke-current-source').value.startsWith('任务 B 第'));const bText=await page.locator('#karaoke-current-source').inputValue();
  await page.evaluate(()=>window.__test.pending.find(r=>r.kind==='release').resolve());await page.waitForTimeout(50);assert.equal(await page.locator('#karaoke-current-source').inputValue(),bText);
  await open();assert.equal(await page.locator('.candidate-history').textContent(),'');
  assert.deepEqual(errors,[]);console.log('PASS: review filter/range/decision/locate, cleanup confirmation, latest history, parked controls, late release and close/reopen lifecycle');
 } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
