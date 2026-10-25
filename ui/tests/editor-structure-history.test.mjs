import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorStructureHistory } from '../src/editor-structure-history.ts';
import { createEditorBackend } from '../src/editor-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const cue=(n,job='a')=>({id:'cue',job_id:job,segment_index:0,start_ms:n,end_ms:n+1000,source_text:'source',translated_text:'译文',source_edited:false,translation_edited:false,translation_stale:false,timing_edited:true});
function harness(){
 let ctx={jobId:'a',navigation:1,selectedId:'cue',segments:[cue(10)]},available=true,dirty=false,denied=false,flushing=async()=>true,work=async args=>args.request.beforeSegments;
 const calls=[],messages=[],locks=[],cleared=[],applied=[];let changes=0;
 const history=createEditorStructureHistory({
  context:()=>{assert.equal(denied,false,'disposed module must not read context');return ctx;},available:()=>available,dirty:()=>dirty,flush:()=>flushing(),
  backend:createEditorBackend(async(command,args)=>{calls.push({command,args});return work(args);}),
  busy:value=>{locks.push(value);available=!value;},status:(...args)=>messages.push(args),
  apply:(segments,id)=>{applied.push({segments,id});ctx={...ctx,segments};},clearTextHistory:job=>cleared.push(job),changed:()=>changes++,
 });
 return {history,calls,messages,locks,cleared,applied,get ctx(){return ctx;},get changes(){return changes;},change:value=>ctx={...ctx,...value},flush:value=>flushing=value,work:value=>work=value,dirty:value=>dirty=value,deny:()=>denied=true};
}
const remember=(h)=>h.history.remember([cue(0)],[cue(10)],'手动调整时间','a');
const settle=()=>new Promise(resolve=>setImmediate(resolve));

test('history snapshots are immutable, restore uses original IPC contract and selection, text history remains separate',async()=>{
 const h=harness(),before=[cue(0)],after=[cue(10)];h.history.remember(before,after,'手动调整时间','a');before[0].source_text='mutated';after[0].start_ms=900;
 assert.deepEqual(h.cleared,['a']);assert.deepEqual(h.history.presentation(),{available:true,label:'手动调整时间'});
 await h.history.undo();assert.deepEqual(h.calls,[{command:'restore_subtitle_structure',args:{request:{jobId:'a',beforeSegments:[cue(0)],afterSegments:[cue(10)]}}}]);
 assert.deepEqual(h.applied,[{segments:[cue(0)],id:'cue'}]);assert.equal(h.history.presentation().available,false);assert.deepEqual(h.locks,[true,false]);assert.match(h.messages.at(-1)[0],/仅|只在当前 App 会话/);
});
test('per-task history is isolated and bounded to ten entries without merging text or timing commands',async()=>{
 const h=harness();for(let i=0;i<12;i++)h.history.remember([cue(i)],[cue(i+1)],`edit ${i}`,'a');
 h.change({segments:[cue(12)]});h.history.remember([cue(20,'b')],[cue(30,'b')],'task B','b');
 for(let i=11;i>=2;i--){assert.equal(h.history.presentation().label,`edit ${i}`);await h.history.undo();}
 assert.equal(h.ctx.segments[0].start_ms,2);assert.equal(h.history.presentation().available,false);
 h.change({jobId:'b',segments:[cue(30,'b')]});assert.equal(h.history.presentation().label,'task B');await h.history.undo();assert.equal(h.ctx.segments[0].start_ms,20);
});
test('stale snapshot disables undo and refuses native restore; pending text or failed flush preserves entry',async()=>{
 const h=harness();remember(h);h.change({segments:[cue(11)]});assert.equal(h.history.presentation().available,false);await h.history.undo();assert.deepEqual(h.calls,[]);assert.match(h.messages.at(-1)[0],/已变化/);
 h.change({segments:[cue(10)]});h.dirty(true);await h.history.undo();assert.deepEqual(h.calls,[]);assert.match(h.messages.at(-1)[0],/文字修改/);
 h.dirty(false);h.flush(async()=>false);await h.history.undo();assert.equal(h.history.presentation().available,true);
 h.flush(async()=>true);await h.history.undo();assert.equal(h.calls.length,1);
});
test('duplicate preparation and changed task, visit, selection or history during flush do not undo a different edit',async()=>{
 for(const change of [{jobId:'b'},{navigation:2},{selectedId:'other'},'history']){
  const h=harness(),d=deferred();remember(h);h.flush(()=>d.promise);const pending=h.history.undo();await h.history.undo();
  if(change==='history')h.history.remember([cue(5)],[cue(10)],'newer','a');else h.change(change);
  d.resolve(true);await pending;assert.deepEqual(h.calls,[]);assert.deepEqual(h.applied,[]);assert.deepEqual(h.locks,[]);
 }
});
test('late success or failure cannot change history or presentation in another visit, and old entry remains guarded by snapshot',async()=>{
 for(const change of [{jobId:'b'},{navigation:2}])for(const fail of [false,true]){
  const h=harness(),d=deferred();remember(h);h.work(()=>d.promise);const pending=h.history.undo();await settle();h.change(change);
  if(fail)d.reject(Error('old'));else d.resolve([cue(0)]);await pending;assert.deepEqual(h.applied,[]);assert.deepEqual(h.messages,[]);assert.deepEqual(h.locks,[true,false]);
  h.change({jobId:'a',navigation:3,segments:[cue(0)]});assert.equal(h.history.presentation().available,false);
 }
});
test('transaction failure retains history and permits explicit retry; duplicate native calls are suppressed',async()=>{
 const h=harness(),d=deferred();remember(h);h.work(()=>d.promise);const pending=h.history.undo();await settle();await h.history.undo();assert.equal(h.calls.length,1);
 d.reject(Error('fixture rollback'));await pending;assert.match(h.messages.at(-1)[0],/无法撤销.*fixture rollback/);assert.equal(h.history.presentation().available,true);
 h.work(async args=>args.request.beforeSegments);await h.history.undo();assert.equal(h.calls.length,2);assert.equal(h.history.presentation().available,false);
});
test('dispose during preparation or native commit clears session snapshots and releases only owned busy',async()=>{
 for(const phase of ['flush','commit'])for(const fail of [false,true]){
  const h=harness(),d=deferred();remember(h);if(phase==='flush')h.flush(()=>d.promise);else h.work(()=>d.promise);
  const pending=h.history.undo();await settle();h.history.dispose();h.history.dispose();const changes=h.changes;h.deny();
  if(fail)d.reject(Error('late'));else d.resolve(phase==='flush'?true:[cue(0)]);await pending;await h.history.undo();remember(h);
  assert.deepEqual(h.history.presentation(),{available:false,label:''});assert.deepEqual(h.applied,[]);assert.deepEqual(h.messages,[]);assert.equal(h.changes,changes);assert.deepEqual(h.locks,phase==='commit'?[true,false]:[]);
 }
});
