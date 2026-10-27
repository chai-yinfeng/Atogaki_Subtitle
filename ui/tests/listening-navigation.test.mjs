import test from 'node:test';
import assert from 'node:assert/strict';
import { createListeningNavigation } from '../src/listening-navigation.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const settle=()=>new Promise(resolve=>setImmediate(resolve));
const detail=id=>({job:{job_id:id}});
function fixture(overrides={}) {
 let navigation=0,area='listening';const events=[];
 const log=name=>(...args)=>{events.push([name,...args]);};
 const options={begin:()=>++navigation,current:n=>n===navigation&&area==='listening',stop:async()=>{},prepare:log('prepare'),detail:async id=>detail(id),position:async()=>4500,learning:async()=>log('learning')(),apply:log('apply'),play:async()=>log('play')(),failed:log('failed'),...overrides};
 return {session:createListeningNavigation(options),events,leave:()=>{navigation++;area='workbench';}};
}
test('detail and position read concurrently, then learning and shared media apply',async()=>{
 const d=deferred(),p=deferred(),started=[];const f=fixture({detail:id=>{started.push(['detail',id]);return d.promise;},position:id=>{started.push(['position',id]);return p.promise;}});
 const flight=f.session.open('a');await settle();assert.deepEqual(started,[['detail','a'],['position','a']]);p.resolve(4500);await settle();assert.deepEqual(f.events,[['prepare']]);d.resolve(detail('a'));await flight;assert.deepEqual(f.events,[['prepare'],['learning'],['apply',detail('a'),4500]]);
});
test('explicit zero or another source position overrides resume and autoplay is opt in',async()=>{
 for(const override of [0,9000]){const f=fixture();await f.session.open('a',override,true);assert.deepEqual(f.events,[['prepare'],['learning'],['apply',detail('a'),override],['play']]);}
});
test('failed draft/time preflight prevents any read or view preparation',async()=>{
 const f=fixture({stop:async()=>{throw Error('unsaved draft');}});await assert.rejects(f.session.open('a'),/unsaved draft/);assert.deepEqual(f.events,[]);
});
test('leaving during preflight prevents reads',async()=>{
 const wait=deferred();let reads=0;const f=fixture({stop:()=>wait.promise,detail:async()=>{reads++;return detail('a');}});const flight=f.session.open('a');f.leave();wait.resolve();await flight;assert.equal(reads,0);assert.deepEqual(f.events,[]);
});
test('older same-job detail or position cannot mount media after a newer open',async()=>{
 for(const held of ['detail','position']) {
  const wait=deferred();let count=0;const f=fixture({[held]:id=>++count===1?wait.promise:Promise.resolve(held==='detail'?detail(id):9000)});
  const old=f.session.open('a');await settle();await f.session.open('a');wait.resolve(held==='detail'?detail('a'):4500);await old;
  assert.equal(f.events.filter(e=>e[0]==='apply').length,1);assert.deepEqual(f.events.find(e=>e[0]==='apply'),['apply',detail('a'),held==='position'?9000:4500]);
 }
});
test('leaving during learning preparation prevents mounting and autoplay',async()=>{
 const wait=deferred();const f=fixture({learning:()=>wait.promise});const flight=f.session.open('a',null,true);await settle();f.leave();wait.resolve();await flight;assert.deepEqual(f.events,[['prepare']]);
});
test('mismatched task detail fails before reading learning or applying media',async()=>{
 const f=fixture({detail:async()=>detail('b')});await f.session.open('a');assert.deepEqual(f.events.map(e=>e[0]),['prepare','failed']);assert.match(String(f.events[1][1]),/身份不匹配/);
});
test('current read failures are shown; old play rejection is ignored after leaving',async()=>{
 const failed=fixture({position:async()=>{throw Error('position failed');}});await failed.session.open('a');assert.match(String(failed.events.at(-1)[1]),/position failed/);
 const wait=deferred(),f=fixture({play:()=>wait.promise});const flight=f.session.open('a',null,true);await settle();f.leave();wait.reject(Error('old play'));await flight;assert.equal(f.events.some(e=>e[0]==='failed'),false);
});
test('dispose ignores late read errors and prevents future opens',async()=>{
 const wait=deferred();const f=fixture({detail:()=>wait.promise});const flight=f.session.open('a');await settle();f.session.dispose();wait.reject(Error('disposed'));await flight;await f.session.open('b');assert.deepEqual(f.events,[['prepare']]);
});
