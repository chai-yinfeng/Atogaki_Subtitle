import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorStructureHistory, structureEditNotice } from '../src/editor-structure-history.ts';
import { createStructureEditBackend } from '../src/editor-structure-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const cue=(n,job='a')=>({id:'cue',job_id:job,segment_index:0,start_ms:n,end_ms:n+1000,source_text:'source',translated_text:'译文',source_edited:false,translation_edited:false,translation_stale:false,timing_edited:true});
const top=(token='token',revision=42)=>({token,label:'调整时间',expected_revision:revision});
const saved=(token='token')=>({segments:[cue(10)],workspace_revision:42,translation_context_stale_ids:[],undo:{latest:top(token),discarded_entries:0,oversized:false},warnings:[]});
const restored=(latest=null)=>({segments:[cue(0)],workspace_revision:43,translation_context_stale_ids:['consumer'],latest,discarded_entries:0});
function harness(){
 let ctx={jobId:'a',navigation:1,selectedId:'cue',segments:[cue(10)]},available=true,dirty=false,denied=false,flushing=async()=>true,work=async()=>restored(),state=async()=>({segments:ctx.segments,workspace_revision:42,latest:top()});
 const calls=[],messages=[],locks=[],cleared=[],applied=[];let changes=0;
 const history=createEditorStructureHistory({
  context:()=>{assert.equal(denied,false,'disposed module must not read context');return ctx;},available:()=>available,dirty:()=>dirty,flush:()=>flushing(),
  backend:createStructureEditBackend(async(command,args)=>{calls.push({command,args});return command==='get_structure_edit_state'?state():work(args);}),
  busy:value=>{locks.push(value);available=!value;},status:(...args)=>messages.push(args),
  apply:(result,id)=>{applied.push({result,id});ctx={...ctx,segments:result.segments};},clearTextHistory:job=>cleared.push(job),changed:()=>changes++,
 });
 return {history,calls,messages,locks,cleared,applied,get ctx(){return ctx;},get changes(){return changes;},change:value=>ctx={...ctx,...value},flush:value=>flushing=value,work:value=>work=value,state:value=>state=value,dirty:value=>dirty=value,deny:()=>denied=true};
}
const remember=h=>h.history.remember(saved(),'a');
const settle=()=>new Promise(resolve=>setImmediate(resolve));
test('undo sends only token; committed metadata reaches view; next descriptor uses the actual restored revision',async()=>{
 const h=harness(),result=saved();h.history.remember(result,'a');result.undo.latest.token='mutated';
 assert.deepEqual(h.cleared,['a']);assert.deepEqual(h.history.presentation(),{available:true,label:'调整时间'});
 h.work(async()=>restored(top('previous',43)));await h.history.undo();
 assert.deepEqual(h.calls,[{command:'undo_subtitle_structure',args:{request:{jobId:'a',token:'token'}}}]);
 assert.deepEqual(h.applied,[{result:restored(top('previous',43)),id:'cue'}]);
 await h.history.undo();assert.equal(h.calls[1].args.request.token,'previous');assert.deepEqual(h.locks,[true,false,true,false]);
});
test('text saves invalidate local descriptor; failed flush or dirty input keeps current token without issuing undo',async()=>{
 const h=harness();remember(h);h.dirty(true);await h.history.undo();assert.deepEqual(h.calls,[]);assert.match(h.messages.at(-1)[0],/文字修改/);
 h.dirty(false);h.flush(async()=>false);await h.history.undo();assert.equal(h.history.presentation().available,true);
 h.history.invalidate('a');assert.equal(h.history.presentation().available,false);await h.history.undo();assert.deepEqual(h.calls,[]);
});
test('refresh recovers only backend latest token for matching rows and visit, never repaints subtitles or polls',async()=>{
 const h=harness();assert.deepEqual(h.calls,[]);await h.history.refresh();assert.equal(h.history.presentation().available,true);assert.deepEqual(h.applied,[]);
 h.state(async()=>({segments:[cue(999)],workspace_revision:44,latest:top('wrong',44)}));await h.history.refresh();assert.equal(h.history.presentation().available,false);
 const d=deferred();h.state(()=>d.promise);const pending=h.history.refresh();h.change({navigation:2});d.resolve({segments:[cue(10)],workspace_revision:42,latest:top()});await pending;assert.equal(h.history.presentation().available,false);
});
test('late refresh cannot replace a newer receipt or reenable an invalidated text save',async()=>{
 for(const kind of ['record','text']){
  const h=harness(),d=deferred();h.state(()=>d.promise);const pending=h.history.refresh();
  if(kind==='record')h.history.remember(saved('new'),'a');else h.history.invalidate('a');
  d.resolve({segments:[cue(10)],workspace_revision:42,latest:top('old')});await pending;
  if(kind==='record'){await h.history.undo();assert.equal(h.calls.at(-1).args.request.token,'new');}else assert.equal(h.history.presentation().available,false);
 }
});
test('oversized edits and budget eviction replace the advertised scope, preserve saved data and explain limits',()=>{
 const h=harness();remember(h);const oversized=saved();oversized.undo={latest:null,oversized:true,discarded_entries:1};h.history.remember(oversized,'a');assert.equal(h.history.presentation().available,false);assert.match(structureEditNotice(oversized),/修改已保存.*无法撤销/);
 const pruned=saved('new');pruned.undo.discarded_entries=2;pruned.warnings=['辅助标记失败'];h.history.remember(pruned,'a');assert.equal(h.history.presentation().available,true);assert.match(structureEditNotice(pruned),/辅助标记失败.*2 条/);assert.deepEqual(h.applied,[]);
});
test('duplicate preparation and changed task, visit, selection or receipt during flush do not undo another edit',async()=>{
 for(const change of [{jobId:'b'},{navigation:2},{selectedId:'other'},'history']){
  const h=harness(),d=deferred();remember(h);h.flush(()=>d.promise);const pending=h.history.undo();await h.history.undo();
  if(change==='history')h.history.remember(saved('new'),'a');else h.change(change);
  d.resolve(true);await pending;assert.deepEqual(h.calls,[]);assert.deepEqual(h.applied,[]);assert.deepEqual(h.locks,[]);
 }
});
test('late completion never repaints another visit; failed undo retains token for explicit retry',async()=>{
 const h=harness(),d=deferred();remember(h);h.work(()=>d.promise);const pending=h.history.undo();await settle();await h.history.undo();assert.equal(h.calls.length,1);
 d.reject(Error('fixture rollback'));await pending;assert.match(h.messages.at(-1)[0],/无法撤销.*rollback/);assert.equal(h.history.presentation().available,true);
 h.work(async()=>restored());await h.history.undo();assert.equal(h.history.presentation().available,false);
 const other=harness(),late=deferred();remember(other);other.work(()=>late.promise);const old=other.history.undo();await settle();other.change({navigation:2});late.resolve(restored());await old;assert.deepEqual(other.applied,[]);assert.deepEqual(other.messages,[]);
});
test('dispose during flush, state read or undo prevents late callbacks and releases owned busy exactly once',async()=>{
 for(const phase of ['flush','refresh','undo'])for(const fail of [false,true]){
  const h=harness(),d=deferred();remember(h);if(phase==='flush')h.flush(()=>d.promise);else if(phase==='refresh')h.state(()=>d.promise);else h.work(()=>d.promise);
  const pending=phase==='refresh'?h.history.refresh():h.history.undo();await settle();h.history.dispose();h.history.dispose();const changes=h.changes;h.deny();
  if(fail)d.reject(Error('late'));else d.resolve(phase==='flush'?true:phase==='refresh'?{segments:[cue(10)],workspace_revision:42,latest:top()}:restored());await pending;
  await h.history.undo();remember(h);assert.deepEqual(h.history.presentation(),{available:false,label:''});assert.deepEqual(h.applied,[]);assert.deepEqual(h.messages,[]);assert.equal(h.changes,changes);assert.deepEqual(h.locks,phase==='undo'?[true,false]:[]);
 }
});
