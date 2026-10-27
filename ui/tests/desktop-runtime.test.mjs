import test from 'node:test';
import assert from 'node:assert/strict';
import { createDesktopRuntime } from '../src/desktop-runtime.ts';
const settle=()=>new Promise(resolve=>setImmediate(resolve));
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
function fixture(overrides={}) {
 const callbacks={},timers=new Map(),events=[];let timerId=0;
 const subscribe=name=>async handler=>{callbacks[name]=handler;events.push(['subscribe',name]);return ()=>events.push(['release',name]);};
 const options={refreshTasks:async()=>events.push(['refresh']),updateTimings:()=>events.push(['timings']),subscribeVisibility:subscribe('visibility'),subscribePlayback:subscribe('playback'),subscribeClose:subscribe('close'),visibility:value=>events.push(['visibility',value]),playback:value=>events.push(['playback',value]),close:event=>{event.preventDefault();events.push(['close']);},failed:error=>events.push(['failed',String(error)]),...overrides};
 const runtime=createDesktopRuntime(options,{repeat:(tick,delay)=>{timers.set(++timerId,{tick,delay});return timerId;},cancel:id=>timers.delete(id)});
 return {runtime,timers,callbacks,events};
}
test('mount is idempotent and registers exactly the existing timer periods and events',async()=>{
 const f=fixture();f.runtime.mount();f.runtime.mount();await settle();assert.deepEqual([...f.timers.values()].map(t=>t.delay),[2000,1000]);assert.equal(f.events.filter(e=>e[0]==='subscribe').length,3);
 for(const timer of f.timers.values())timer.tick();await settle();assert.equal(f.events.filter(e=>e[0]==='refresh').length,1);assert.equal(f.events.filter(e=>e[0]==='timings').length,1);f.runtime.dispose();
});
test('event values and synchronous close prevention preserve handler semantics',async()=>{
 const f=fixture();f.runtime.mount();await settle();await f.callbacks.visibility(true);await f.callbacks.playback('pause');let prevented=false;const close=f.callbacks.close({preventDefault:()=>{prevented=true;}});assert.equal(prevented,true);await close;
 assert.deepEqual(f.events.slice(-3),[['visibility',true],['playback','pause'],['close']]);f.runtime.dispose();
});
test('dispose releases timers and subscriptions once, and blocks retained callbacks',async()=>{
 const f=fixture();f.runtime.mount();await settle();const ticks=[...f.timers.values()].map(t=>t.tick);f.runtime.dispose();f.runtime.dispose();f.runtime.mount();assert.equal(f.timers.size,0);assert.equal(f.events.filter(e=>e[0]==='release').length,3);const count=f.events.length;
 for(const tick of ticks)tick();await f.callbacks.visibility(false);await f.callbacks.playback('play');await f.callbacks.close({preventDefault:()=>assert.fail('disposed close')});await settle();assert.equal(f.events.length,count);
});
test('registration completed after disposal is immediately released, not retained',async()=>{
 const wait=deferred();let callback,released=0;const f=fixture({subscribeVisibility:handler=>{callback=handler;return wait.promise;}});f.runtime.mount();await settle();f.runtime.dispose();wait.resolve(()=>{released++;});await settle();assert.equal(released,1);await callback(true);assert.equal(f.events.some(e=>e[0]==='visibility'),false);
});
test('current registration failures are reported without blocking other resources',async()=>{
 const f=fixture({subscribeVisibility:()=>{throw Error('sync registration');},subscribePlayback:async()=>{throw Error('async registration');}});f.runtime.mount();await settle();assert.equal(f.events.filter(e=>e[0]==='failed').length,2);assert.equal(f.events.some(e=>e[0]==='subscribe'&&e[1]==='close'),true);assert.equal(f.timers.size,2);f.runtime.dispose();
});
test('late registration or handler errors do not present into a disposed page',async()=>{
 const registration=deferred(),handler=deferred();const f=fixture({subscribeVisibility:()=>registration.promise,playback:()=>handler.promise});f.runtime.mount();await settle();const playback=f.callbacks.playback('play');f.runtime.dispose();registration.reject(Error('late subscribe'));handler.reject(Error('late handler'));await playback;await settle();assert.equal(f.events.some(e=>e[0]==='failed'),false);
});
test('async refresh failures are observed and one failed release does not skip others',async()=>{
 const f=fixture({refreshTasks:async()=>{throw Error('refresh failure');},subscribeVisibility:async()=>()=>{throw Error('release failure');}});f.runtime.mount();await settle();[...f.timers.values()][0].tick();await settle();assert.match(f.events.find(e=>e[0]==='failed')[1],/refresh failure/);f.runtime.dispose();assert.equal(f.events.filter(e=>e[0]==='release').length,2);assert.equal(f.timers.size,0);
});
