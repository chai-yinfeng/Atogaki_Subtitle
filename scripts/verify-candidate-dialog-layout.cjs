const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  for(const [width,height,large] of [[1180,760,false],[960,640,false],[960,640,true]]){
   const page=await browser.newPage({viewport:{width,height}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
   await require('./editor-processing-fixture.cjs').installProcessingFixture(page);
   await page.addInitScript(()=>{const base=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(cmd,args)=>{const result=await base(cmd,args);if(cmd==='preview_retranscription'){result.original_segments=window.__test.cues.map(c=>({...c,source_text:c.source_text.repeat(8)}));result.candidate_segments=result.original_segments.map(c=>({...c,source_text:'候选 '+c.source_text}));result.configuration={note:'长参数 '.repeat(1000)};}return result;};});
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();await page.locator('[data-editor-action="repair"]').click();await page.locator('#preview-retranscription').click();await page.locator('#retranscription-dialog[open]').waitFor();
   if(large)await page.addStyleTag({content:'#retranscription-dialog,#retranscription-dialog button,#retranscription-dialog p,#retranscription-dialog summary {font-size:20px!important}'});
   const geometry=()=>page.locator('#retranscription-dialog').evaluate(root=>{
    const box=e=>{const r=e.getBoundingClientRect();return{top:r.top,bottom:r.bottom,left:r.left,right:r.right,width:r.width,height:r.height};};
    const note=[...root.querySelectorAll('p')].find(p=>p.textContent.startsWith('确认后只替换')),footer=root.querySelector('.confirmation-actions'),body=root.querySelector('.candidate-comparison-body');
    return {root:box(root),note:box(note),footer:box(footer),body:body?box(body):null,viewport:{width:innerWidth,height:innerHeight},overflow:root.scrollWidth-root.clientWidth};
   });
   let g=await geometry();console.log(JSON.stringify({width,height,large,initial:g}));
   // The warning must be reachable in full, without the pinned action row covering it.
   const note=page.locator('#retranscription-dialog p').filter({hasText:'确认后只替换所示范围'});await note.scrollIntoViewIfNeeded();
   g=await geometry();assert.ok(g.root.top>=0&&g.root.bottom<=height,'dialog fits viewport');assert.ok(g.overflow<=1,'no horizontal clipping');assert.ok(g.note.top>=Math.max(g.root.top,g.body?.top??g.root.top)&&g.note.bottom<=Math.min(g.footer.top,g.body?.bottom??g.footer.top)+1,'replacement explanation visible above footer');
   await page.locator('.candidate-diff summary').click();
   await page.locator('.candidate-diff pre').scrollIntoViewIfNeeded();
   await page.evaluate(()=>document.querySelector('#retranscription-confirm-message').textContent='采用冲突：字幕已修改，请重新对照。'.repeat(10));
   await page.locator('#retranscription-confirm-message').scrollIntoViewIfNeeded();
   await page.locator('.candidate-comparison-body').focus();await page.keyboard.press('End');await page.waitForFunction(()=>document.querySelector('.candidate-comparison-body').scrollTop>0);
   g=await geometry();assert.ok(g.footer.top>=0&&g.footer.bottom<=height,'actions stay visible with expanded details and errors');
   const status=await page.locator('#retranscription-confirm-message').boundingBox();assert.ok(status.y>=g.body.top-1&&status.y+status.height<=g.footer.top+1,'full status is scrollable above actions');
   await page.locator('#confirm-retranscription').focus();await page.keyboard.press('Tab');assert.equal(await page.evaluate(()=>document.activeElement.id),'close-retranscription-preview');
   await page.locator('#retranscription-original-list').evaluate(e=>e.scrollTop=e.scrollHeight);assert.ok(await page.locator('#retranscription-original-list').evaluate(e=>e.scrollTop)>0,'long original list independently scrolls');
   const output=process.env.ATOGAKI_EDITOR_QA_OUTPUT;if(output){fs.mkdirSync(output,{recursive:true});await page.screenshot({path:path.join(output,`candidate-${width}-${height}-${large?'large':'normal'}.png`)});}
   await page.keyboard.press('Escape');assert.equal(await page.locator('#retranscription-dialog').count(),0);assert.deepEqual(errors,[]);await page.close();
  }
  console.log('PASS: candidate warning/status unobscured, long list/config scroll, pinned actions, focus wrap and close at standard/small/large-font sizes');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
