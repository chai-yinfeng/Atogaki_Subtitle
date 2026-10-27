import test from 'node:test';
import assert from 'node:assert/strict';
import { createTaskNavigation } from '../src/task-navigation.ts';
import { createEditorNavigation } from '../src/editor-navigation.ts';
const detail=(id='a',editable=false)=>({job:{job_id:id},editable});
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
function fixture(overrides={}) {
 let navigation=0,visible=false;const events=[];
 const log=name=>(...args)=>{events.push([name,...args]);};
 const options={begin:()=>++navigation,current:n=>n===navigation,visible:()=>visible,stop:async()=>{},knownEditable:()=>false,read:async id=>detail(id),editable:d=>d.editable,editor:async(...args)=>log('editor')(...args),prepare:section=>{visible=true;log('prepare')(section);},accept:log('accept'),render:log('render'),translation:async()=>log('translation')(),failed:log('failed'),...overrides};
 return {session:createTaskNavigation(options),events,options,leave:()=>{navigation++;visible=false;}};
}
test('completed card opens editor without a duplicate progress read',async()=>{
 const f=fixture({knownEditable:()=>true,read:async()=>{throw Error('duplicate read');}});await f.session.open('a');assert.deepEqual(f.events,[['editor','a',undefined]]);
});
test('explicit translation and export retain the requested tool even for completed tasks',async()=>{
 for(const section of ['translation','export']) {
  const d=detail('a',true),f=fixture({knownEditable:()=>true,read:async()=>d});await f.session.open('a',section);
  assert.deepEqual(f.events,[['prepare',section],['accept',d],['render',d],['translation']]);
 }
});
test('fresh completed detail routes to editor and reuses the loaded result',async()=>{
 const d=detail('a',true),f=fixture({read:async()=>d});await f.session.open('a');assert.deepEqual(f.events,[['prepare','review'],['accept',d],['editor','a',d]]);
});
test('stale completed summary and incomplete detail settle in progress with one read',async()=>{
 let navigation=0,visible=false,reads=0,rendered=0;const d=detail();let task,editor;
 const begin=()=>++navigation,current=n=>n===navigation;
 task=createTaskNavigation({begin,current,visible:()=>visible,stop:async()=>{},knownEditable:()=>true,read:async()=>{reads++;throw Error('must reuse editor detail');},editable:d=>d.editable,editor:id=>editor.open(id),prepare:()=>{visible=true;},accept:()=>{},render:()=>{rendered++;},translation:async()=>{},failed:e=>{throw e;}});
 editor=createEditorNavigation({begin,current,stop:async()=>{},prepare:()=>{},read:async()=>{reads++;return d;},editable:d=>d.editable,progress:(id,d)=>task.open(id,'review',d),apply:()=>{throw Error('incomplete editor');},waveform:async()=>{},processingRange:()=>{},signals:async()=>{},renderList:()=>{},translation:async()=>{},failed:e=>{throw e;}});
 await task.open('a');assert.equal(reads,1);assert.equal(rendered,1);assert.equal(visible,true);
});
test('leave preflight failure never prepares or reads another task',async()=>{
 const f=fixture({stop:async()=>{throw Error('draft failed');}});await assert.rejects(f.session.open('a'),/draft failed/);assert.deepEqual(f.events,[]);
});
test('navigation during preflight prevents detail reads and view changes',async()=>{
 const wait=deferred();let reads=0;const f=fixture({stop:()=>wait.promise,read:async()=>{reads++;return detail();}});const flight=f.session.open('a');f.leave();wait.resolve();await flight;assert.equal(reads,0);assert.deepEqual(f.events,[]);
});
test('old detail or errors cannot replace a newer progress visit',async()=>{
 for(const reject of [false,true]) {
  const wait=deferred();const f=fixture({read:id=>id==='a'?wait.promise:Promise.resolve(detail(id))});const old=f.session.open('a');await new Promise(r=>setImmediate(r));await f.session.open('b');
  if(reject)wait.reject(Error('old read'));else wait.resolve(detail());await old;
  assert.deepEqual(f.events.filter(e=>e[0]==='render').map(e=>e[1].job.job_id),['b']);assert.equal(f.events.some(e=>e[0]==='failed'),false);
 }
});
test('wrong task detail is rejected before adoption or rendering',async()=>{
 const f=fixture({read:async()=>detail('b')});await f.session.open('a');assert.deepEqual(f.events.map(e=>e[0]),['prepare','failed']);assert.match(String(f.events[1][1]),/身份不匹配/);
});
test('dispose rejects late work and prevents new opens',async()=>{
 const wait=deferred();const f=fixture({read:()=>wait.promise});const flight=f.session.open('a');await new Promise(r=>setImmediate(r));f.session.dispose();wait.resolve(detail());await flight;await f.session.open('b');assert.deepEqual(f.events,[['prepare','review']]);
});
