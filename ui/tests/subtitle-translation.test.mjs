import test from 'node:test';
import assert from 'node:assert/strict';
import { createSubtitleTranslation, needsTranslation } from '../src/subtitle-translation.ts';
import { createTranslationBackend } from '../src/subtitle-translation-backend.ts';
const flush=()=>new Promise(resolve=>setImmediate(resolve));
function harness(){
 const cue={id:'c',job_id:'a',segment_index:0,start_ms:1000,end_ms:2000,source_text:'source',translated_text:null,translation_stale:false};
 let context={jobId:'a',navigation:1,segments:[cue],contextStaleIds:[],provider:'fixture provider',configured:true,sourceLabel:'日语',targetLabel:'中文'},dirty=false,available=true,timer=null;
 const pending=[],events=[];
 const request=(kind,args)=>new Promise((resolve,reject)=>pending.push({kind,args,resolve,reject}));
 const tool=createSubtitleTranslation({context:()=>context,available:()=>available,dirty:()=>dirty,flush:cue=>request('flush',[cue.id]),confirm:options=>request('confirm',[options]),backend:{single:(...a)=>request('single',a),batch:(...a)=>request('batch',a),load:(...a)=>request('load',a)},acceptSegment:cue=>events.push(['segment',cue]),acceptSegments:cues=>events.push(['segments',cues]),acceptDetail:detail=>events.push(['detail',detail]),status:(...a)=>events.push(['status',...a]),changed:()=>events.push(['changed']),time:ms=>`${ms}ms`,elapsed:seconds=>`${seconds}s`,clock:{now:()=>5000,repeat:tick=>{timer=tick;events.push(['timer-start']);return()=>{timer=null;events.push(['timer-stop']);};}}});
 return {tool,cue,context,pending,events,setContext:v=>context=v,setDirty:v=>dirty=v,setAvailable:v=>available=v,get timer(){return timer;}};
}
const translated={id:'c',job_id:'a',translated_text:'译文'};
const result={segments:[translated],requested_count:1,translated_count:1,empty_segment_ids:[],resumed:true};
test('adapter preserves original command and request keys',async()=>{
 const calls=[],backend=createTranslationBackend(async(...args)=>calls.push(args));
 await backend.single('a','c');await backend.batch('a',true);await backend.load('a');
 assert.deepEqual(calls,[['translate_subtitle',{jobId:'a',segmentId:'c'}],['translate_all_subtitles',{jobId:'a',onlyPending:true}],['get_job_detail',{jobId:'a'}]]);
});
test('pending includes missing, stale and context dependent cues',()=>{
 const h=harness();assert.equal(needsTranslation(h.cue,[]),true);h.cue.translated_text='ok';assert.equal(needsTranslation(h.cue,[]),false);assert.equal(needsTranslation(h.cue,['c']),true);h.cue.translation_stale=true;assert.equal(needsTranslation(h.cue,[]),true);
});
test('single flushes before network, suppresses duplicate and refreshes persisted detail',async()=>{
 const h=harness(),run=h.tool.single(h.cue);await h.tool.single(h.cue);assert.deepEqual(h.pending.map(r=>r.kind),['flush']);assert(h.tool.has('a'));
 h.pending[0].resolve();await flush();assert.deepEqual(h.pending[1].args,['a','c']);h.pending[1].resolve(translated);await flush();assert.deepEqual(h.events.find(e=>e[0]==='segment'),['segment',translated]);h.pending[2].resolve({snapshot:1});await run;
 assert.deepEqual(h.events.find(e=>e[0]==='detail'),['detail',{snapshot:1}]);assert.match(h.events.findLast(e=>e[0]==='status')[1],/fixture provider.*更新/);assert.equal(h.tool.has('a'),false);assert.equal(h.timer,null);
});
test('failed draft save never sends translation and allows retry',async()=>{
 const h=harness(),run=h.tool.single(h.cue);h.pending[0].reject(Error('disk full'));await run;assert.equal(h.pending.length,1);assert.match(h.events.findLast(e=>e[0]==='status')[1],/disk full/);assert(!h.tool.has('a'));
 const retry=h.tool.single(h.cue);h.pending[1].reject(Error('retry'));await retry;
});
test('navigation during flush stops new network work and same-task return rejects late completion',async()=>{
 const h=harness(),run=h.tool.single(h.cue);h.context.navigation=2;h.pending[0].resolve();await run;assert.equal(h.pending.length,1);
 const next=h.tool.single(h.cue);h.pending[1].resolve();await flush();h.context.navigation=3;const count=h.events.length;h.pending[2].resolve(translated);await next;
 assert.equal(h.pending.length,3);assert(!h.events.slice(count).some(e=>['segment','detail','status'].includes(e[0])));
});
test('batch confirmation is deduplicated, dirty state rechecked, cancel never sends',async()=>{
 for(const mode of ['dirty','cancel','navigate','busy']){
  const h=harness(),run=h.tool.batch(true);await h.tool.batch(true);assert.equal(h.pending.length,1);
  assert.match(h.pending[0].args[0].message,/1 段缺失/);
  if(mode==='dirty')h.setDirty(true);if(mode==='navigate')h.context.navigation=2;if(mode==='busy')h.setAvailable(false);
  h.pending[0].resolve(mode!=='cancel');await run;assert.equal(h.pending.length,1);assert(!h.tool.has('a'));assert.equal(h.timer,null);
 }
});
test('batch success reports actual saved count and empty cue instead of invented fixed batches',async()=>{
 const h=harness(),run=h.tool.batch(true);h.pending[0].resolve(true);await flush();assert.deepEqual(h.pending[1].args,['a',true]);assert.doesNotMatch(h.events.findLast(e=>e[0]==='status')[1],/\d+ 批/);
 h.pending[1].resolve({...result,translated_count:0,empty_segment_ids:['c'],segments:[h.cue]});await flush();h.pending[2].resolve({done:true});await run;
 assert.match(h.events.findLast(e=>e[0]==='status')[1],/已保存 0\/1.*第 1 段（1000ms）/);assert.equal(h.timer,null);
});
test('batch failure refreshes already persisted batches and preserves resume guidance',async()=>{
 const h=harness(),run=h.tool.batch(true);h.pending[0].resolve(true);await flush();h.pending[1].reject(Error('provider failed'));await flush();assert.equal(h.pending[2].kind,'load');h.pending[2].resolve({partial:true});await run;
 assert.deepEqual(h.events.find(e=>e[0]==='detail'),['detail',{partial:true}]);assert.match(h.events.findLast(e=>e[0]==='status')[1],/之前成功.*补译.*provider failed/);
});
test('late reload failure after navigation cannot replace another task message',async()=>{
 const h=harness(),run=h.tool.batch(false);h.pending[0].resolve(true);await flush();h.pending[1].reject(Error('provider'));await flush();h.setContext({...h.context,jobId:'b',navigation:2});const count=h.events.length;h.pending[2].reject(Error('reload'));await run;
 assert(!h.events.slice(count).some(e=>['detail','status'].includes(e[0])));
});
test('dispose clears timer and suppresses late success, failures and pending confirmation',async()=>{
 for(const stage of ['confirm','network','reload']){
  const h=harness(),run=h.tool.batch(true);
  if(stage!=='confirm'){h.pending[0].resolve(true);await flush();}
  if(stage==='reload'){h.pending[1].resolve(result);await flush();}
  h.tool.dispose();const count=h.events.length;
  if(stage==='confirm')h.pending[0].resolve(true);else if(stage==='network')h.pending[1].reject(Error('late'));else h.pending[2].resolve({late:true});
  await run;assert.equal(h.events.length,count);assert.equal(h.timer,null);assert(!h.tool.has('a'));
 }
});
test('different tasks can finish independently without clearing another task activity or timer',async()=>{
 const h=harness(),first=h.tool.single(h.cue);h.pending[0].resolve();await flush();
 const other={...h.cue,id:'d',job_id:'b'};h.setContext({...h.context,jobId:'b',navigation:2,segments:[other]});const second=h.tool.single(other);h.pending[2].resolve();await flush();
 h.pending[1].reject(Error('old task'));await first;assert(h.tool.has('b'));assert(h.timer);assert.equal(h.events.filter(e=>e[0]==='timer-start').length,1);
 h.pending[3].resolve(other);await flush();h.pending[4].resolve({job:'b'});await second;assert.equal(h.timer,null);assert(!h.events.some(e=>e[0]==='status'&&e[1].includes('old task')));
});
