import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorStructureActions } from '../src/editor-structure-actions.ts';
import { createStructureEditBackend } from '../src/editor-structure-backend.ts';

const deferred = () => { let resolve, reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return {promise,resolve,reject}; };
const cue = (id, start, end, source, translation) => ({id,job_id:'a',segment_index:id==='left'?0:1,start_ms:start,end_ms:end,source_text:source,translated_text:translation,source_edited:false,translation_edited:false,translation_stale:false,timing_edited:false});
function harness() {
  let ctx={jobId:'a',navigation:1,selectedId:'left',segments:[cue('left',0,1000,'First','第一'),cue('right',1000,2000,'second','第二')],sourceLanguage:'en',targetLanguage:'zh'};
  let available=true, dirty=false, flushing=async()=>true, work=async()=>ctx.segments.map(c=>({...c})), denied=false;
  let readState=async()=>({segments:ctx.segments.map(c=>({...c})),workspace_revision:41,latest:null});
  const calls=[],messages=[],locks=[],history=[],applied=[];
  const actions=createEditorStructureActions({
    context:()=>{assert.equal(denied,false,'disposed module must not read context');return ctx;},
    available:()=>available,dirty:()=>dirty,flush:()=>flushing(),lock:()=>{},positionMs:()=>504,time:String,
    busy:value=>{locks.push(value);available=!value;},status:(...args)=>messages.push(args),
    remember:(...args)=>{history.push(args);return "";},apply:(...args)=>applied.push(args),
    backend:createStructureEditBackend(async(command,args)=>{if(command==='get_structure_edit_state')return readState();calls.push({command,args});const segments=await work();return {segments,workspace_revision:42,translation_context_stale_ids:[],undo:{latest:{token:"token",label:"edit",expected_revision:42},discarded_entries:0,oversized:false},warnings:[]};}),
  });
  return {actions,calls,messages,locks,history,applied,get ctx(){return ctx;},change:value=>ctx={...ctx,...value},flush:value=>flushing=value,work:value=>work=value,dirty:value=>dirty=value,deny:()=>denied=true,state:value=>readState=value};
}
const settle=()=>new Promise(resolve=>setImmediate(resolve));

