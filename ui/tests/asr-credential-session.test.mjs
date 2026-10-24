import test from 'node:test';
import assert from 'node:assert/strict';
import {createSettingsMutationGate} from '../src/settings-mutation-gate.ts';
import {createAsrCredentialSession} from '../src/asr-credential-session.ts';
import {createAsrCredentialBackend} from '../src/asr-credential-backend.ts';
import {createDesktopSettingsSave} from '../src/desktop-settings-save.ts';
import {createTranslationProviderActions} from '../src/translation-provider-actions.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return{promise,resolve,reject};};
const status={providerId:'gemini-transcribe',providerName:'Gemini',configured:true,credentialStore:'fixture'};
function harness(){
 let visit=1,open=true,dead=false,secret=' mock-key ',exists=true,work=async()=>status,refresh=async()=>{};
 const calls=[],accepted=[],messages=[],cleared=[],acks=[],changes=[],invalidations=[];const mutations=createSettingsMutationGate();
 const guard=()=>{if(dead)throw Error('released context');};
 const session=createAsrCredentialSession({backend:{save:(key,clear)=>{calls.push(['save',key,clear]);return work();},check:()=>{calls.push(['check']);return work();}},mutations,visit:()=>{guard();return visit;},isOpen:()=>{guard();return open;},capture:clear=>{calls.push(['capture']);return exists?{apiKey:clear?null:secret,acknowledge:()=>acks.push(true)}:null;},accepted:s=>accepted.push(s),invalidate:()=>invalidations.push(true),refresh:()=>{calls.push(['refresh']);return refresh();},changed:()=>changes.push(session.busy),message:(text,warning)=>messages.push([text,warning]),clearMessage:text=>cleared.push(text),checkHint:'fixture store'});
 return{session,mutations,calls,accepted,messages,cleared,acks,changes,invalidations,work:f=>work=f,refresh:f=>refresh=f,visit:n=>visit=n,open:b=>open=b,secret:s=>secret=s,exists:b=>exists=b,deny:()=>dead=true};
}
test('credential write gate releases idempotently and versions every ownership change',()=>{
 const gate=createSettingsMutationGate();assert.equal(gate.revision,0);const release=gate.acquire();assert.equal(gate.busy,true);assert.equal(gate.acquire(),null);release();release();assert.equal(gate.busy,false);assert.equal(gate.revision,2);const other=gate.acquire();assert.equal(gate.revision,3);other();
});
test('ASR adapter preserves two existing commands and exact request shape',async()=>{
 const calls=[],b=createAsrCredentialBackend(async(...args)=>{calls.push(args);return status;});await b.save('mock',false);await b.save(null,true);await b.check();assert.deepEqual(calls,[['save_asr_credential',{request:{apiKey:'mock',clear:false}}],['save_asr_credential',{request:{apiKey:null,clear:true}}],['check_asr_credential',{}]]);
});
test('ASR check neither captures nor acknowledges secret input',async()=>{
 const h=harness();await h.session.run('check');assert.deepEqual(h.calls,[['check']]);assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[status]);assert.equal(h.mutations.revision,0);
});
test('ASR save and delete invalidate reads, acknowledge captured inputs and refresh profiles',async()=>{
 const h=harness();await h.session.run('save');assert.deepEqual(h.calls,[['capture'],['save','mock-key',false],['refresh']]);assert.equal(h.invalidations.length,2);assert.equal(h.acks.length,1);assert.equal(h.mutations.busy,false);h.calls.length=0;await h.session.run('delete');assert.deepEqual(h.calls,[['capture'],['save',null,true],['refresh']]);
});
test('pending ASR action serializes duplicate and alternate actions',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('save');await h.session.run('delete');await h.session.run('check');assert.equal(h.calls.length,2);pending.resolve(status);await run;assert.equal(h.session.busy,false);
});
test('all three credential-capable write entry points share ownership before capturing secrets',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('save');const messages=[];
 const save=createDesktopSettingsSave({mutations:h.mutations,message:t=>messages.push(t),capture:()=>{throw Error('must not capture while busy');}});
 const provider=createTranslationProviderActions({mutations:h.mutations,message:t=>messages.push(t),captureCredential:()=>{throw Error('must not capture while busy');}});
 await save.save(false);await provider.run('save','gemini');await provider.run('check','gemini');assert.equal(messages.length,3);assert.ok(messages.every(t=>t.includes('另一个配置或凭据操作')));pending.resolve(status);await run;assert.equal(h.mutations.busy,false);
 const release=h.mutations.acquire();await h.session.run('save');await h.session.run('check');assert.equal(h.calls.filter(c=>c[0]==='capture').length,1);release();
});
test('empty or unavailable input releases gate and never invokes credential persistence',async()=>{
 const h=harness();h.secret('  ');await h.session.run('save');assert.equal(h.calls.some(c=>c[0]==='save'),false);assert.match(h.messages.at(-1)[0],/请输入/);assert.equal(h.mutations.busy,false);h.exists(false);await h.session.run('delete');assert.equal(h.mutations.busy,false);h.open(false);await h.session.run('check');assert.equal(h.calls.some(c=>c[0]==='check'),false);
});
test('failed write preserves input, releases gate and can retry; committed refresh failure is precise',async()=>{
 const h=harness();h.work(async()=>{throw Error('fixture write failure');});await h.session.run('save');assert.deepEqual(h.acks,[]);assert.equal(h.mutations.busy,false);h.work(async()=>status);h.refresh(async()=>{throw Error('fixture refresh failure');});await h.session.run('save');assert.equal(h.acks.length,1);assert.match(h.messages.at(-1)[0],/Key 已保存，但状态刷新失败/);
});
test('late success or error after close/reopen never acknowledges new inputs or replays old notices',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('save');h.visit(3);h.session.hidden();const messages=h.messages.length;if(fail)pending.reject(Error('old failure'));else pending.resolve(status);await run;assert.equal(h.messages.length,messages);assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[]);assert.equal(h.calls.filter(c=>c[0]==='refresh').length,fail?0:1);assert.equal(h.mutations.busy,false);}
});
test('closed settings do not refresh after committed write',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('delete');h.visit(2);h.open(false);pending.resolve(status);await run;assert.equal(h.calls.some(c=>c[0]==='refresh'),false);assert.deepEqual(h.acks,[]);
});
test('a credential check superseded by another write drops old status and clears only its owned notice',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('check');const release=h.mutations.acquire();release();pending.resolve(status);await run;assert.deepEqual(h.accepted,[]);assert.match(h.cleared[0],/正在检查系统凭据/);
});
test('dispose ignores late success/error without reading released context and eventually releases ownership',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('save');h.session.dispose();h.deny();const changes=h.changes.length;if(fail)pending.reject(Error('late'));else pending.resolve(status);await run;await h.session.run('check');h.session.hidden();assert.equal(h.changes.length,changes);assert.deepEqual(h.acks,[]);assert.equal(h.mutations.busy,false);}
});
test('global save and provider writes own the same gate until completion, then permit ASR retry',async()=>{
 for(const owner of ['global','provider']){
  const h=harness(),pending=deferred(),messages=[];
  const common={mutations:h.mutations,visit:()=>1,isOpen:()=>true,message:t=>messages.push(t)};
  const session=owner==='global'?createDesktopSettingsSave({...common,backend:{save:()=>pending.promise,profiles:async()=>[]},capture:()=>({request:{apiKey:null},acknowledge(){},hasNewInput:()=>false}),invalidateLoads(){},accepted(){},refreshTranslation:async()=>{},reload:async()=>{},close(){},changed(){}}):createTranslationProviderActions({...common,backend:{credential:()=>pending.promise,profiles:async()=>[],asrStatus:async()=>status},captureCredential:()=>({apiKey:'mock',applied(){},asrStatus(){}}),invalidateSettingsLoads(){},profilesAccepted(){},credentialMessage(){}});
  const run=owner==='global'?session.save(false):session.run('save','gemini');assert.equal(h.mutations.busy,true);await h.session.run('save');assert.equal(h.calls.length,0);pending.resolve({needsOnboarding:false});await run;assert.equal(h.mutations.busy,false);await h.session.run('save');assert.ok(h.calls.some(c=>c[0]==='save'));
 }
});
test('a provider credential check started before ASR ownership cannot publish stale credential notices',async()=>{
 const gate=createSettingsMutationGate(),pending=deferred(),messages=[],accepted=[];
 const session=createTranslationProviderActions({mutations:gate,visit:()=>1,isOpen:()=>true,backend:{check:()=>pending.promise,profiles:async()=>[]},message:t=>messages.push(t),profilesAccepted:p=>accepted.push(p)});
 const run=session.run('check','gemini');const release=gate.acquire();release();pending.resolve({providerName:'Gemini',storedInSystem:true});await run;assert.deepEqual(messages,[]);assert.deepEqual(accepted,[]);
});
test('provider disposal releases shared ownership after persistence without invalidating released settings',async()=>{
 const gate=createSettingsMutationGate(),pending=deferred();let dead=false;
 const guard=()=>{if(dead)throw Error('released settings');};
 const session=createTranslationProviderActions({mutations:gate,visit:()=>{guard();return 1;},isOpen:()=>{guard();return true;},backend:{credential:()=>pending.promise},captureCredential:()=>({apiKey:'mock'}),invalidateSettingsLoads:guard});
 const run=session.run('save','gemini');session.dispose();dead=true;pending.resolve();await run;assert.equal(gate.busy,false);
});
