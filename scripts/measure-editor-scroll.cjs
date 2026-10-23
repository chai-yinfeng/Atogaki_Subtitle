// Production UI, synthetic IPC only. Small scroll deltas complement the large
// jumps in measure-editor-webkit.cjs; RAF work is not a CPU profiler or JS heap.
const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const { installEditorFixture } = require('./editor-ui-fixture.cjs');
const fs = require('node:fs'), path = require('node:path');
const output = process.env.ATOGAKI_EDITOR_QA_OUTPUT || '/tmp/atogaki-editor-performance';
const label = process.env.ATOGAKI_MEASUREMENT_LABEL || 'scroll';
const percentile = (values, fraction) => [...values].sort((a,b)=>a-b)[Math.floor(values.length*fraction)];
(async()=>{
  const browser = await webkit.launch({headless:true}), results = [];
  try {
    for (const count of [300,3000,10000]) {
      const page = await browser.newPage({viewport:{width:1180,height:760}}), errors=[];
      page.on('pageerror', error=>errors.push(String(error)));
      await installEditorFixture(page,count,10800000);
      await page.addInitScript(()=>{
        const raf=window.requestAnimationFrame.bind(window);
        window.__rafWork=[];
        window.requestAnimationFrame=callback=>raf(time=>{
          const start=performance.now();
          try { callback(time); } finally { window.__rafWork.push(performance.now()-start); }
        });
      });
      await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1430');
      await page.locator('.job-open').first().click();
      await page.locator('.editor-cue-row').first().waitFor();
      const rounds=[];
      for (let round=0;round<3;round++) rounds.push(await page.evaluate(async()=>{
        const host=document.querySelector('#editor-cue-list');
        host.scrollTop=2800;
        const settle=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
        await settle();
        let attributes=0,addedRows=0,removedRows=0,maxRows=0;
        const observer=new MutationObserver(records=>{
          for(const record of records) {
            if(record.type==='attributes')attributes++;
            else {
              addedRows+=[...record.addedNodes].filter(n=>n.nodeType===1 && n.matches('.editor-cue-row')).length;
              removedRows+=[...record.removedNodes].filter(n=>n.nodeType===1 && n.matches('.editor-cue-row')).length;
            }
          }
        });
        observer.observe(host,{subtree:true,attributes:true,childList:true});
        window.__rafWork=[];
        const intervals=[];let previous=performance.now();
        // 7 px/frame advances by a quarter of the fixed row height.
        for(let i=1;i<=180;i++) {
          host.scrollTop=2800+i*7;
          await new Promise(requestAnimationFrame);
          const now=performance.now();intervals.push(now-previous);previous=now;
          maxRows=Math.max(maxRows,host.querySelectorAll('.editor-cue-row').length);
        }
        await settle();observer.disconnect();
        return {attributes,addedRows,removedRows,maxRows,intervals,rafWork:window.__rafWork};
      }));
      results.push({count,errors,rounds:rounds.map(({intervals,rafWork,...rest})=>({...rest,
        frameP95Ms:percentile(intervals,.95),framesOver50Ms:intervals.filter(t=>t>50).length,
        rafCallbackP95Ms:percentile(rafWork,.95),rafCallbackTotalMs:rafWork.reduce((a,b)=>a+b,0)}))});
      console.log(JSON.stringify(results.at(-1)));
      await page.close();
    }
    fs.mkdirSync(output,{recursive:true});
    fs.writeFileSync(path.join(output,`${label}-scroll.json`),JSON.stringify({label,results},null,2)+'\n');
  } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