test('Cut sends bilingual text and validated workspace revision; UI remembers only returned receipt',async()=>{
  const h=harness();await h.actions.cut();
  assert.deepEqual(h.calls,[{command:'split_subtitle_recorded',args:{request:{jobId:'a',expectedRevision:41,segmentId:'left',boundaryMs:500,leftSourceText:'First',rightSourceText:'First',leftTranslatedText:'第一',rightTranslatedText:'第一'}}}]);
  assert.equal(h.history[0][0].undo.latest.token,'token');
  assert.equal(h.history[0][1],'a');assert.equal(h.applied[0][1],'left');assert.deepEqual(h.locks,[true,false]);
});
test('Join retains language spacing, punctuation and missing-translation rules in IPC',async()=>{
  for(const [sourceLanguage,targetLanguage,left,right,translation,expectedSource,expectedTranslation] of [
    ['en','zh','First','second','第二','First second','第一第二'],
    ['ja','ko','最初','次','둘째','最初次','第一 둘째'],
    ['en','zh','First',', second','第二','First, second','第一第二'],
    ['en','zh','First','second',null,'First second',null],
  ]){
    const h=harness();h.change({sourceLanguage,targetLanguage,segments:[cue('left',0,1000,left,'第一'),cue('right',1000,2000,right,translation)]});await h.actions.join();
    assert.deepEqual(h.calls[0],{command:'merge_subtitles_recorded',args:{request:{jobId:'a',expectedRevision:41,leftSegmentId:'left',rightSegmentId:'right',sourceText:expectedSource,translatedText:expectedTranslation}}});
  }
});
test('pending flush reserves preparation; changed selection or navigation cannot dispatch against another visit',async()=>{
  for(const change of [{selectedId:'right'},{navigation:2},{jobId:'b'}]){
    const h=harness(),d=deferred();h.flush(()=>d.promise);const pending=h.actions.cut();await h.actions.join();h.change(change);d.resolve(true);await pending;
    assert.deepEqual(h.calls,[]);assert.deepEqual(h.locks,[]);assert.deepEqual(h.applied,[]);
  }
});
test('failed draft flush, new text and nonadjacent blocks prevent writes and allow explicit retry',async()=>{
  const h=harness();h.flush(async()=>false);await h.actions.cut();assert.equal(h.calls.length,0);
  h.flush(async()=>true);h.dirty(true);await h.actions.join();assert.equal(h.calls.length,0);assert.match(h.messages.at(-1)[0],/文字修改/);
  h.dirty(false);h.change({segments:[h.ctx.segments[0],{...h.ctx.segments[1],start_ms:1100}]});await h.actions.join();assert.equal(h.calls.length,0);assert.match(h.messages.at(-1)[0],/存在空白/);
  await h.actions.cut();assert.equal(h.calls.length,1);
});
test('late success and error do not repaint or create history in another task or same-task visit',async()=>{
  for(const change of [{jobId:'b'},{navigation:2}])for(const fail of [false,true]){
    const h=harness(),d=deferred();h.work(()=>d.promise);const pending=h.actions.join();await settle();assert.equal(h.calls.length,1);
    h.change(change);if(fail)d.reject(Error('old failure'));else d.resolve([]);await pending;
    assert.deepEqual(h.applied,[]);assert.deepEqual(h.history,[]);assert.equal(h.messages.length,1);assert.deepEqual(h.locks,[true,false]);
  }
});
test('dispose during preparation or native commit releases owned busy once and prevents late callbacks',async()=>{
  for(const phase of ['flush','commit'])for(const fail of [false,true]){
    const h=harness(),d=deferred();if(phase==='flush')h.flush(()=>d.promise);else h.work(()=>d.promise);
    const pending=h.actions.cut();await settle();h.actions.dispose();h.actions.dispose();h.deny();
    if(fail)d.reject(Error('late'));else d.resolve(phase==='flush'?true:[]);await pending;await h.actions.join();
    assert.deepEqual(h.history,[]);assert.deepEqual(h.applied,[]);assert.deepEqual(h.locks,phase==='commit'?[true,false]:[]);
  }
});
test('native failure preserves current data, releases busy and permits explicit retry without duplicate submission',async()=>{
  const h=harness(),d=deferred();h.work(()=>d.promise);const pending=h.actions.cut();await settle();await h.actions.cut();assert.equal(h.calls.length,1);
  d.reject(Error('fixture transaction failed'));await pending;assert.match(h.messages.at(-1)[0],/切开失败.*fixture transaction/);assert.deepEqual(h.history,[]);assert.deepEqual(h.applied,[]);
  h.work(async()=>[]);await h.actions.cut();assert.equal(h.calls.length,2);assert.equal(h.history.length,1);assert.deepEqual(h.locks,[true,false,true,false]);
});

test('on-demand revision read cannot dispatch after changed rows, selection, visit or dispose',async()=>{
 for(const change of ['rows','selection','visit','dispose']){
  const h=harness(),d=deferred(),snapshot=h.ctx.segments.map(c=>({...c}));h.state(()=>d.promise);
  const run=h.actions.cut();await settle();assert.deepEqual(h.locks,[true]);
  if(change==='rows')h.change({segments:[{...snapshot[0],source_text:'new saved text'},snapshot[1]]});
  if(change==='selection')h.change({selectedId:'right'});
  if(change==='visit')h.change({navigation:2});
  if(change==='dispose'){h.actions.dispose();h.deny();}
  d.resolve({segments:snapshot,workspace_revision:41,latest:null});await run;
  assert.deepEqual(h.calls,[]);assert.deepEqual(h.applied,[]);assert.deepEqual(h.history,[]);assert.deepEqual(h.locks,[true,false]);
 }
 const h=harness();h.state(async()=>({segments:[],workspace_revision:41,latest:null}));await h.actions.join();assert.deepEqual(h.calls,[]);assert.match(h.messages.at(-1)[0],/字幕已变化/);
});
