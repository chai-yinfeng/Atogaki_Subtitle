import test from 'node:test';
import assert from 'node:assert/strict';
import { createTranslationProviderActions } from '../src/translation-provider-actions.ts';
import { createTranslationProviderBackend } from '../src/translation-provider-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const check=id=>({providerId:id,providerName:id,storedInSystem:true,availableFromEnvironment:false,credentialStore:'fixture'});
function harness(){
 let visit=1,open=true,dead=false,write=async()=>{},read=async()=>[],test=async()=>{},inspect=async id=>check(id),asr=async()=>({configured:true});
 const calls=[],messages=[],profiles=[],acks=[],captured=[];
 const session=createTranslationProviderActions({backend:{credential:(...args)=>{calls.push(['credential',...args]);return write(...args);},check:id=>{calls.push(['check',id]);return inspect(id);},test:id=>{calls.push(['test',id]);return test(id);},profiles:()=>{calls.push(['profiles']);return read();},asrStatus:()=>{calls.push(['asr']);return asr();}},visit:()=>{if(dead)throw Error('disposed visit');return visit;},isOpen:()=>open,invalidateSettingsLoads:()=>{},captureCredential:(id,clear)=>{captured.push([id,clear]);return {apiKey:null,applied:()=>acks.push('applied'),asrStatus:()=>acks.push('asr')};},profilesAccepted:value=>profiles.push(value),credentialMessage:(...args)=>messages.push(args.join(' ')),message:value=>messages.push(value)});
 return {session,calls,messages,profiles,acks,captured,visit:n=>visit=n,open:x=>open=x,write:f=>write=f,read:f=>read=f,test:f=>test=f,inspect:f=>inspect=f,asr:f=>asr=f,deny:()=>dead=true};
}

test('provider action adapter preserves five existing IPC contracts',async()=>{
 const calls=[],b=createTranslationProviderBackend(async(...args)=>calls.push(args));await b.check('deepseek');await b.test('deepl');await b.credential('gemini',null,true);await b.profiles();await b.asrStatus();
 assert.deepEqual(calls,[['check_translation_api_key',{providerId:'deepseek'}],['test_translation_connection',{providerId:'deepl'}],['save_translation_credential',{providerId:'gemini',apiKey:null,clear:true}],['translation_profiles',{}],['asr_credential_status',{}]]);
});

test('provider checks and tests never capture a secret or implicitly mutate credentials',async()=>{
 const h=harness();await h.session.run('check','deepseek');await h.session.run('test','deepl');await h.session.run('test','none');await h.session.run('save','hy-mt2-local');
 assert.deepEqual(h.captured,[]);assert.deepEqual(h.calls.filter(c=>c[0]!=='profiles'),[['check','deepseek'],['test','deepl']]);assert.match(h.messages[0],/未调用翻译 API/);
});

test('provider session blocks duplicate direct calls and serializes credential writes across providers',async()=>{
 const h=harness(),pending=deferred();h.write(()=>pending.promise);const one=h.session.run('save','deepseek');await h.session.run('delete','gemini');await h.session.run('save','deepseek');assert.equal(h.captured.length,1);
 pending.resolve();await one;assert.deepEqual(h.acks,['applied']);assert.equal(h.calls.filter(c=>c[0]==='credential').length,1);
 await h.session.run('delete','gemini');assert.deepEqual(h.captured,[['deepseek',false],['gemini',true]]);assert.deepEqual(h.acks,['applied','applied','asr']);
});

test('provider failed persistence does not acknowledge input, and committed refresh failure says saved',async()=>{
 const h=harness();h.write(async()=>{throw Error('write failed');});await h.session.run('save','deepseek');assert.deepEqual(h.acks,[]);assert.match(h.messages[0],/write failed/);
 h.write(async()=>{});h.read(async()=>{throw Error('refresh failed');});await h.session.run('save','deepseek');assert.deepEqual(h.acks,['applied']);assert.match(h.messages.at(-1),/凭据已保存，但状态刷新失败/);
});

test('late credential completion reloads a reopened settings visit without old messages or input clearing',async()=>{
 const h=harness(),pending=deferred();h.write(()=>pending.promise);const one=h.session.run('save','gemini');h.visit(3);pending.resolve();await one;
 assert.deepEqual(h.acks,[]);assert.deepEqual(h.messages,[]);assert.deepEqual(h.calls,[['credential','gemini',null,false],['profiles']]);assert.equal(h.profiles.length,1);
});

test('closed settings suppress late credential success and all follow-up reads',async()=>{
 const h=harness(),pending=deferred();h.write(()=>pending.promise);const one=h.session.run('delete','gemini');h.visit(2);h.open(false);pending.resolve();await one;
 assert.deepEqual(h.acks,[]);assert.deepEqual(h.messages,[]);assert.deepEqual(h.calls,[['credential','gemini',null,true]]);
});

test('only newest provider notice is presented even if older checks complete later',async()=>{
 const h=harness(),old=deferred();h.inspect(id=>id==='deepseek'?old.promise:Promise.resolve(check(id)));const one=h.session.run('check','deepseek');await h.session.run('check','deepl');old.resolve(check('deepseek'));await one;
 assert.equal(h.messages.length,1);assert.match(h.messages[0],/^deepl：/);
});

test('provider profile generations discard older success and failure after a newer accepted snapshot',async()=>{
 for(const fail of [true,false]){
  const h=harness(),old=deferred();h.read(()=>old.promise);const one=h.session.refreshProfiles();h.session.invalidateProfiles();h.read(async()=>[{provider_id:'new'}]);await h.session.refreshProfiles();
  if(fail)old.reject(Error('stale failure'));else old.resolve([{provider_id:'old'}]);await one;assert.deepEqual(h.profiles,[[{provider_id:'new'}]]);
 }
});

test('failed connection test refreshes readiness without changing its failure notice',async()=>{
 const h=harness();h.test(async()=>{throw Error('connection failed');});await h.session.run('test','deepseek');assert.match(h.messages[0],/^连接测试失败：/);assert.equal(h.profiles.length,1);assert.deepEqual(h.acks,[]);
});

test('provider dispose and expired view prevent late callbacks and released context access',async()=>{
 for(const expired of ['view','session']){
  const h=harness(),pending=deferred();let visible=true;h.write(()=>pending.promise);const one=h.session.run('save','gemini',()=>visible);
  if(expired==='session'){h.session.dispose();h.deny();}else visible=false;
  pending.resolve();await one;assert.deepEqual(h.acks,[]);assert.deepEqual(h.messages,[]);assert.equal(h.profiles.length,0);
  if(expired==='session'){await h.session.run('save','deepseek');await h.session.refreshProfiles();assert.equal(h.captured.length,1);}
 }
});


test('credential mutation invalidates pre-write profile reads',async()=>{
 const h=harness(),old=deferred(),write=deferred();h.read(()=>old.promise);const read=h.session.refreshProfiles();h.write(()=>write.promise);const saving=h.session.run('save','deepseek');old.resolve([{provider_id:'stale'}]);await read;assert.deepEqual(h.profiles,[]);
 h.read(async()=>[{provider_id:'fresh'}]);write.resolve();await saving;assert.deepEqual(h.profiles,[[{provider_id:'fresh'}]]);
});
