import test from 'node:test';import assert from 'node:assert/strict';
import {createVideoRenderSession} from '../src/video-render-session.ts';import {createVideoRenderBackend} from '../src/video-render-backend.ts';
const flush=()=>new Promise(r=>setImmediate(r));
const caps={ready_for_hard_subtitles:true},render={id:'r',source_job_id:'a',status:'running',output_path:'/out/a.mp4'};
function harness(){
 let context={jobId:'a',navigation:1,inputPath:'C:\\media\\a.mp4',videoAvailable:true,staleCount:0},dirty=false,draft={outputPath:'/out/a.mp4',track:'bilingual',quality:'balanced'},tick=null;
 const pending=[],events=[];const request=(kind,args)=>new Promise((resolve,reject)=>pending.push({kind,args,resolve,reject}));
 const tool=createVideoRenderSession({context:()=>context,available:()=>true,dirty:()=>dirty,staleCount:()=>context.staleCount,suggestedName:()=> 'a.bilingual.mp4',backend:Object.fromEntries(['capabilities','estimates','list','pick','submit','cancel','reveal'].map(kind=>[kind,(...args)=>request(kind,args)])),confirm:opts=>request('confirm',[opts]),status:(...a)=>events.push(['status',...a]),fileManager:'Finder',repeat:fn=>{tick=fn;return()=>{tick=null;};},view:{open:()=>events.push(['open']),close:()=>events.push(['close']),read:()=>({...draft}),output:path=>{draft.outputPath=path;events.push(['output',path]);},message:t=>events.push(['message',t]),capabilities:e=>events.push(['capabilities',e]),estimates:e=>events.push(['estimates',e]),history:()=>events.push(['history']),busy:(...a)=>events.push(['busy',...a])}});
 const open=async(renders=[])=>{const run=tool.open();pending.at(-3).resolve(caps);pending.at(-2).resolve([]);pending.at(-1).resolve(renders);await run;draft.outputPath='/out/a.mp4';};
 return {tool,pending,events,context,draft,open,setContext:v=>context=v,setDirty:v=>dirty=v,get tick(){return tick;}};
}
test('all seven IPC contracts preserve original keys',async()=>{
 const calls=[],b=createVideoRenderBackend(async(...a)=>calls.push(a));await b.capabilities();await b.estimates('a');await b.list('a');await b.pick('/out','a.mp4');await b.submit({sourceJobId:'a',outputPath:'/out/a.mp4',subtitleTrack:'source',quality:'compact',overwriteExisting:false});await b.cancel('r');await b.reveal('/out/a.mp4');
 assert.deepEqual(calls,[['media_capabilities'],['video_render_estimates',{sourceJobId:'a'}],['list_video_renders',{sourceJobId:'a'}],['pick_video_output_file',{initialDirectory:'/out',suggestedName:'a.mp4'}],['submit_video_render',{request:{sourceJobId:'a',outputPath:'/out/a.mp4',subtitleTrack:'source',quality:'compact',overwriteExisting:false}}],['cancel_video_render',{renderId:'r'}],['reveal_rendered_video',{path:'/out/a.mp4'}]]);
});
test('opening protects dirty, stale and unavailable video before requesting',async()=>{
 for(const reason of ['dirty','stale','audio']){const h=harness();if(reason==='dirty')h.setDirty(true);if(reason==='stale')h.context.staleCount=1;if(reason==='audio')h.context.videoAvailable=false;await h.tool.open();assert.equal(h.pending.length,0);assert(h.events.some(e=>e[0]==='status'));}
});
test('open clears prior output, requires capabilities and stops polling on close',async()=>{
 const h=harness(),run=h.tool.open();assert.equal(h.tool.canSubmit,false);assert.deepEqual(h.events[0],['output','']);h.pending[0].reject(Error('ffmpeg missing'));h.pending[1].resolve([]);h.pending[2].resolve([render]);await run;assert.equal(h.tool.canSubmit,false);h.tool.close();assert.equal(h.tick,null);await h.tool.submit();assert.equal(h.pending.length,3);
});
test('history polling coalesces and never runs for completed/closed tools',async()=>{
 const h=harness();await h.open([render]);h.tick();h.tick();const one=h.tool.refresh();assert.equal(h.pending.filter(r=>r.kind==='list').length,2);h.pending.at(-1).resolve([{...render,status:'done'}]);await one;h.tick();assert.equal(h.pending.filter(r=>r.kind==='list').length,2);h.tool.close();await h.tool.refresh();assert.equal(h.pending.filter(r=>r.kind==='list').length,2);
});
test('old open and history cannot affect a new visit or clear its request lease',async()=>{
 const h=harness(),old=h.tool.open();h.tool.close();h.context.navigation=2;const next=h.tool.open();const count=h.events.length;h.pending[0].resolve(caps);h.pending[1].reject(Error('old estimates'));h.pending[2].resolve([render]);await old;assert.equal(h.events.length,count);assert.equal(h.tool.canSubmit,false);h.pending[3].resolve(caps);h.pending[4].resolve([]);h.pending[5].resolve([]);await next;assert.equal(h.tool.canSubmit,true);h.tool.close();
});
test('late picker cannot populate a different task output',async()=>{
 const h=harness();await h.open();const pick=h.tool.chooseOutput();assert.deepEqual(h.pending[3].args,['C:\\media','a.bilingual.mp4']);assert.equal(h.tool.canSubmit,false);h.tool.close();h.setContext({...h.context,jobId:'b',navigation:2});const count=h.events.length;h.pending[3].resolve({path:'/wrong.mp4',alreadyExists:true});await pick;assert.equal(h.events.length,count);
});
async function selected(h){const run=h.tool.chooseOutput();h.pending.at(-1).resolve({path:'/out/a.mp4',alreadyExists:true});await run;}
test('overwrite cancel, changed options, dirty input and navigation never submit',async()=>{
 for(const mode of ['cancel','path','track','quality','dirty','navigation']){
  const h=harness();await h.open();await selected(h);const run=h.tool.submit();await h.tool.submit();const confirm=h.pending.at(-1);assert.equal(confirm.kind,'confirm');if(mode==='path')h.draft.outputPath='/out/other.mp4';if(mode==='track')h.draft.track='source';if(mode==='quality')h.draft.quality='high';if(mode==='dirty')h.setDirty(true);if(mode==='navigation')h.context.navigation++;
  confirm.resolve(mode!=='cancel');await run;assert(!h.pending.some(r=>r.kind==='submit'));h.tool.close();
 }
});
test('confirmed submit freezes original request and refreshes history; duplicate starts suppressed',async()=>{
 const h=harness();await h.open();await selected(h);const run=h.tool.submit();h.pending.at(-1).resolve(true);await flush();const submit=h.pending.at(-1);assert.equal(submit.kind,'submit');assert.deepEqual(submit.args,[{sourceJobId:'a',outputPath:'/out/a.mp4',subtitleTrack:'bilingual',quality:'balanced',overwriteExisting:true}]);await h.tool.submit();assert.equal(h.pending.at(-1),submit);submit.resolve(render);await flush();h.pending.at(-1).resolve([render]);await run;assert(h.tool.canSubmit);h.tool.close();
});
test('submit errors unlock; disposal suppresses late submit finally and cancels timer',async()=>{
 const h=harness();await h.open();const first=h.tool.submit();h.pending.at(-1).reject(Error('revision changed'));await first;assert(h.tool.canSubmit);assert(h.events.some(e=>e[0]==='message'&&e[1].includes('revision changed')));const retry=h.tool.submit();h.tool.dispose();const count=h.events.length;h.pending.at(-1).resolve(render);await retry;assert.equal(h.events.length,count);assert.equal(h.tick,null);
});
test('cancel is limited to current active history and rechecks context after confirmation',async()=>{
 for(const mode of ['navigation','completed']){const h=harness();await h.open([{...render}]);await h.tool.cancel('not-ours');const run=h.tool.cancel('r');await h.tool.cancel('r');const confirm=h.pending.at(-1);assert.equal(confirm.kind,'confirm');if(mode==='navigation')h.context.navigation++;else {const reload=h.tool.refresh();h.pending.at(-1).resolve([{...render,status:'done'}]);await reload;}confirm.resolve(true);await run;assert(!h.pending.some(r=>r.kind==='cancel'));h.tool.close();}
});
test('only completed history paths may be revealed and late errors stay isolated',async()=>{
 const h=harness();await h.open([{...render,status:'done'}]);await h.tool.reveal('/arbitrary');const run=h.tool.reveal('/out/a.mp4');assert.equal(h.pending.at(-1).kind,'reveal');h.tool.close();const count=h.events.length;h.pending.at(-1).reject(Error('late Finder'));await run;assert.equal(h.events.length,count);
});
