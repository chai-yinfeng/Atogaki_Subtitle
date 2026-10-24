import test from 'node:test';
import assert from 'node:assert/strict';
import { createModelPickerSession } from '../src/model-picker-session.ts';
import { createModelPickerBackend } from '../src/model-picker-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
function harness(){
 let visit=1,open=true,dead=false,eligible=true,capture=true,pick=async()=>'/fixture.bin';
 const calls=[],applied=[],messages=[],changes=[],cleared=[];
 const guard=()=>{if(dead)throw Error('disposed context read');};
 const session=createModelPickerSession({backend:{pick:kind=>{calls.push(kind);return pick(kind);}},visit:()=>{guard();return visit;},isOpen:()=>{guard();return open;},capture:()=>capture?{apply:path=>{if(!eligible)return false;applied.push(path);return true;}}:null,changed:()=>changes.push(session.busy),message:text=>messages.push(text),clearMessage:text=>cleared.push(text)});
 return {session,calls,applied,messages,changes,cleared,visit:n=>visit=n,open:x=>open=x,pick:f=>pick=f,eligible:x=>eligible=x,capture:x=>capture=x,deny:()=>dead=true};
}

test('model picker adapter preserves three parameterless native commands',async()=>{
 const calls=[],backend=createModelPickerBackend(async(...args)=>calls.push(args));await backend.pick('whisper');await backend.pick('vad');await backend.pick('hy-mt2');assert.deepEqual(calls,[['pick_model_file',{}],['pick_vad_model_file',{}],['pick_hy_mt2_model_file',{}]]);
});

test('successful chooser applies only its captured field and marks configuration unsaved',async()=>{
 const h=harness();await h.session.pick('whisper');assert.deepEqual(h.calls,['whisper']);assert.deepEqual(h.applied,['/fixture.bin']);assert.match(h.messages.at(-1),/尚未保存配置/);assert.deepEqual(h.changes,[true,false]);
});

test('one native chooser serializes duplicate and cross-kind requests',async()=>{
 const h=harness(),pending=deferred();h.pick(()=>pending.promise);const one=h.session.pick('whisper');await h.session.pick('whisper');await h.session.pick('vad');assert.deepEqual(h.calls,['whisper']);assert.equal(h.session.busy,true);pending.resolve('/chosen.bin');await one;await h.session.pick('vad');assert.deepEqual(h.calls,['whisper','vad']);assert.equal(h.session.busy,false);
});

test('cancel and current error preserve input and permit retry',async()=>{
 const h=harness();h.pick(async()=>null);await h.session.pick('whisper');assert.match(h.messages.at(-1),/已取消/);assert.deepEqual(h.applied,[]);h.pick(async()=>{throw Error('fixture dialog failure');});await h.session.pick('vad');assert.match(h.messages.at(-1),/无法打开模型选择器/);assert.equal(h.session.busy,false);h.pick(async()=>'/retry.gguf');await h.session.pick('hy-mt2');assert.deepEqual(h.applied,['/retry.gguf']);
});

test('a captured field can reject replacement after newer input without losing its draft',async()=>{
 const h=harness(),pending=deferred();h.pick(()=>pending.promise);const one=h.session.pick('whisper');h.eligible(false);pending.resolve('/old-result.bin');await one;assert.deepEqual(h.applied,[]);assert.match(h.messages.at(-1),/保留现有值/);
});

test('close and reopen suppress old chooser success or error',async()=>{
 for(const fail of [false,true]){
  const h=harness(),pending=deferred();h.pick(()=>pending.promise);const one=h.session.pick('vad');h.visit(3);h.session.hidden();assert.deepEqual(h.cleared,['正在打开模型文件选择器…']);if(fail)pending.reject(Error('old failure'));else pending.resolve('/old.bin');await one;assert.deepEqual(h.applied,[]);assert.deepEqual(h.messages,['正在打开模型文件选择器…']);assert.equal(h.session.busy,false);
 }
});

test('closed settings, missing field and invalid picker kind issue no native request',async()=>{
 const h=harness();h.open(false);await h.session.pick('whisper');h.open(true);h.capture(false);await h.session.pick('vad');h.capture(true);await h.session.pick('other');assert.deepEqual(h.calls,[]);assert.deepEqual(h.changes,[]);
});

test('picker dispose ignores late result and future requests without reading released context',async()=>{
 for(const fail of [false,true]){
  const h=harness(),pending=deferred();h.pick(()=>pending.promise);const one=h.session.pick('whisper');h.session.dispose();h.deny();const changes=h.changes.length;
  if(fail)pending.reject(Error('late failure'));else pending.resolve('/late.bin');await one;await h.session.pick('hy-mt2');assert.deepEqual(h.applied,[]);assert.equal(h.changes.length,changes);assert.equal(h.session.busy,false);
 }
});
