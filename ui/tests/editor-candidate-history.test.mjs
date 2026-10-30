import test from 'node:test';
import assert from 'node:assert/strict';
import { createCandidateHistory } from '../src/editor-candidate-history.ts';
function harness(){
 let context={jobId:'a',navigation:1},visible=true;
 const calls=[],events=[];
 const deferred=(kind,args=[])=>new Promise((resolve,reject)=>calls.push({kind,args,resolve,reject}));
 const history=createCandidateHistory({context:()=>context,visible:()=>visible,list:job=>deferred('list',[job]),release:(...args)=>deferred('release',args),confirmRelease:()=>deferred('confirm'),rows:r=>events.push(['rows',r]),busy:(...args)=>events.push(['busy',...args]),released:(...args)=>events.push(['released',...args]),failed:e=>events.push(['error',String(e)])});
 return {history,calls,events,setContext:c=>context=c,hide:()=>visible=false};
}
const run=id=>({id,cleanable:true,status:'succeeded'});
async function ready(h){const read=h.history.refresh();h.calls.at(-1).resolve([run('r')]);await read;}
const tick=()=>new Promise(resolve=>setImmediate(resolve));
test('history accepts only latest reads, including errors',async()=>{
 const h=harness(),a=h.history.refresh(),b=h.history.refresh();h.calls[1].resolve([run('new')]);await b;h.calls[0].reject(Error('old'));await a;assert.deepEqual(h.events,[['rows',[run('new')]]]);
});
test('confirmation is deduplicated and closing invalidates authorization',async()=>{
 const h=harness();await ready(h);const release=h.history.release('r');await h.history.release('r');assert.equal(h.calls.filter(c=>c.kind==='confirm').length,1);h.history.reset();h.hide();h.calls[1].resolve(true);await release;assert.equal(h.calls.some(c=>c.kind==='release'),false);
});
test('task navigation during confirmation never sends release',async()=>{
 const h=harness();await ready(h);const release=h.history.release('r');h.setContext({jobId:'a',navigation:2});h.calls[1].resolve(true);await release;assert.equal(h.calls.length,2);
});
test('late release results cannot touch a new history and reopening has a new epoch',async()=>{
 const h=harness();await ready(h);const release=h.history.release('r');h.calls[1].resolve(true);await tick();assert.deepEqual(h.calls[2].args,['a','r']);h.history.reset();await ready(h);const count=h.events.length;h.calls[2].resolve(100);await release;assert.equal(h.events.length,count);
});
test('successful release reports bytes, refreshes, and keeps other requests serial per run',async()=>{
 const h=harness();await ready(h);const release=h.history.release('r');h.calls[1].resolve(true);await tick();h.calls[2].resolve(123);await tick();assert.equal(h.calls[3].kind,'list');h.calls[3].resolve([{...run('r'),released:true,cleanable:false}]);await release;assert.deepEqual(h.events.find(e=>e[0]==='released'),['released','r',123]);assert.deepEqual(h.events.at(-1),['busy','r',false]);
});
test('unlisted or protected runs never prompt; failed confirmation releases lock',async()=>{
 const h=harness(),read=h.history.refresh();h.calls[0].resolve([{...run('protected'),cleanable:false}]);await read;await h.history.release('protected');await h.history.release('missing');assert.equal(h.calls.length,1);await ready(h);const release=h.history.release('r');h.calls[2].reject(Error('dialog failed'));await release;assert.match(h.events.find(e=>e[0]==='error')[1],/dialog failed/);assert.deepEqual(h.events.at(-1),['busy','r',false]);
});
test('dispose invalidates pending reads and forbids new commands',async()=>{
 const h=harness(),read=h.history.refresh();h.history.dispose();h.calls[0].resolve([run('r')]);await read;await h.history.refresh();await h.history.release('r');assert.equal(h.calls.length,1);assert.deepEqual(h.events,[['rows',[]]]);
});
test('committed adoption invalidates old reads and immediately removes cleanup permission',async()=>{
 const h=harness();await ready(h);const old=h.history.refresh();h.history.markAdopted('r');const adopted=h.events.at(-1)[1][0];assert.equal(adopted.adopted,true);assert.equal(adopted.cleanable,false);
 h.calls[1].resolve([run('r')]);await old;assert.deepEqual(h.events.at(-1),['rows',[adopted]]);await h.history.release('r');assert.equal(h.calls.length,2);
});
test('adoption during a pending cleanup confirmation prevents release',async()=>{
 const h=harness();await ready(h);const cleanup=h.history.release('r');h.history.markAdopted('r');h.calls[1].resolve(true);await cleanup;assert.equal(h.calls.some(c=>c.kind==='release'),false);
});

test('completed cleanup disappears, partial cleanup remains retryable, and source records stay visible',async()=>{
 const h=harness(),read=h.history.refresh();
 const partial={...run('partial'),released:true,cleanup_pending:true};
 const adopted={...run('adopted'),adopted:true,cleanable:false};
 const original={...run('original'),is_candidate:false,cleanable:false};
 h.calls[0].resolve([{...run('done'),released:true},partial,adopted,original]);await read;
 assert.deepEqual(h.events.at(-1),['rows',[partial,adopted,original]]);
});
test('failed cleanup keeps its reason and retry removes the row immediately on success',async()=>{
 const h=harness();await ready(h);
 const failed=h.history.release('r');h.calls[1].resolve(true);await tick();h.calls[2].reject(Error('permission denied'));await failed;
 const row=h.events.filter(e=>e[0]==='rows').at(-1)[1][0];assert.match(row.cleanupError,/permission denied/);assert.equal(row.cleanable,true);
 const retry=h.history.release('r');h.calls[3].resolve(true);await tick();h.calls[4].resolve(0);await tick();
 assert.deepEqual(h.events.filter(e=>e[0]==='rows').at(-1),['rows',[]]);
 h.calls[5].resolve([{...run('r'),released:true,cleanable:false}]);await retry;
 assert.deepEqual(h.events.filter(e=>e[0]==='rows').at(-1),['rows',[]]);
});
