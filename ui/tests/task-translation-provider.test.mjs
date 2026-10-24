import test from 'node:test';
import assert from 'node:assert/strict';
import { createTaskTranslationProvider } from '../src/task-translation-provider.ts';
import { createTaskProviderBackend } from '../src/task-translation-provider-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const status=id=>({provider_id:id,provider:id,configured:id!=='none',model:null,endpoint_kind:'test',configuration_hint:null});
function harness(){
 let context={jobId:'a',navigation:1},read=async()=>status('deepseek'),save=async()=>{},dead=false;
 const reads=[],writes=[],accepted=[],messages=[],events=[];
 const session=createTaskTranslationProvider({context:()=>{if(dead)throw Error('disposed context read');return context;},backend:{status:id=>{reads.push(id);return read(id);},select:(id,provider)=>{writes.push([id,provider]);return save(id,provider);}},changed:()=>events.push('changed'),loading:()=>events.push('loading'),accepted:value=>accepted.push(value.provider_id),message:(...args)=>messages.push(args)});
 return {session,reads,writes,accepted,messages,events,context:value=>context=value,read:value=>read=value,save:value=>save=value,denyContext:()=>dead=true};
}

test('backend preserves existing command names and nullable/global versus task arguments',async()=>{
 const calls=[],backend=createTaskProviderBackend(async(...args)=>calls.push(args));
 await backend.status(null);await backend.status('a');await backend.select('a','gemini');
 assert.deepEqual(calls,[['translation_status',{jobId:null}],['translation_status',{jobId:'a'}],['set_task_translation',{jobId:'a',providerId:'gemini'}]]);
});

test('newer reads own status and errors on the same task visit',async()=>{
 const h=harness(),old=deferred(),fresh=deferred();let n=0;h.read(()=>++n===1?old.promise:fresh.promise);
 const one=h.session.refresh(),two=h.session.refresh();fresh.resolve(status('gemini'));await two;old.reject(Error('old failure'));await one;
 assert.deepEqual(h.accepted,['gemini']);assert.deepEqual(h.messages,[]);assert.equal(h.session.state().selected,'gemini');
});

test('status from another task/navigation cannot replace the current provider',async()=>{
 const h=harness(),old=deferred();h.read(id=>id==='a'?old.promise:Promise.resolve(status('deepl')));
 const one=h.session.refresh();h.context({jobId:'b',navigation:2});await h.session.refresh();old.resolve(status('gemini'));await one;
 assert.deepEqual(h.accepted,['deepl']);assert.equal(h.session.state().selected,'deepl');
});

test('explicit choice is allowed while loading, duplicates are blocked, and newer input survives save',async()=>{
 const h=harness(),save=deferred();h.save(()=>save.promise);h.read(async()=>status('gemini'));
 assert.equal(h.session.state().canApply,false);h.session.choose('gemini');const one=h.session.apply();await h.session.apply();
 h.session.choose('deepl');assert.equal(h.session.state().busy,true);assert.equal(h.session.state().selected,'deepl');
 save.resolve();await one;assert.deepEqual(h.writes,[['a','gemini']]);assert.deepEqual(h.accepted,['gemini']);assert.equal(h.session.state().selected,'deepl');assert.equal(h.session.state().canApply,true);
});

test('a task mutation cannot refresh or announce success in a different task',async()=>{
 const h=harness(),save=deferred();h.save(()=>save.promise);h.session.choose('gemini');const one=h.session.apply();
 h.context({jobId:'b',navigation:2});h.read(async()=>status('deepl'));await h.session.refresh();save.resolve();await one;
 assert.deepEqual(h.writes,[['a','gemini']]);assert.deepEqual(h.reads,['b']);assert.deepEqual(h.accepted,['deepl']);assert.deepEqual(h.messages,[]);assert.equal(h.session.state().busy,false);
});

test('returning to a task waits for its pending write then reads status for the new visit',async()=>{
 const h=harness(),save=deferred();h.save(()=>save.promise);h.read(async id=>status(id==='a'?'gemini':'deepl'));
 h.session.choose('gemini');const one=h.session.apply();h.context({jobId:'b',navigation:2});await h.session.refresh();
 h.context({jobId:'a',navigation:3});assert.equal(await h.session.refresh(),false);assert.equal(h.session.state().busy,true);
 save.resolve();await one;assert.deepEqual(h.reads,['b','a']);assert.deepEqual(h.accepted,['deepl','gemini']);assert.deepEqual(h.messages,[]);assert.equal(h.session.state().selected,'gemini');assert.equal(h.session.state().canApply,true);
});

test('failed writes retain choice and release busy without implying a successful commit',async()=>{
 const h=harness();h.save(async()=>{throw Error('write failed');});h.session.choose('openai-compatible');await h.session.apply();
 assert.equal(h.session.state().selected,'openai-compatible');assert.equal(h.session.state().busy,false);assert.equal(h.session.state().canApply,true);assert.match(h.messages[0][0],/设置任务后端失败/);assert.deepEqual(h.reads,[]);
});

test('a saved provider with failed status read is reported accurately and retains intent',async()=>{
 const h=harness();h.read(async()=>{throw Error('status unavailable');});h.session.choose('gemini');await h.session.apply();
 assert.deepEqual(h.writes,[['a','gemini']]);assert.match(h.messages[0][0],/已保存，但状态读取失败/);assert.equal(h.session.state().selected,'gemini');assert.equal(h.session.state().canApply,true);
});

test('closing tools suppresses their old message while current backend status remains truthful',async()=>{
 const h=harness(),save=deferred();h.save(()=>save.promise);h.read(async()=>status('gemini'));h.session.choose('gemini');const one=h.session.apply();
 h.session.closed();save.resolve();await one;assert.deepEqual(h.accepted,['gemini']);assert.deepEqual(h.messages,[]);
});

test('global status and invalid choices never become an implicit task mutation',async()=>{
 const h=harness();h.context({jobId:null,navigation:1});await h.session.refresh();h.session.choose('gemini');await h.session.apply();assert.deepEqual(h.reads,[null]);assert.deepEqual(h.writes,[]);
 h.context({jobId:'a',navigation:2});h.session.choose('unknown');assert.equal(h.session.state().canApply,false);h.session.choose('none');await h.session.apply();assert.deepEqual(h.writes,[['a','none']]);
});

test('dispose detaches state/context callbacks from both late reads and writes',async()=>{
 for(const operation of ['refresh','apply']){
  const h=harness(),pending=deferred();h.read(()=>pending.promise);h.save(()=>pending.promise);h.session.choose('gemini');
  const one=h.session[operation]();h.session.dispose();h.denyContext();const count=h.events.length;
  pending.resolve(status('gemini'));await one;await h.session.refresh();await h.session.apply();h.session.choose('none');
  assert.deepEqual(h.accepted,[]);assert.deepEqual(h.messages,[]);assert.equal(h.events.length,count);assert.equal(h.session.state().canApply,false);
 }
});
