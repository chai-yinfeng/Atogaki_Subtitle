import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorStructureActions } from '../src/editor-structure-actions.ts';
import { createEditorBackend } from '../src/editor-backend.ts';

const deferred = () => { let resolve, reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return {promise,resolve,reject}; };
const cue = (id, start, end, source, translation) => ({id,job_id:'a',segment_index:id==='left'?0:1,start_ms:start,end_ms:end,source_text:source,translated_text:translation,source_edited:false,translation_edited:false,translation_stale:false,timing_edited:false});
function harness() {
  let ctx={jobId:'a',navigation:1,selectedId:'left',segments:[cue('left',0,1000,'First','第一'),cue('right',1000,2000,'second','第二')],sourceLanguage:'en',targetLanguage:'zh'};
  let available=true, dirty=false, flushing=async()=>true, work=async()=>ctx.segments.map(c=>({...c})), denied=false;
  const calls=[],messages=[],locks=[],history=[],applied=[];
  const actions=createEditorStructureActions({
    context:()=>{assert.equal(denied,false,'disposed module must not read context');return ctx;},
    available:()=>available,dirty:()=>dirty,flush:()=>flushing(),lock:()=>{},positionMs:()=>504,time:String,
    busy:value=>{locks.push(value);available=!value;},status:(...args)=>messages.push(args),
    remember:(...args)=>history.push(args),apply:(...args)=>applied.push(args),
    backend:createEditorBackend(async(command,args)=>{calls.push({command,args});return work();}),
  });
  return {actions,calls,messages,locks,history,applied,get ctx(){return ctx;},change:value=>ctx={...ctx,...value},flush:value=>flushing=value,work:value=>work=value,dirty:value=>dirty=value,deny:()=>denied=true};
}
const settle=()=>new Promise(resolve=>setImmediate(resolve));

test('Cut sends original bilingual text with snapped boundary; history owns an immutable before snapshot',async()=>{
  const h=harness();await h.actions.cut();
  assert.deepEqual(h.calls,[{command:'split_subtitle',args:{request:{jobId:'a',segmentId:'left',boundaryMs:500,leftSourceText:'First',rightSourceText:'First',leftTranslatedText:'第一',rightTranslatedText:'第一'}}}]);
  h.ctx.segments[0].source_text='later';assert.equal(h.history[0][0][0].source_text,'First');
  assert.equal(h.history[0][3],'a');assert.equal(h.applied[0][1],'left');assert.deepEqual(h.locks,[true,false]);
});
test('Join retains language spacing, punctuation and missing-translation rules in IPC',async()=>{
  for(const [sourceLanguage,targetLanguage,left,right,translation,expectedSource,expectedTranslation] of [
    ['en','zh','First','second','第二','First second','第一第二'],
    ['ja','ko','最初','次','둘째','最初次','第一 둘째'],
    ['en','zh','First',', second','第二','First, second','第一第二'],
    ['en','zh','First','second',null,'First second',null],
  ]){
    const h=harness();h.change({sourceLanguage,targetLanguage,segments:[cue('left',0,1000,left,'第一'),cue('right',1000,2000,right,translation)]});await h.actions.join();
    assert.deepEqual(h.calls[0],{command:'merge_subtitles',args:{request:{jobId:'a',leftSegmentId:'left',rightSegmentId:'right',sourceText:expectedSource,translatedText:expectedTranslation}}});
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
