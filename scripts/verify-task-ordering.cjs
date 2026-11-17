const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const p=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];p.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(p,30);
  await p.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke,qa=window.__sortQA={order:Array.from({length:16},(_,i)=>'job-'+i),saves:[]};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    if(cmd==='reorder_jobs'){qa.saves.push(args.jobIds);qa.order=args.jobIds;return null;}
    const result=await invoke(cmd,args);
    if(cmd==='list_job_summaries')return qa.order.map((id,i)=>({...result[0],job_id:id,display_name:'节目 '+id+' · 电台与字幕 '+i}));
    return result;
   };
  });
  await p.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  await p.locator('#open-recent-jobs').click();await p.locator('[data-job-card]').nth(15).waitFor({state:'attached'});
  const order=()=>p.locator('[data-job-card]').evaluateAll(nodes=>nodes.map(e=>e.dataset.jobCard));
  const before=await order();
  const start=await p.locator('.job-drag-handle').first().boundingBox(),target=await p.locator('[data-job-card]').nth(2).boundingBox();
  await p.mouse.move(start.x+6,start.y+10);await p.mouse.down();await p.mouse.move(start.x+25,target.y+target.height-5,{steps:6});
  assert.equal(await p.locator('.job-drag-preview').count(),1);assert.equal(await p.locator('.job-drop-placeholder').count(),1);
  const ghost=await p.locator('.job-drag-preview').boundingBox();assert.ok(ghost.y>start.y);
  await p.screenshot({path:'/tmp/atogaki-task-drag.png'});
  await p.mouse.up();await p.waitForFunction(()=>window.__sortQA.saves.length===1);
  assert.notDeepEqual(await order(),before);
  assert.deepEqual(await order(),await p.evaluate(()=>window.__sortQA.order));
  assert.equal(await p.locator('.job-drag-preview,.job-drop-placeholder').count(),0);
  // Escape cancels the drag, restores the original order and leaves the sheet open.
  const saved=await order(),second=await p.locator('.job-drag-handle').first().boundingBox();
  await p.mouse.move(second.x+5,second.y+10);await p.mouse.down();await p.mouse.move(second.x+15,second.y+100,{steps:4});await p.keyboard.press('Escape');await p.mouse.up();
  assert.deepEqual(await order(),saved);assert.equal(await p.evaluate(()=>window.__sortQA.saves.length),1);
  assert.equal(await p.locator('#recent-jobs-drawer').evaluate(e=>e.open),true);
  // Only the drawer scrolls while dragging near its lower edge.
  const third=await p.locator('.job-drag-handle').first().boundingBox();
  await p.mouse.move(third.x+6,third.y+10);await p.mouse.down();await p.mouse.move(third.x+15,745,{steps:5});
  await p.waitForFunction(()=>document.querySelector('#recent-jobs-drawer').scrollTop>20);
  assert.equal(await p.evaluate(()=>document.scrollingElement.scrollTop),0);
  await p.keyboard.press('Escape');await p.mouse.up();
  await p.locator('.job-drag-handle').first().focus();await p.keyboard.press('ArrowDown');
  await p.waitForFunction(()=>window.__sortQA.saves.length===2);
  assert.deepEqual(await order(),await p.evaluate(()=>window.__sortQA.order));
  assert.equal(await p.evaluate(()=>window.__test.calls.some(call=>call.cmd==='get_job_detail')),false,'drag cannot open a task');
  assert.deepEqual(errors,[]);console.log('PASS: pointer ghost, insertion gap, persistent drop, Escape rollback, sheet autoscroll, keyboard ordering and no accidental task open.');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
