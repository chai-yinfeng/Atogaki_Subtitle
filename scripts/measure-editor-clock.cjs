// Deterministic timeupdate dispatch at one paused media time. Measures the UI
// clock hot path, not decoder throughput, native CPU, heap or scrolling FPS.
const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const fs=require('node:fs'),path=require('node:path');
const output=process.env.ATOGAKI_EDITOR_QA_OUTPUT||'/tmp/atogaki-editor-clock';
const label=process.env.ATOGAKI_MEASUREMENT_LABEL||'clock';const target=path.join(output,`${label}.json`);
if(fs.existsSync(target))throw Error(`Refusing to overwrite ${target}`);
const percentile=(values,fraction)=>[...values].sort((a,b)=>a-b)[Math.floor(values.length*fraction)];
(async()=>{const browser=await webkit.launch({headless:true});const results=[];try{
 for(const count of [300,3000,10000]){
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,count,10800000);await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1||document.querySelector('#karaoke-media-host video')?.readyState>=1);
  const rounds=[];
  for(let round=0;round<3;round++)rounds.push(await page.evaluate(async()=>{
   const media=document.querySelector('#karaoke-media-host audio, #karaoke-media-host video'),list=document.querySelector('#editor-cue-list'),title=document.querySelector('#karaoke-segment-time');media.pause();
   const settle=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));await settle();
   let rowAttributes=0,rowChildren=0,titleChildren=0;const observer=new MutationObserver(records=>{for(const r of records){if(list.contains(r.target)){if(r.type==='attributes')rowAttributes++;else rowChildren++;}else if(title.contains(r.target))titleChildren++;}});
   observer.observe(list,{subtree:true,attributes:true,childList:true,characterData:true});observer.observe(title,{subtree:true,childList:true,characterData:true});
   const handler=[],intervals=[];let previous=performance.now();
   for(let i=0;i<120;i++){await new Promise(requestAnimationFrame);const now=performance.now();intervals.push(now-previous);previous=now;const start=performance.now();media.dispatchEvent(new Event('timeupdate'));handler.push(performance.now()-start);}
   await settle();observer.disconnect();return {rowAttributes,rowChildren,titleChildren,handler,intervals,rows:list.querySelectorAll('.editor-cue-row').length,playbackCue:list.querySelector('[aria-current="true"]')?.dataset.cueId};
  }));
  const result={count,errors,rounds:rounds.map(({handler,intervals,...r})=>({...r,handlerTotalMs:handler.reduce((a,b)=>a+b,0),handlerP95Ms:percentile(handler,.95),frameP95Ms:percentile(intervals,.95),framesOver50Ms:intervals.filter(t=>t>50).length}))};results.push(result);console.log(JSON.stringify(result));await page.close();
 }
 fs.mkdirSync(output,{recursive:true});fs.writeFileSync(target,JSON.stringify({label,engine:'Playwright WebKit',viewport:[1180,760],eventsPerRound:120,roundsPerScale:3,method:'paused fixed-time synthetic timeupdate',results},null,2)+'\n',{flag:'wx'});
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exitCode=1;});
