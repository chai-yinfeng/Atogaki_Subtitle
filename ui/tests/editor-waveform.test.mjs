import test from 'node:test';
import assert from 'node:assert/strict';
import { createEditorWaveform } from '../src/editor-waveform.ts';
const waveform=(start,end=start+30000)=>({start_ms:start,end_ms:end,duration_ms:1200000,point_duration_ms:20,peaks:[{min:-.5,max:.5}]});
function harness(){
 let context={jobId:'a',navigation:1,durationMs:1200000,windowMs:30000},id=0,points=800;
 const timers=new Map(),requests=[],events=[];
 const loader=createEditorWaveform({context:()=>context,points:()=>points,request:(jobId,start,end,count)=>new Promise((resolve,reject)=>requests.push({jobId,start,end,count,resolve,reject})),started:()=>events.push('start'),accepted:w=>events.push(['accepted',w.start_ms]),failed:e=>events.push(['failed',String(e)])},{schedule:callback=>{timers.set(++id,callback);return id;},cancel:id=>timers.delete(id)});
 return {loader,requests,events,timers,setContext:value=>context=value,setPoints:value=>points=value,context:()=>context,flush(){const callbacks=[...timers.values()];timers.clear();callbacks.forEach(cb=>cb());}};
}
test('the latest viewport response wins even if an older request resolves last',async()=>{
 const h=harness(),old=h.loader.load(1000,true),next=h.loader.load(40000,true);
 h.requests[1].resolve(waveform(40000));await next;assert.equal(h.loader.window.start_ms,40000);
 h.requests[0].resolve(waveform(1000));await old;assert.equal(h.loader.window.start_ms,40000);
 assert.deepEqual(h.events.filter(e=>Array.isArray(e)),[['accepted',40000]]);assert.equal(h.loader.loading,false);
});
test('viewport intent invalidates in-flight data before the coalescing timer fires',async()=>{
 const h=harness(),first=h.loader.load(0,true);
 h.loader.schedule(1000);h.loader.schedule(2000);h.loader.schedule(3000);
 assert.equal(h.timers.size,1);assert.equal(h.requests.length,1);assert.equal(h.loader.startMs,3000);
 h.requests[0].resolve(waveform(0));await first;assert.equal(h.loader.window,null);
 h.flush();assert.equal(h.requests.length,2);assert.equal(h.requests[1].start,3000);
 h.requests[1].resolve(waveform(3000));await Promise.resolve();assert.equal(h.loader.window.start_ms,3000);
});
test('a direct load supersedes queued panning and stale errors cannot clear a newer viewport',async()=>{
 const h=harness();h.loader.schedule(500);const old=h.loader.load(1000,true);assert.equal(h.timers.size,0);
 const current=h.loader.load(2000,true);h.requests[1].resolve(waveform(2000));await current;
 h.requests[0].reject(Error('stale'));await old;assert.equal(h.loader.window.start_ms,2000);
 assert.equal(h.events.some(e=>Array.isArray(e)&&e[0]==='failed'),false);
 const failed=h.loader.load(3000,true);h.requests[2].reject(Error('current'));await failed;
 assert.equal(h.loader.window,null);assert.equal(h.loader.loading,false);assert.match(h.events.at(-1)[1],/current/);
});
test('job and navigation identity, reset and dispose invalidate requests and scheduled work',async()=>{
 const h=harness(),old=h.loader.load(0,true);
 h.setContext({...h.context(),navigation:3});h.requests[0].resolve(waveform(0));await old;assert.equal(h.loader.window,null);
 h.loader.schedule(100);h.loader.reset();assert.equal(h.timers.size,0);assert.equal(h.loader.startMs,0);
 const last=h.loader.load(500,true);h.loader.schedule(900);h.loader.dispose();assert.equal(h.timers.size,0);
 h.requests[1].resolve(waveform(500));await last;assert.equal(h.loader.window,null);assert.equal(h.loader.loading,false);
 h.loader.schedule(200);await h.loader.load(200);assert.equal(h.requests.length,2);
});
test('window anchors and peak counts remain bounded for long and not-yet-known media',async()=>{
 const h=harness();h.setPoints(9000);const late=h.loader.load(1200000);assert.equal(h.requests[0].start,1170000);assert.equal(h.requests[0].count,2400);
 h.requests[0].resolve(waveform(1170000));await late;
 h.loader.reset();h.setContext({...h.context(),durationMs:Infinity});h.setPoints(10);
 const initial=h.loader.load(0);assert.equal(h.requests[1].start,0);assert.equal(h.requests[1].end,30000);assert.equal(h.requests[1].count,240);
 h.requests[1].resolve(waveform(0));await initial;
});
