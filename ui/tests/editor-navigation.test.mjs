import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorNavigation } from '../src/editor-navigation.ts';
const deferred = () => { let resolve,reject; const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject}; };
const detail = (id='a',editable=true) => ({job:{job_id:id},editable});
function fixture(overrides={}) {
  let navigation=0,area='karaoke';const events=[];
  const record = name => (...args) => {events.push([name,...args]);};
  const options={begin:()=>++navigation,current:n=>n===navigation&&area==='karaoke',stop:async()=>{events.push(['stop']);},prepare:record('prepare'),read:async id=>detail(id),editable:d=>d.editable,progress:async id=>{events.push(['progress',id]);},apply:record('apply'),waveform:async()=>{events.push(['waveform']);},processingRange:record('range'),signals:async id=>{events.push(['signals',id]);},renderList:record('list'),translation:async()=>{events.push(['translation']);},failed:record('failed'),...overrides};
  return {session:createEditorNavigation(options),events,leave:()=>{area='workbench';navigation++;},options};
}
test('open reuses supplied detail and coordinates existing services in order',async()=>{
 const f=fixture({read:async()=>{throw Error('must reuse detail');}});await f.session.open('a',detail());
 assert.deepEqual(f.events.map(e=>e[0]),['stop','prepare','apply','waveform','range','signals','list','translation']);
});
test('failed leave preflight preserves the current view and propagates the error',async()=>{
 const f=fixture({stop:async()=>{throw Error('unsaved time or failed draft');}});
 await assert.rejects(f.session.open('a'),/unsaved time/);assert.deepEqual(f.events,[]);
});
test('navigation while flushing prevents preparation or detail reads',async()=>{
 const wait=deferred();let reads=0;const f=fixture({stop:()=>wait.promise,read:async()=>{reads++;return detail();}});
 const flight=f.session.open('a');f.leave();wait.resolve();await flight;assert.equal(reads,0);assert.deepEqual(f.events,[]);
});
test('late detail and late errors cannot overwrite a newer editor visit',async()=>{
 for(const reject of [false,true]) {
  const wait=deferred();const f=fixture({read:id=>id==='a'?wait.promise:Promise.resolve(detail(id))});
  const old=f.session.open('a');await new Promise(r=>setImmediate(r));await f.session.open('b');
  if(reject)wait.reject(Error('old failure'));else wait.resolve(detail());await old;
  assert.deepEqual(f.events.filter(e=>e[0]==='apply').map(e=>e[1].job.job_id),['b']);assert.equal(f.events.some(e=>e[0]==='failed'),false);
 }
});
test('leaving during waveform stops range, review and translation initialization',async()=>{
 const wait=deferred();const f=fixture({waveform:()=>wait.promise});const flight=f.session.open('a');await new Promise(r=>setImmediate(r));f.leave();wait.resolve();await flight;
 assert.deepEqual(f.events.map(e=>e[0]),['stop','prepare','apply']);
});
test('leaving during review stops subsequent list and translation work',async()=>{
 const wait=deferred();const f=fixture({signals:()=>wait.promise});const flight=f.session.open('a');await new Promise(r=>setImmediate(r));f.leave();wait.resolve();await flight;
 assert.deepEqual(f.events.map(e=>e[0]),['stop','prepare','apply','waveform','range']);
});
test('incomplete tasks route to progress without mounting editor services',async()=>{
 const f=fixture();await f.session.open('a',detail('a',false));assert.deepEqual(f.events,[['stop'],['prepare','a'],['progress','a']]);
});
test('mismatched detail is rejected before mounting and current errors are shown',async()=>{
 const f=fixture();await f.session.open('a',detail('b'));assert.deepEqual(f.events.map(e=>e[0]),['stop','prepare','failed']);assert.match(String(f.events[2][1]),/身份不匹配/);
});
test('dispose invalidates pending work and prevents later opens',async()=>{
 const wait=deferred();const f=fixture({waveform:()=>wait.promise});const flight=f.session.open('a');await new Promise(r=>setImmediate(r));f.session.dispose();wait.resolve();await flight;await f.session.open('b');
 assert.deepEqual(f.events.map(e=>e[0]),['stop','prepare','apply']);
});
