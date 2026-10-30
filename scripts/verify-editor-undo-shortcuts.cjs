const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE), assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:732}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1438');
  await page.locator('.job-open').first().click();await page.locator('.editor-cue-row').first().click();
  const calls=()=>page.evaluate(()=>window.__test.calls.filter(c=>c.cmd==='undo_subtitle_structure').length);
  const saveTime=async(endMs=2700)=>{
   await page.locator('#editor-cue-end').fill(`00:00:02.${String(endMs-2000).padStart(3,'0')}`);
   await page.locator('[data-editor-action="save-time"]').click();
   await page.waitForFunction(endMs=>!document.querySelector('#undo-subtitle-structure').disabled&&window.__test.cues[0].end_ms===endMs,endMs);
  };
  for(const key of ['Control+z','Meta+z']){
   await saveTime();await page.locator('.editor-cue-row').first().focus();await page.keyboard.press(key);
   await page.waitForFunction(()=>window.__test.cues[0].end_ms===2800);
  }
  assert.equal(await calls(),2);
  await saveTime();
  const input=page.locator('#karaoke-current-source'),original=await input.inputValue();
  for(const key of ['Control+z','Meta+z']){
   await input.focus();await page.keyboard.press('End');await page.keyboard.type(' undo-test');
   assert.notEqual(await input.inputValue(),original);
   await page.keyboard.press(key);
   await page.waitForFunction(original=>document.querySelector('#karaoke-current-source').value===original,original);
   assert.equal(await calls(),2);
   assert.equal(await page.evaluate(()=>window.__test.cues[0].end_ms),2700);
  }
  // Text autosave can advance revisions; use a fresh timing record for this check.
  await saveTime(2600);
  await input.focus();
  // Clicking the timeline must transfer focus out of the textarea before undo.
  const canvas=page.locator('#karaoke-waveform'),box=await canvas.boundingBox();
  await page.mouse.click(box.x+box.width*.5,box.y+40);
  assert.equal(await page.evaluate(()=>document.activeElement.id),'karaoke-waveform');
  await page.keyboard.press('Control+z');await page.waitForFunction(()=>window.__test.cues[0].end_ms===2700);
  assert.equal(await calls(),3);
  await saveTime(2600);await page.locator('[data-editor-action="repair"]').click();
  await page.keyboard.press('Control+z');await page.keyboard.press('Meta+z');assert.equal(await calls(),3);
  await page.locator('[data-editor-action="close-repair"]').click();
  await page.locator('.editor-cue-row').first().focus();await page.keyboard.press('Control+Shift+z');assert.equal(await calls(),3);
  assert.deepEqual(errors,[]);
  console.log('PASS: Ctrl/Command+Z use native text undo in inputs, structure undo on rows/canvas, transfer focus on timeline clicks, and do not undo the main editor from dialogs');
 } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1});
