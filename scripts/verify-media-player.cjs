const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const b=await webkit.launch({headless:true}),p=await b.newPage({viewport:{width:1180,height:760}}),errors=[];
 p.on('pageerror',e=>errors.push(String(e)));
 await require('./editor-ui-fixture.cjs').installEditorFixture(p);
 await p.addInitScript(()=>{
  const ipc=window.__TAURI_INTERNALS__,invoke=ipc.invoke,convert=ipc.convertFileSrc;
  ipc.convertFileSrc=path=>convert(path==='/fallback.wav'?'/fixture.wav':path);
  window.__test.mode='fallback';window.__test.resumeMs=4500;
  ipc.invoke=async(cmd,args)=>{
   const result=await invoke(cmd,args);
   if(cmd==='list_job_summaries')return result.map(j=>({...j,translation_status:'translated',translated_segment_count:j.segment_count}));
   if(cmd==='get_job_detail'){
    const broken=window.__test.mode==='broken';
    return {...result,job:{...result.job,input_path:broken?'/bad.mp4':'/fixture.wav'},playback_path:broken?'/bad.mp4':'/fixture.wav',audio_fallback_path:broken?'/bad.wav':'/fallback.wav'};
   }
   if(cmd==='get_playback_position')return window.__test.resumeMs;
   if(cmd==='save_playback_position'){window.__test.resumeMs=args.request.positionMs;return null;}
   return result;
  };
 });
 await p.route('**/bad.mp4',route=>route.fulfill({status:200,contentType:'video/mp4',body:'invalid video'}));
 await p.route('**/bad.wav',route=>route.fulfill({status:200,contentType:'audio/wav',body:'invalid audio'}));
 await p.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1420');await p.locator('.job-open').first().click();
 const audio=p.locator('#karaoke-media-host audio');await audio.waitFor();
 await audio.evaluate(async a=>{
  if(a.readyState<1)await new Promise(r=>a.addEventListener('loadedmetadata',r,{once:true}));
  await new Promise(r=>{a.addEventListener('seeked',r,{once:true});a.currentTime=7;});a.muted=true;a.volume=.3;a.playbackRate=1.5;window.__originalMedia=a;
 });
 await p.locator('#karaoke-toggle-playback').click();
 await audio.evaluate(a=>a.dispatchEvent(new Event('error')));
 await p.waitForFunction(()=>{const a=document.querySelector('#karaoke-media-host audio');return a!==window.__originalMedia&&a.readyState>=1&&!a.paused;});
 const fallback=await audio.evaluate(a=>({time:a.currentTime,volume:a.volume,muted:a.muted,rate:a.playbackRate}));
 assert.ok(fallback.time>=7&&fallback.time<8);assert.equal(fallback.volume,.3);assert.equal(fallback.muted,true);assert.equal(fallback.rate,1.5);
 assert.match(await p.locator('#karaoke-media-message').innerText(),/自动使用任务音频/);
 await p.locator('#karaoke-toggle-playback').click();
 const clock=await p.locator('#karaoke-current-time').innerText();
 await p.evaluate(()=>{window.__originalMedia.currentTime=19;for(const event of ['timeupdate','loadedmetadata','error','play'])window.__originalMedia.dispatchEvent(new Event(event));});
 assert.equal(await p.locator('#karaoke-current-time').innerText(),clock);assert.equal(await audio.count(),1);
 await audio.evaluate(async a=>{await new Promise(r=>{a.addEventListener('seeked',r,{once:true});a.currentTime=18;});});
 await p.locator('[data-editor-action="back"]').click();assert.equal(await p.locator('#karaoke-media-host audio').count(),0);
 await p.locator('.job-open').first().click();
 await p.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.currentTime>=17.9);
 assert.ok(await p.locator('#editor-cue-list').evaluate(e=>e.scrollTop>0));
 assert.equal(await p.locator('[data-cue-id="cue-6"]').getAttribute('aria-current'),'true');
 await p.locator('[data-editor-action="back"]').click();
 // Shared listening player restores and persists position without an editor instance.
 await p.locator('#show-listening').click();await p.locator('[data-listen-job]').first().click();
 const listening=p.locator('#listening-media-host audio');await listening.waitFor();
 await p.waitForFunction(()=>{const a=document.querySelector('#listening-media-host audio');return a.readyState>=1&&a.currentTime>=4.4;});
 assert.ok(await listening.evaluate(a=>a.currentTime<4.7));
 await listening.evaluate(async a=>{await new Promise(r=>{a.addEventListener('seeked',r,{once:true});a.currentTime=9;});});
 await p.locator('#show-workbench').click();assert.equal(await listening.count(),0);
 assert.equal(await p.evaluate(()=>window.__test.resumeMs),9000);
 // Both invalid sources stop after one fallback instead of recreating players forever.
 await p.evaluate(()=>window.__test.mode='broken');await p.locator('.job-open').first().click();
 await p.waitForFunction(()=>document.querySelector('#karaoke-media-message').textContent.includes('媒体加载失败'));
 assert.equal(await p.locator('#karaoke-media-host video').count(),0);assert.equal(await audio.count(),1);
 assert.equal(await audio.evaluate(a=>a.paused),true);
 assert.deepEqual(errors,[]);console.log('PASS: real WebKit fallback, playback state, old events, listening resume/persist and bounded double failure');
 await b.close();
})().catch(e=>{console.error(e);process.exit(1)});
