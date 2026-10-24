import test from 'node:test';
import assert from 'node:assert/strict';
import { createDesktopSettingsSave, createVersionedDirtyFields } from '../src/desktop-settings-save.ts';
import { createDesktopSettingsBackend } from '../src/desktop-settings-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const request=()=>({whisperModelPath:'/model',vadModelPath:null,hyMt2ModelPath:null,translationProviderId:'deepseek',translationModel:'saved-model',translationBaseUrl:null,translationStyleInstruction:null,apiKey:null,clearApiKey:false,networkProxyMode:'environment',networkProxyUrl:null,modelMirrorUrl:null,onboardingCompleted:false});
function harness(){
 let visit=1,open=true,dead=false,write=async()=>({needsOnboarding:false}),profiles=async()=>[],refresh=async()=>{},capture=true;
 const dirty=createVersionedDirtyFields(),drafts=createVersionedDirtyFields(),input=request(),messages=[],accepted=[],busy=[],writes=[],events=[];
 dirty.add('model');drafts.add('deepseek');
 const session=createDesktopSettingsSave({backend:{save:r=>{writes.push(r);return write(r);},profiles:()=>profiles()},visit:()=>{if(dead)throw Error('disposed visit access');return visit;},capture:()=>{
  if(!capture)return null;
  const fields=dirty.snapshot(),profile=drafts.snapshot(),id=input.translationProviderId,revision=dirty.revision,profileRevision=drafts.revision;
  return {request:{...input},acknowledge:()=>{dirty.acknowledge(fields);drafts.acknowledge(profile,id);},hasNewInput:()=>dirty.revision!==revision||drafts.revision!==profileRevision};
 },invalidateLoads:()=>events.push('invalidate'),accepted:(...args)=>accepted.push(args),refreshTranslation:()=>refresh(),reload:async()=>events.push('reload'),isOpen:()=>open,close:()=>{open=false;visit++;events.push('close');},changed:x=>busy.push(x),message:x=>messages.push(x)});
 return {session,dirty,drafts,input,messages,accepted,busy,writes,events,write:f=>write=f,profiles:f=>profiles=f,refresh:f=>refresh=f,visit:n=>visit=n,open:x=>open=x,valid:x=>capture=x,deny:()=>dead=true};
}

test('versioned dirty acknowledgements preserve edits, same-value re-entry and other providers',()=>{
 const fields=createVersionedDirtyFields();fields.add('a');fields.add('b');const saved=fields.snapshot();fields.add('a');fields.acknowledge(saved);
 assert.equal(fields.has('a'),true);assert.equal(fields.has('b'),false);
 fields.add('b');const profiles=fields.snapshot();fields.acknowledge(profiles,'a');assert.equal(fields.has('b'),true);
 const before=fields.snapshot();fields.clear();fields.add('b');fields.acknowledge(before);assert.equal(fields.has('b'),true);
});

test('settings backend preserves save envelope and profile command',async()=>{
 const calls=[],backend=createDesktopSettingsBackend(async(...args)=>{calls.push(args);return [];});const value=request();
 await backend.save(value);await backend.profiles();assert.deepEqual(calls,[['save_desktop_settings',{request:value}],['translation_profiles',{}]]);
});

test('settings save is frozen and duplicate submits do not send another write',async()=>{
 const h=harness(),pending=deferred();h.write(()=>pending.promise);const one=h.session.save(false);await h.session.save(true);
 h.input.translationProviderId='gemini';h.input.translationModel='new-model';h.dirty.add('provider');h.drafts.add('gemini');
 pending.resolve({needsOnboarding:false});await one;
 assert.equal(h.writes.length,1);assert.equal(h.writes[0].translationProviderId,'deepseek');assert.equal(h.writes[0].translationModel,'saved-model');
 assert.equal(h.dirty.has('provider'),true);assert.equal(h.drafts.has('gemini'),true);assert.equal(h.drafts.has('deepseek'),false);assert.match(h.messages.at(-1),/新输入尚未保存/);assert.deepEqual(h.busy,[true,false]);
});

test('same provider edits made during settings save stay dirty',async()=>{
 const h=harness(),pending=deferred();h.write(()=>pending.promise);const one=h.session.save(false);h.dirty.add('model');h.drafts.add('deepseek');pending.resolve({needsOnboarding:false});await one;
 assert.equal(h.dirty.has('model'),true);assert.equal(h.drafts.has('deepseek'),true);
});

test('failed settings write retains intent and does not present a successful save',async()=>{
 const h=harness();h.write(async()=>{throw Error('test rejected');});await h.session.save(true);
 assert.equal(h.accepted.length,0);assert.equal(h.dirty.has('model'),true);assert.match(h.messages.at(-1),/^保存失败/);assert.equal(h.events.includes('close'),false);assert.deepEqual(h.busy,[true,false]);
});

test('settings profile or translation refresh failure is distinct from failed persistence',async()=>{
 for(const method of ['profiles','refresh']){
  const h=harness();h[method](async()=>{throw Error('refresh unavailable');});await h.session.save(true);
  assert.equal(h.accepted.length,1);assert.equal(h.dirty.size,0);assert.match(h.messages.at(-1),/已保存，但状态刷新未完成/);assert.equal(h.events.includes('close'),false);
 }
});

test('onboarding closes only without new input, including during status refresh',async()=>{
 const h=harness(),pending=deferred();h.refresh(()=>pending.promise);const one=h.session.save(true);
 while(!h.accepted.length)await new Promise(resolve=>setImmediate(resolve));
 h.dirty.add('model');pending.resolve();await one;assert.equal(h.events.includes('close'),false);assert.match(h.messages.at(-1),/新输入尚未保存/);
 const clean=harness();await clean.session.save(true);assert.equal(clean.events.includes('close'),true);
});

test('settings close and reopen ignores old presentation then reloads the new visit',async()=>{
 const h=harness(),pending=deferred();h.write(()=>pending.promise);const one=h.session.save(true);h.visit(3);h.dirty.clear();h.dirty.add('new-visit');pending.resolve({needsOnboarding:false});await one;
 assert.equal(h.accepted.length,0);assert.equal(h.dirty.has('new-visit'),true);assert.deepEqual(h.messages,['正在保存配置…']);assert.equal(h.events.includes('reload'),true);assert.equal(h.events.includes('close'),false);
});

test('late profile result and failed save cannot message a closed settings visit',async()=>{
 for(const phase of ['write','profiles']){
  const h=harness(),pending=deferred();h[phase](()=>pending.promise);const one=h.session.save(false);
  if(phase==='profiles')await new Promise(resolve=>setImmediate(resolve));
  h.visit(2);h.open(false);pending.reject(Error('late failure'));await one;
  assert.equal(h.accepted.length,0);assert.deepEqual(h.messages,['正在保存配置…']);assert.equal(h.events.includes('reload'),false);
 }
});

test('invalid settings capture sends nothing, dispose ignores late callbacks and new saves',async()=>{
 const invalid=harness();invalid.valid(false);await invalid.session.save(true);assert.deepEqual(invalid.writes,[]);assert.deepEqual(invalid.busy,[]);
 const h=harness(),pending=deferred();h.write(()=>pending.promise);const one=h.session.save(false);h.session.dispose();h.deny();const count=h.events.length;
 pending.resolve({needsOnboarding:false});await one;await h.session.save(false);assert.equal(h.accepted.length,0);assert.equal(h.events.length,count);assert.equal(h.writes.length,1);
});
