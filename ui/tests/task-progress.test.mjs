import test from 'node:test';
import assert from 'node:assert/strict';
import { createTaskProgress } from '../src/task-progress.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
const settle=()=>new Promise(resolve=>setImmediate(resolve));
function fixture() {
 let context={jobId:'a',navigation:1},next=0;const timers=new Map(),requests=[],events=[];
 const poller=createTaskProgress({context:()=>context,read:id=>{const wait=deferred();requests.push({id,...wait});return wait.promise;},apply:d=>events.push(['apply',d]),failed:e=>events.push(['failed',String(e)])},{repeat:fn=>{timers.set(++next,fn);return next;},cancel:id=>timers.delete(id)});
 return {poller,timers,requests,events,setContext:value=>{context=value;},tick:()=>{for(const fn of timers.values())fn();}};
}
test('only mounted eligible progress contexts read; mounting twice creates one timer',async()=>{
 const f=fixture();f.tick();assert.equal(f.requests.length,0);f.poller.mount();f.poller.mount();assert.equal(f.timers.size,1);
 f.setContext(null);f.tick();assert.equal(f.requests.length,0);
 f.setContext({jobId:'a',navigation:1});f.tick();f.requests[0].resolve({job:{job_id:'a'},status:'processing'});await settle();assert.equal(f.events[0][0],'apply');f.poller.dispose();
});
test('slow reads never overlap and another tick can retry after failure',async()=>{
 const f=fixture();f.poller.mount();f.tick();f.tick();f.tick();assert.equal(f.requests.length,1);f.requests[0].reject(Error('current failure'));await settle();assert.match(f.events[0][1],/current failure/);f.tick();assert.equal(f.requests.length,2);f.requests[1].resolve({job:{job_id:'a'}});await settle();assert.equal(f.events[1][0],'apply');f.poller.dispose();
});
test('same-job navigation and other-job visits reject late results and errors',async()=>{
 for(const jobId of ['a','b'])for(const reject of [false,true]) {
  const f=fixture();f.poller.mount();f.tick();f.setContext({jobId,navigation:2});
  if(reject)f.requests[0].reject(Error('old'));else f.requests[0].resolve({job:{job_id:'a'}});await settle();assert.deepEqual(f.events,[]);f.poller.dispose();
 }
});
test('becoming ineligible while a read runs protects editing or completed state',async()=>{
 const f=fixture();f.poller.mount();f.tick();f.setContext(null);f.requests[0].resolve({job:{job_id:'a'}});await settle();assert.deepEqual(f.events,[]);f.tick();assert.equal(f.requests.length,1);f.poller.dispose();
});
test('unmount and remount invalidate an old read even with identical context',async()=>{
 const f=fixture();f.poller.mount();f.tick();f.poller.unmount();assert.equal(f.timers.size,0);f.poller.mount();f.tick();assert.equal(f.requests.length,1);f.requests[0].resolve({job:{job_id:'a'}});await settle();assert.deepEqual(f.events,[]);f.tick();assert.equal(f.requests.length,2);f.requests[1].resolve({job:{job_id:'a'}});await settle();assert.equal(f.events.length,1);f.poller.dispose();
});
test('wrong task identity is rejected without applying the detail',async()=>{
 const f=fixture();f.poller.mount();f.tick();f.requests[0].resolve({job:{job_id:'b'}});await settle();assert.equal(f.events.length,1);assert.equal(f.events[0][0],'failed');assert.match(f.events[0][1],/身份不匹配/);f.poller.dispose();
});
test('completion may unmount inside apply and no timer survives',async()=>{
 let reads=0,callback;const timers=new Map();let poller;
 poller=createTaskProgress({context:()=>({jobId:'a',navigation:1}),read:async()=>{reads++;return {job:{job_id:'a'}};},apply:()=>poller.unmount(),failed:()=>assert.fail('unexpected error')},{repeat:fn=>{callback=fn;timers.set(1,fn);return 1;},cancel:id=>timers.delete(id)});
 poller.mount();callback();await settle();assert.equal(timers.size,0);callback();assert.equal(reads,1);poller.dispose();
});
test('dispose cancels the timer, ignores pending errors and prevents remount',async()=>{
 const f=fixture();f.poller.mount();f.tick();f.poller.dispose();f.poller.mount();f.requests[0].reject(Error('departed'));await settle();assert.deepEqual(f.events,[]);assert.equal(f.timers.size,0);
});
