import test from 'node:test';
import assert from 'node:assert/strict';
import { createDesktopStartup } from '../src/desktop-startup.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const settle=()=>new Promise(resolve=>setImmediate(resolve));
function fixture(overrides={}) {
 const events=[];const options={jobs:[],directory:async()=>'/isolated',directoryAccepted:path=>events.push(['directory',path]),directoryFailed:()=>events.push(['directoryFailed']),failed:(name,e)=>events.push(['failed',name,String(e)]),...overrides};return {startup:createDesktopStartup(options),events};
}
test('all independent reads start once without awaiting another read',async()=>{
 const wait=deferred(),started=[];const f=fixture({jobs:[{name:'settings',run:()=>{started.push('settings');return wait.promise;}},{name:'tasks',run:async()=>{started.push('tasks');}}]});assert.equal(f.startup.active,false);f.startup.mount();f.startup.mount();assert.equal(f.startup.active,true);assert.deepEqual(started,['settings','tasks']);await settle();assert.deepEqual(f.events,[['directory','/isolated']]);wait.resolve();f.startup.dispose();assert.equal(f.startup.active,false);
});
test('a synchronous or async job failure does not block other startup reads',async()=>{
 let tasks=0;const f=fixture({jobs:[{name:'sync',run:()=>{throw Error('sync failure');}},{name:'async',run:async()=>{throw Error('async failure');}},{name:'tasks',run:async()=>{tasks++;}}]});f.startup.mount();await settle();assert.equal(tasks,1);assert.deepEqual(f.events.filter(e=>e[0]==='failed').map(e=>e[1]),['sync','async']);assert.equal(f.events.some(e=>e[0]==='directory'),true);f.startup.dispose();
});
test('loaders receive a lifecycle guard for accepting their own read results',async()=>{
 let current;const f=fixture({jobs:[{name:'guarded',run:async guard=>{current=guard;}}]});f.startup.mount();assert.equal(current(),true);f.startup.dispose();assert.equal(current(),false);
});
test('late directory success and failure cannot update a disposed page',async()=>{
 for(const reject of [false,true]){const wait=deferred(),f=fixture({directory:()=>wait.promise});f.startup.mount();f.startup.dispose();if(reject)wait.reject(Error('old directory'));else wait.resolve('/old');await settle();assert.deepEqual(f.events,[]);}
});
test('directory failure is reported independently of successful jobs',async()=>{
 for(const sync of [false,true]){const f=fixture({directory:()=>{if(sync)throw Error('sync');return Promise.reject(Error('async'));}});f.startup.mount();await settle();assert.deepEqual(f.events,[['directoryFailed']]);f.startup.dispose();}
});
test('late loader rejection is ignored and dispose prevents subsequent mount',async()=>{
 const wait=deferred();let runs=0;const f=fixture({jobs:[{name:'held',run:()=>{runs++;return wait.promise;}}],directory:async()=>'/path'});f.startup.mount();f.startup.dispose();f.startup.mount();wait.reject(Error('disposed'));await settle();assert.equal(runs,1);assert.deepEqual(f.events,[]);
});
