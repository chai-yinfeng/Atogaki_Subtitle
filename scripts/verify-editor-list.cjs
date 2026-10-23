const assert = require('node:assert/strict');
const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
(async()=>{
  const browser=await webkit.launch({headless:true});
  try {
    const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
    page.on('pageerror',error=>errors.push(String(error)));
    await require('./editor-ui-fixture.cjs').installEditorFixture(page,10000,20000);
    await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1430');
    await page.locator('.job-open').first().click();
    await page.locator('.editor-cue-row').first().waitFor();
    const mutations=await page.evaluate(async()=>{
      const host=document.querySelector('#editor-cue-list');
      const settle=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
      // Both viewport edges determine the range; avoid crossing the bottom
      // boundary when the panel height is not a multiple of the row height.
      const top=Array.from({length:26},(_,i)=>2801+i).find(t=>Math.ceil((t+host.clientHeight)/28)===Math.ceil((t+1+host.clientHeight)/28));
      host.scrollTop=top;await settle();
      const row=host.querySelector('[data-cue-id="cue-100"]');
      row.querySelector('button').focus({preventScroll:true});
      let changes=0;const details=[];
      const observer=new MutationObserver(records=>{changes+=records.length;details.push(...records.map(r=>({type:r.type,attribute:r.attributeName,target:r.target.className})));});
      observer.observe(host,{subtree:true,attributes:true,childList:true,characterData:true});
      host.scrollTop=top+1;await settle();
      const withinRow=changes;changes=0;
      observer.disconnect();observer.observe(row,{subtree:true,attributes:true,childList:true,characterData:true});
      host.scrollTop=top+28;await settle();observer.disconnect();
      return {withinRow,retainedRow:changes,details,sameNode:row===host.querySelector('[data-cue-id="cue-100"]'),
        focusRetained:document.activeElement===row.querySelector('button'),rows:host.querySelectorAll('.editor-cue-row').length};
    });
    assert.equal(mutations.withinRow,0,JSON.stringify(mutations));assert.equal(mutations.retainedRow,0,JSON.stringify(mutations));
    assert.equal(mutations.sameNode,true);assert.equal(mutations.focusRetained,true);assert.ok(mutations.rows<40);
    // Playback/selection and explicit text updates must invalidate their own rows
    // even when the scroll window itself has not changed.
    await page.locator('[data-cue-id="cue-100"]').click();
    assert.equal(await page.locator('[data-cue-id="cue-100"]').getAttribute('aria-selected'),'true');
    await page.locator('#karaoke-current-source').fill('局部保存仍更新可见摘要');
    await page.locator('#karaoke-save-text').click();
    await page.waitForFunction(()=>document.querySelector('[data-cue-id="cue-100"] .cue-source')?.textContent==='局部保存仍更新可见摘要');
    await page.locator('#editor-search').fill('局部保存仍更新可见摘要');
    assert.equal(await page.locator('.editor-cue-row').count(),1);
    assert.equal(await page.locator('.editor-cue-row').getAttribute('aria-rowindex'),'1');
    assert.equal(await page.locator('.editor-cue-row').evaluate(row=>row.offsetTop),0);
    await page.locator('#editor-search').fill('没有匹配的文字');
    assert.equal(await page.locator('.editor-cue-row').count(),0);
    await page.locator('#editor-search').fill('');
    await page.locator('#editor-cue-list').focus();await page.keyboard.press('End');
    assert.equal(await page.locator('[data-cue-id="cue-9999"]').getAttribute('aria-selected'),'true');
    assert.ok(await page.locator('.editor-cue-row').count()<40);
    await page.locator('[data-editor-action="back"]').click();
    await page.locator('.job-open').first().click();
    await page.locator('.editor-cue-row').first().waitFor();
    assert.deepEqual(errors,[]);
    console.log('PASS: pixel scroll does not mutate rows; retained focus, local save, filtering, empty restore, End and remount',mutations);
  } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
