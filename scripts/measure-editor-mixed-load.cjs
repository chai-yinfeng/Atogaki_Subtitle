// Production bundle, synthetic backend/media. RSS is process memory, not retained JS heap.
const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {execFileSync}=require('node:child_process');
const output=process.env.ATOGAKI_EDITOR_QA_OUTPUT||'/tmp/atogaki-editor-mixed-load';
const seconds=Number(process.env.ATOGAKI_SOAK_SECONDS||600),count=Number(process.env.ATOGAKI_CUE_COUNT||10000);
if(!Number.isFinite(seconds)||seconds<10||!Number.isInteger(count)||count<30)throw Error('Invalid measurement duration or cue count');
const rss=()=>{
 const rows=execFileSync('/bin/ps',['-axo','pid=,ppid=,rss=,comm='],{encoding:'utf8'}).trim().split('\n').map(line=>{const match=line.trim().match(/^(\d+)\s+(\d+)\s+(\d+)\s+(.+)$/);return match?{pid:+match[1],ppid:+match[2],rssKiB:+match[3],command:match[4]}:null;}).filter(Boolean);
 const owned=new Set([process.pid]);let changed=true;while(changed){changed=false;for(const row of rows)if(owned.has(row.ppid)&&!owned.has(row.pid)){owned.add(row.pid);changed=true;}}
 const processes=rows.filter(row=>row.pid!==process.pid&&owned.has(row.pid)&&!row.command.endsWith('/ps'));
 return {processes,browserHostTreeRssMiB:processes.reduce((sum,row)=>sum+row.rssKiB,0)/1024};
};
(async()=>{
 fs.mkdirSync(output,{recursive:true});const log=path.join(output,'samples.jsonl');fs.writeFileSync(log,'',{flag:'wx'});
 const browser=await webkit.launch({headless:true});const errors=[],samples=[],started=Date.now();let cycles=0;
 try{
  const page=await browser.newPage({viewport:{width:1180,height:760}});page.on('pageerror',error=>errors.push(String(error)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,count,10800000);
  await page.addInitScript(()=>{
   const test=window.__test,base=window.__TAURI_INTERNALS__.invoke;
   test.bJob={...test.job,job_id:'job-b',display_name:'混合负载任务 B'};
   test.bCues=test.cues.map((cue,i)=>({...cue,id:'b-'+i,job_id:'job-b'}));test.commandCounts={};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    test.commandCounts[cmd]=(test.commandCounts[cmd]||0)+1;
    if(cmd==='update_subtitle'&&args.request.jobId==='job-b'){
     const r=args.request,c=test.bCues.find(c=>c.id===r.segmentId);Object.assign(c,{source_text:r.sourceText,translated_text:r.translatedText,source_edited:true});return {...c};
    }
    const result=await base(cmd,args);test.calls.length=0; // The fixture's debug log must not create a measurement leak.
    if(cmd==='list_job_summaries')return [test.job,test.bJob];
    if(cmd==='get_job_detail'&&args.jobId==='job-b')return {...result,job:test.bJob,segments:structuredClone(test.bCues)};
    return result;
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  const source=page.locator('#karaoke-current-source'),search=page.locator('#editor-search');
  while(Date.now()-started<seconds*1000){
   const cycle=++cycles;
   await page.locator('.job-open').nth((cycle-1)%2).click();await page.locator('.editor-cue-row').first().waitFor();
   await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1);
   await page.locator('#karaoke-toggle-playback').click();
   await page.waitForFunction(()=>!document.querySelector('#karaoke-media-host audio').paused);
   const host=page.locator('#editor-cue-list'),box=await host.boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);
   for(let i=0;i<8;i++){await page.mouse.wheel(0,140);await page.waitForTimeout(40);}
   const index=count-cycle%Math.min(count-1,500);
   await search.fill(`第 ${index} 句：`);await page.waitForFunction(()=>document.querySelectorAll('.editor-cue-row').length===1);
   await page.locator('.editor-cue-row').click();const text=(await source.inputValue())+` [qa-${cycle}]`;
   await source.fill(text);await page.locator('#karaoke-save-text').click();
   await page.waitForFunction(expected=>[...window.__test.cues,...window.__test.bCues].some(cue=>cue.source_text===expected),text);
   await search.fill('');await page.locator('[data-editor-action="repair"]').click();await page.locator('#editor-repair-dialog[open]').waitFor();await page.locator('[data-editor-action="close-repair"]').click();
   await page.locator('#karaoke-open-workspace').click();await page.keyboard.press('Escape');
   await page.locator('[data-editor-action="follow"]').click();await page.waitForTimeout(1800);
   const active=await page.evaluate(()=>({rows:document.querySelectorAll('.editor-cue-row').length,elements:document.querySelectorAll('*').length,audio:document.querySelectorAll('audio,video').length,tools:document.querySelectorAll('#editor-repair-dialog,#workspace-view.editor-task-tools').length,commands:{...window.__test.commandCounts}}));
   assert.ok(active.rows>0&&active.rows<100,'viewport bounds');assert.equal(active.audio,1);assert.equal(active.tools,0,'tools parked');
   await page.locator('[data-editor-action="back"]').click();await page.locator('.job-open').first().waitFor();await page.waitForTimeout(200);
   const left=await page.evaluate(()=>({rows:document.querySelectorAll('.editor-cue-row').length,media:document.querySelectorAll('#karaoke-media-host audio,#karaoke-media-host video').length,elements:document.querySelectorAll('*').length}));
   assert.equal(left.rows,0,'list released on leave');assert.equal(left.media,0,'player released on leave');assert.deepEqual(errors,[]);
   const sample={cycle,elapsedSeconds:(Date.now()-started)/1000,active,left,...rss()};samples.push(sample);fs.appendFileSync(log,JSON.stringify(sample)+'\n');console.log(JSON.stringify({cycle,elapsedSeconds:sample.elapsedSeconds,rssMiB:sample.browserHostTreeRssMiB,rows:active.rows,elements:active.elements,leftElements:left.elements}));
  }
  const last=samples.at(-1);assert.equal(last.active.commands.get_job_detail,cycles,'full detail only on explicit opens');assert.equal(last.active.commands.update_subtitle,cycles,'one text save per cycle');assert.ok(last.active.commands.get_waveform_window<=cycles*10,'no per-frame boundary reload');
  const sourceFiles=['ui/src/main.ts','ui/src/editor-cue-list.ts','ui/src/media-player.ts','ui/src/task-tools.ts','ui/src/candidate-comparison-view.ts'];
  fs.writeFileSync(path.join(output,'summary.json'),JSON.stringify({engine:'Playwright WebKit',browserVersion:browser.version(),platform:process.platform,arch:process.arch,headless:true,productionBundle:true,syntheticAudioSeconds:20,declaredTimelineMs:10800000,cuesPerTask:count,requestedSeconds:seconds,elapsedSeconds:(Date.now()-started)/1000,cycles,errors,samples,sourceSha256:Object.fromEntries(sourceFiles.map(file=>[file,crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')])),limitations:['Synthetic IPC lives in renderer; two cue arrays included in RSS.','Only descendants are attributed; macOS XPC WebContent is excluded. Host-tree RSS is not renderer or total memory; no heap/native Tauri proof.','20 second silence is not three-hour media decoding.','No before/after baseline; do not infer scrolling improvement.']},null,2)+'\n',{flag:'wx'});
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
