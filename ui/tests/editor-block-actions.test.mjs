import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorBlockActions } from '../src/editor-block-actions.ts';
import { createStructureEditBackend } from '../src/editor-structure-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const cue=(id='left',start=0)=>({id,job_id:'a',segment_index:0,start_ms:start,end_ms:start+1000,source_text:'source',translated_text:'译文',source_edited:false,translation_edited:false,translation_stale:false,timing_edited:false});
const identity={jobId:'a',navigation:1};
function harness(){
 let ctx={...identity,selectedId:'left',segments:[cue()]},available=true,dirty=false,dead=false,flush=async()=>true,confirm=async()=>true,work=async args=>args.request.insertion?[...args.request.beforeSegments,args.request.insertion]:[];
 const calls=[],messages=[],locks=[],history=[],applied=[],seeks=[],confirmations=[];let pauses=0;
 const actions=createEditorBlockActions({
  context:()=>{assert.equal(dead,false,'disposed module must not read context');return ctx;},available:()=>available,dirty:()=>dirty,flush:()=>flush(),
  confirm:async value=>{confirmations.push(value);return confirm();},pause:()=>pauses++,seek:value=>seeks.push(value),
  backend:createStructureEditBackend(async(command,args)=>{calls.push({command,args});const segments=await work(args);return {segments,workspace_revision:42,translation_context_stale_ids:[],undo:{latest:{token:"token",label:args.request.removeId?"删除字幕块":"插入字幕块",expected_revision:42},discarded_entries:0,oversized:false},warnings:[]};}),
  busy:value=>{locks.push(value);available=!value;},status:(...args)=>messages.push(args),remember:(...args)=>{history.push(args);return "";},apply:(...args)=>applied.push(args),
 });
 return {actions,calls,messages,locks,history,applied,seeks,confirmations,get pauses(){return pauses;},get ctx(){return ctx;},change:value=>ctx={...ctx,...value},flush:value=>flush=value,confirm:value=>confirm=value,work:value=>work=value,dirty:value=>dirty=value,deny:()=>dead=true};
}
const settle=()=>new Promise(resolve=>setImmediate(resolve));
test('insertion uses original block IPC and history, snapshots submitted cue before draft flush, then seeks selected new block',async()=>{
 const h=harness(),pending=deferred(),insertion=cue('new',1200);h.flush(()=>pending.promise);const run=h.actions.insert(identity,insertion);insertion.source_text='later';pending.resolve(true);assert.equal(await run,null);
 assert.deepEqual(h.calls,[{command:'edit_subtitle_block_recorded',args:{request:{jobId:'a',beforeSegments:[cue()],removeId:null,insertion:cue('new',1200)}}}]);assert.equal(h.applied[0][1],'new');assert.deepEqual(h.seeks,[1200]);assert.equal(h.history[0][0].undo.latest.label,'插入字幕块');assert.deepEqual(h.locks,[true,false]);
});
test('confirmed deletion uses exact original cue and current before snapshot, no seek, same history',async()=>{
 const h=harness();await h.actions.deleteSelected();assert.equal(h.pauses,1);assert.equal(h.confirmations[0].danger,true);assert.deepEqual(h.calls,[{command:'edit_subtitle_block_recorded',args:{request:{jobId:'a',beforeSegments:[cue()],removeId:'left',insertion:null}}}]);assert.equal(h.history[0][0].undo.latest.label,'删除字幕块');assert.deepEqual(h.seeks,[]);assert.deepEqual(h.locks,[true,false]);
});
test('changed visit, task, selection, saved content or new draft while confirmation waits cancels the old deletion',async()=>{
 for(const change of [{jobId:'b'},{navigation:2},{selectedId:'other'},{segments:[{...cue(),source_text:'new saved text'}]},'dirty']){
  const h=harness(),d=deferred();h.confirm(()=>d.promise);const run=h.actions.deleteSelected();await settle();assert.equal(h.confirmations.length,1);
  if(change==='dirty')h.dirty(true);else h.change(change);d.resolve(true);await run;assert.deepEqual(h.calls,[]);assert.deepEqual(h.history,[]);assert.deepEqual(h.applied,[]);
  if(change==='dirty'||'segments' in change||'selectedId' in change)assert.match(h.messages.at(-1)[0],/已变化，未删除/);
 }
});
test('changes during second draft flush cannot delete a modified cue or enter another visit',async()=>{
 for(const change of [{navigation:2},{selectedId:'other'},{segments:[{...cue(),end_ms:900}]}]){
  const h=harness(),d=deferred();let n=0;h.flush(()=>++n===1?Promise.resolve(true):d.promise);const run=h.actions.deleteSelected();await settle();assert.equal(n,2);h.change(change);d.resolve(true);await run;assert.deepEqual(h.calls,[]);
 }
});
test('reservation prevents repeated confirmation or cross insert/delete preparation; cancel and blocked drafts leave data intact',async()=>{
 const h=harness(),d=deferred();h.confirm(()=>d.promise);const run=h.actions.deleteSelected();await settle();await h.actions.deleteSelected();assert.notEqual(await h.actions.insert(identity,cue('new',1200)),null);assert.equal(h.confirmations.length,1);d.resolve(false);await run;assert.deepEqual(h.calls,[]);assert.deepEqual(h.locks,[]);
 h.flush(async()=>false);await h.actions.deleteSelected();assert.equal(h.confirmations.length,1);assert.notEqual(await h.actions.insert(identity,cue('new',1200)),null);
 h.flush(async()=>true);h.confirm(async()=>true);await h.actions.deleteSelected();assert.equal(h.calls.length,1);
});
test('insertion refuses wrong job and changed identity during flush; failure retains input value and permits explicit retry',async()=>{
 const h=harness();assert.notEqual(await h.actions.insert(identity,{...cue('new',1200),job_id:'b'}),null);assert.deepEqual(h.calls,[]);
 const d=deferred();h.flush(()=>d.promise);const run=h.actions.insert(identity,cue('new',1200));h.change({navigation:2});d.resolve(true);assert.notEqual(await run,null);assert.deepEqual(h.calls,[]);
 h.change(identity);h.flush(async()=>true);h.work(async()=>{throw Error('fixture disk full');});const insertion=cue('new',1200);assert.match(await h.actions.insert(identity,insertion),/disk full/);assert.deepEqual(insertion,cue('new',1200));assert.deepEqual(h.history,[]);
 h.work(async()=>[insertion]);assert.equal(await h.actions.insert(identity,insertion),null);assert.equal(h.calls.length,2);assert.deepEqual(h.locks,[true,false,true,false]);
});
test('late native success or failure cannot apply, seek, record history or announce to another task or visit',async()=>{
 for(const kind of ['insert','delete'])for(const change of [{jobId:'b'},{navigation:2}])for(const fail of [false,true]){
  const h=harness(),d=deferred();h.work(()=>d.promise);const run=kind==='insert'?h.actions.insert(identity,cue('new',1200)):h.actions.deleteSelected();await settle();assert.equal(h.calls.length,1);h.change(change);
  if(fail)d.reject(Error('old request'));else d.resolve([]);await run;assert.deepEqual(h.applied,[]);assert.deepEqual(h.history,[]);assert.deepEqual(h.seeks,[]);assert.deepEqual(h.messages,[]);assert.deepEqual(h.locks,[true,false]);
 }
});
test('dispose during flush, confirmation or native commit ignores late callbacks and releases only owned busy',async()=>{
 for(const phase of ['flush','confirm','commit'])for(const fail of [false,true]){
  const h=harness(),d=deferred();if(phase==='flush')h.flush(()=>d.promise);else if(phase==='confirm')h.confirm(()=>d.promise);else h.work(()=>d.promise);
  const run=h.actions.deleteSelected();await settle();h.actions.dispose();h.actions.dispose();h.deny();if(fail)d.reject(Error('late'));else d.resolve(phase==='commit'?[]:true);await run;await h.actions.deleteSelected();assert.notEqual(await h.actions.insert(identity,cue('new',1200)),null);assert.deepEqual(h.history,[]);assert.deepEqual(h.applied,[]);assert.deepEqual(h.messages,[]);assert.deepEqual(h.locks,phase==='commit'?[true,false]:[]);
 }
});
