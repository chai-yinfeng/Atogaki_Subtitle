import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorContextStatus } from '../src/editor-context-status.ts';
import { createEditorBackend } from '../src/editor-backend.ts';
const settle=()=>new Promise(resolve=>setImmediate(resolve));
function harness(){
 let context={jobId:'a',navigation:1},ids=[],denied=false;
 const reads=[],applied=[],failed=[];
 const status=createEditorContextStatus({
  context:()=>{assert.equal(denied,false,'disposed module cannot read current UI');return context;},
  ids:()=>ids,
  read:jobId=>new Promise((resolve,reject)=>reads.push({jobId,resolve,reject})),
  apply:(state,changed)=>{applied.push({state,changed});ids=state.translation_context_stale_ids;},failed:message=>failed.push(message),
 });
 return {status,reads,applied,failed,context:value=>context=value,ids:value=>ids=value,deny:()=>denied=true};
}
const state=ids=>({workspace_revision:42,translation_context_stale_ids:ids});
test('lightweight adapter preserves only task identity and metadata response, never reloads full detail',async()=>{
 const calls=[],response=state(['consumer']);
 const backend=createEditorBackend(async(command,args)=>{calls.push({command,args});return response;});
 assert.equal(await backend.contextStatus('a'),response);
 assert.deepEqual(calls,[{command:'get_translation_context_state',args:{jobId:'a'}}]);
});
test('added and cleared consumer IDs patch precisely; duplicate or reordered IDs do not trigger unnecessary redraw',async()=>{
 const h=harness();h.ids(['old','same']);h.status.refresh('a');h.reads[0].resolve(state(['same','new','new']));await settle();
 assert.deepEqual(h.applied[0].changed,['old','new']);
 h.status.refresh('a');h.reads[1].resolve(state(['new','same']));await settle();assert.equal(h.applied.length,1);
 h.status.refresh('a');h.reads[2].resolve(state([]));await settle();assert.deepEqual(h.applied[1].changed,['same','new']);
});
test('rapid completed saves coalesce to the newest pending read; old success never briefly applies',async()=>{
 const h=harness();h.status.refresh('a');h.status.refresh('a');h.status.refresh('a');assert.equal(h.reads.length,1);
 h.reads[0].resolve(state(['old']));await settle();assert.deepEqual(h.applied,[]);assert.equal(h.reads.length,2);
 h.reads[1].resolve(state(['new']));await settle();assert.deepEqual(h.applied.map(x=>x.state.translation_context_stale_ids),[['new']]);
});
test('new detail or structural metadata invalidates queued reads and old errors without overwriting current state',async()=>{
 for(const fail of [false,true]){
  const h=harness();h.status.refresh('a');h.status.refresh('a');h.status.invalidate();h.ids(['new-structure']);
  if(fail)h.reads[0].reject(Error('old'));else h.reads[0].resolve(state(['old']));await settle();
  assert.deepEqual(h.applied,[]);assert.deepEqual(h.failed,[]);assert.equal(h.reads.length,1);
  h.status.refresh('a');h.reads[1].resolve(state(['new-text']));await settle();assert.deepEqual(h.applied[0].changed,['new-structure','new-text']);
 }
});
test('task or same-task visit change suppresses late metadata; an inactive job save cannot invalidate the new task read',async()=>{
 for(const next of [{jobId:'b',navigation:2},{jobId:'a',navigation:2}]){
  const h=harness();h.status.refresh('a');h.context(next);h.reads[0].resolve(state(['old']));await settle();assert.deepEqual(h.applied,[]);
 }
 const h=harness();h.context({jobId:'b',navigation:2});h.status.refresh('b');h.status.refresh('a');assert.equal(h.reads.length,1);
 h.reads[0].resolve(state(['b-consumer']));await settle();assert.equal(h.applied.length,1);
});
test('read failure preserves existing status and saved data, reports metadata failure once and waits for explicit next refresh',async()=>{
 const h=harness();h.ids(['existing']);h.status.refresh('a');h.reads[0].reject(Error('busy database'));await settle();
 assert.deepEqual(h.applied,[]);assert.equal(h.failed.length,1);assert.match(h.failed[0],/文字已保存.*状态暂未同步.*busy database/);assert.equal(h.reads.length,1);
 h.status.refresh('a');h.reads[1].resolve(state(['recovered']));await settle();assert.deepEqual(h.applied[0].changed,['existing','recovered']);
});
test('dispose releases queued work and prevents reads, status callbacks and context access from late settlement',async()=>{
 for(const fail of [false,true]){
  const h=harness();h.status.refresh('a');h.status.refresh('a');h.status.dispose();h.status.dispose();h.deny();
  if(fail)h.reads[0].reject(Error('late'));else h.reads[0].resolve(state(['late']));await settle();
  h.status.refresh('a');h.status.invalidate();assert.equal(h.reads.length,1);assert.deepEqual(h.applied,[]);assert.deepEqual(h.failed,[]);
 }
});
