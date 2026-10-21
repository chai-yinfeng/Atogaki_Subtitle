// Run against a production build served locally; no user media or real IPC.
const { webkit } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const { installEditorFixture } = require('./editor-ui-fixture.cjs');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const output = process.env.ATOGAKI_EDITOR_QA_OUTPUT || '/tmp/atogaki-editor-performance';
const label = process.env.ATOGAKI_MEASUREMENT_LABEL || 'measurement';
fs.mkdirSync(output,{recursive:true});
function rss(pid) {
  const rows=execFileSync('ps',['-axo','pid=,ppid=,rss='],{encoding:'utf8'}).trim().split('\n').map(line=>line.trim().split(/\s+/).map(Number));
  const ids=new Set([pid]);let changed=true;
  while(changed){changed=false;for(const [id,parent] of rows)if(ids.has(parent)&&!ids.has(id)){ids.add(id);changed=true;}}
  return rows.filter(([id])=>ids.has(id)).reduce((sum,row)=>sum+row[2],0);
}
(async()=>{
  const server=await webkit.launchServer({headless:true});
  const browser=await webkit.connect(server.wsEndpoint());
  const results=[];
  try {
    for (const count of [300,3000,10000]) {
      const context=await browser.newContext({viewport:{width:1180,height:760}});
      await context.tracing.start({screenshots:true,snapshots:true,sources:true});
      const page=await context.newPage(),errors=[];
      page.on('pageerror',e=>errors.push(String(e)));
      await installEditorFixture(page,count,({300:600000,3000:1800000,10000:10800000})[count]);
      await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1430');
      const started=performance.now();
      await page.locator('.job-open').first().click();
      await page.locator('#editor-cue-list .editor-cue-row').first().waitFor();
      const firstInteractiveMs=performance.now()-started;
      const measurements=await page.evaluate(async()=>{
        const host=document.querySelector('#editor-cue-list');
        const frames=[],domCounts=[];let previous=performance.now();
        for(let i=0;i<90;i++) {
          host.scrollTop=(host.scrollHeight-host.clientHeight)*(i%45)/44;
          await new Promise(requestAnimationFrame);
          const now=performance.now();frames.push(now-previous);previous=now;
          domCounts.push(host.querySelectorAll('.editor-cue-row').length);
        }
        frames.sort((a,b)=>a-b);
        const timed=(element,event)=>{const started=performance.now();element.dispatchEvent(new Event(event,{bubbles:true}));return performance.now()-started;};
        const input=document.querySelector('#karaoke-current-source');input.value='性能测试：只修改当前句';
        const inputHandlerMs=timed(input,'input');
        const search=document.querySelector('#editor-search');search.value='今天';
        const searchHandlerMs=timed(search,'input');search.value='';timed(search,'input');
        const navigationStart=performance.now();host.focus();host.dispatchEvent(new KeyboardEvent('keydown',{key:'End',bubbles:true}));
        const lateNavigationMs=performance.now()-navigationStart;
        const lastCueText=document.querySelector('#karaoke-current-source').value;
        document.querySelector('[data-editor-action="reveal-playing"]').click();
        return {maxRenderedRows:Math.max(...domCounts),frameP50Ms:frames[Math.floor(frames.length*.5)],frameP95Ms:frames[Math.floor(frames.length*.95)],framesOver50Ms:frames.filter(value=>value>50).length,inputHandlerMs,searchHandlerMs,lateNavigationMs,lastCueText,domElements:document.querySelectorAll('*').length};
      });
      // Include background poll behavior after the two-second list refresh interval.
      await page.waitForTimeout(2200);
      const calls=await page.evaluate(()=>Object.fromEntries(['get_job_detail','list_jobs','list_job_summaries'].map(cmd=>[cmd,window.__test.calls.filter(call=>call.cmd===cmd).length])));
      const rssKiB=rss(server.process().pid);
      await context.tracing.stop({path:path.join(output,`${label}-${count}.trace.zip`)});
      // Observe repeated scroll cycles without trace snapshot retention. This is
      // bounded process-tree RSS, not JS heap or a proof of long-term stability.
      const scrollCycleRssKiB=[];
      for(let cycle=0;cycle<4;cycle++){
        await page.evaluate(async()=>{const host=document.querySelector('#editor-cue-list');for(let i=0;i<30;i++){host.scrollTop=(host.scrollHeight-host.clientHeight)*i/29;await new Promise(requestAnimationFrame);}});
        scrollCycleRssKiB.push(rss(server.process().pid));
      }
      await context.close();
      results.push({count,firstInteractiveMs,...measurements,rssKiB,scrollCycleRssKiB,calls,errors});
      console.log(JSON.stringify(results.at(-1)));
    }
    fs.writeFileSync(path.join(output,`${label}.json`),JSON.stringify({engine:'Playwright WebKit',label,results},null,2)+'\n');
  } finally { await browser.close();await server.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
