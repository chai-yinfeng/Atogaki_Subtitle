import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorProcessing } from '../src/editor-processing.ts';
function harness(){
 let context={jobId:'a',navigation:1},visible=true,id=0;
 const requests=[],events=[],timers=new Map(),segments=[{id:'cue',source_text:'before'}];
 const backend=Object.fromEntries(['preview','full','refresh','reopen','adopt','cancel','progress','signals','review'].map(kind=>[kind,(...args)=>new Promise((resolve,reject)=>requests.push({kind,args,resolve,reject}))]));
 const processing=createEditorProcessing({backend,context:()=>context,visible:()=>visible,segments:()=>segments,busyChanged:(busy)=>events.push(['busy',busy]),mutationLock:v=>events.push(['lock',v]),previewed:p=>events.push(['preview',p.job_id]),adopted:(...args)=>events.push(['adopt',...args]),status:s=>events.push(['status',s]),comparisonStatus:s=>events.push(['comparison',s]),time:String},{schedule:cb=>{timers.set(++id,cb);return id},cancel:id=>timers.delete(id)});
 return {processing,requests,events,timers,segments,setContext:v=>context=v,hide:()=>visible=false,flush:()=>{const callbacks=[...timers.values()];timers.clear();callbacks.forEach(cb=>cb())}};
}
const preview=(job='a')=>({job_id:job,preview_id:'run',comparison_id:'comparison',candidate_segments:[]});
const range={startMs:0,endMs:1000,providerId:'whisper.cpp',authorizeCloudAudioUpload:false,qualitySignalId:'signal'};
test('old task results cannot open a preview or clear the next task busy state',async()=>{
 const h=harness(),a=h.processing.previewRange(range);h.processing.reset();h.setContext({jobId:'b',navigation:2});const b=h.processing.previewRange(range);
 h.requests[0].resolve(preview());await a;assert.equal(h.processing.busy,true);assert.equal(h.events.some(e=>e[0]==='preview'),false);
 h.requests[1].resolve(preview('b'));await b;assert.equal(h.processing.busy,false);assert.deepEqual(h.events.filter(e=>e[0]==='preview'),[['preview','b']]);
});
test('closed tools and same-job navigation invalidate candidate presentation',async()=>{
 const h=harness(),a=h.processing.previewFull();h.hide();h.requests[0].resolve(preview());await a;assert.equal(h.processing.preview,null);
 const b=h.processing.previewFull();h.setContext({jobId:'a',navigation:3});h.requests[1].resolve(preview());await b;assert.equal(h.events.some(e=>e[0]==='preview'),false);
});
test('adoption snapshots input and retains the originating quality signal',async()=>{
 const h=harness(),p=h.processing.previewRange(range);h.requests[0].resolve(preview());await p;
 const a=h.processing.adopt();h.segments[0].source_text='later';assert.equal(h.processing.adopting,true);
 h.requests[1].resolve([{id:'new'}]);await a;const adopted=h.events.find(e=>e[0]==='adopt');assert.equal(adopted[1][0].source_text,'before');assert.equal(adopted[4],'signal');assert.equal(h.processing.preview,null);assert.deepEqual(h.events.filter(e=>e[0]==='lock'),[['lock',true],['lock',false]]);
});
test('late adoption after reset never applies a task snapshot',async()=>{
 const h=harness(),p=h.processing.previewFull();h.requests[0].resolve(preview());await p;const a=h.processing.adopt();h.processing.reset();h.setContext({jobId:'b',navigation:2});h.requests[1].resolve([]);await a;assert.equal(h.events.some(e=>e[0]==='adopt'),false);
});
test('revision conflicts retain preview and refresh compares without a new ASR request',async()=>{
 const h=harness(),p=h.processing.previewRange(range);h.requests[0].resolve(preview());await p;const a=h.processing.adopt();h.requests[1].reject(Error('revision conflict'));await a;assert.equal(h.processing.preview.preview_id,'run');assert.match(h.events.filter(e=>e[0]==='comparison').at(-1)[1],/revision conflict/);
 const r=h.processing.refresh();assert.equal(h.requests[2].kind,'refresh');h.requests[2].resolve({...preview(),comparison_id:'new'});await r;assert.equal(h.processing.preview.comparison_id,'new');
});
test('progress is serial and late progress cannot overwrite a final result',async()=>{
 const h=harness(),p=h.processing.previewFull();h.flush();assert.equal(h.requests[1].kind,'progress');assert.equal(h.timers.size,0);h.requests[0].resolve(preview());await p;const count=h.events.length;h.requests[1].resolve({phase:'transcribing'});await new Promise(resolve=>setImmediate(resolve));assert.equal(h.events.length,count);assert.equal(h.timers.size,0);h.processing.dispose();
});
test('signal reads accept only the latest request and release resources on dispose',async()=>{
 const h=harness(),a=h.processing.signals('a'),b=h.processing.signals('a');h.requests[1].resolve(['new']);assert.deepEqual(await b,['new']);h.requests[0].resolve(['old']);assert.equal(await a,null);
 const p=h.processing.previewFull();h.processing.dispose();assert.equal(h.timers.size,0);h.requests[2].resolve(preview());await p;assert.equal(h.processing.preview,null);
});
