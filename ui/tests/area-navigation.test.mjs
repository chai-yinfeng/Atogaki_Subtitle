import test from 'node:test';
import assert from 'node:assert/strict';
import { createAreaNavigation } from '../src/area-navigation.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const settle=()=>new Promise(resolve=>setImmediate(resolve));
function fixture(overrides={}) {
 let navigation=0,area='workbench';const events=[];
 const log=name=>(...args)=>{events.push([name,...args]);};
 const options={begin:()=>++navigation,current:n=>n===navigation,area:()=>area,stop:async()=>{},apply:value=>{area=value;log('area')(value);},refresh:log('refresh'),learning:async()=>log('learning')(),dictionary:async()=>log('dictionary')(),renderDictionary:log('renderDictionary'),selectedEditor:()=>null,editor:async()=>++navigation,scrollTop:log('scroll'),...overrides};
 return {navigation:createAreaNavigation(options),events,options,visit:()=>++navigation,setArea:value=>{area=value;}};
}
test('workbench and listening apply once then refresh and scroll',async()=>{
 for(const area of ['workbench','listening']){const f=fixture();await f.navigation.show(area);assert.deepEqual(f.events,[['area',area],['refresh'],['scroll']]);}
});
test('preflight rejection never applies an area or initializes its services',async()=>{
 const f=fixture({stop:async()=>{throw Error('unsaved time');}});await assert.rejects(f.navigation.show('learning'),/unsaved time/);assert.deepEqual(f.events,[]);
});
test('a newer visit during preflight prevents an old area from appearing',async()=>{
 const wait=deferred(),f=fixture({stop:()=>wait.promise});const flight=f.navigation.show('learning');f.visit();wait.resolve();await flight;assert.deepEqual(f.events,[]);
});
test('learning and dictionary prepare concurrently before rendering',async()=>{
 const a=deferred(),b=deferred(),started=[];const f=fixture({learning:()=>{started.push('learning');return a.promise;},dictionary:()=>{started.push('dictionary');return b.promise;}});const flight=f.navigation.show('learning');await settle();assert.deepEqual(started,['learning','dictionary']);a.resolve();await settle();assert.deepEqual(f.events,[['area','learning']]);b.resolve();await flight;assert.deepEqual(f.events,[['area','learning'],['renderDictionary'],['scroll']]);
});
test('late learning completion cannot render a dialog or scroll a newer same-area visit',async()=>{
 const wait=deferred();const f=fixture({learning:()=>wait.promise});const flight=f.navigation.show('learning');await settle();f.visit();wait.resolve();await flight;assert.deepEqual(f.events,[['area','learning'],['dictionary']]);
});
test('editor child navigation ID authorizes only its actual visit',async()=>{
 let f;const calls=[];f=fixture({selectedEditor:()=> 'selected',editor:async(id,d)=>{calls.push([id,d]);return f.visit();}});const d={job:'a'};await f.navigation.show('karaoke',d,'explicit');assert.deepEqual(calls,[['explicit',d]]);assert.deepEqual(f.events,[['area','karaoke'],['scroll']]);
});
test('editor fallback or newer same-area visit prevents stale scrolling',async()=>{
 for(const fallback of [false,true]) {
  const wait=deferred();let f;f=fixture({selectedEditor:()=> 'a',editor:()=>wait.promise});const flight=f.navigation.show('karaoke');await settle();const child=f.visit();if(!fallback)f.visit();wait.resolve(fallback?null:child);await flight;assert.deepEqual(f.events,[['area','karaoke']]);
 }
});
test('an empty editor selection presents the area without opening a task',async()=>{
 const f=fixture({editor:async()=>{assert.fail('no target');}});await f.navigation.show('karaoke');assert.deepEqual(f.events,[['area','karaoke'],['scroll']]);
});
test('dispose invalidates pending initialization and future shows',async()=>{
 const wait=deferred(),f=fixture({learning:()=>wait.promise});const flight=f.navigation.show('learning');await settle();f.navigation.dispose();wait.resolve();await flight;await f.navigation.show('workbench');assert.deepEqual(f.events,[['area','learning'],['dictionary']]);
});
