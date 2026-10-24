import test from 'node:test';
import assert from 'node:assert/strict';
import {createSettingsMutationGate} from '../src/settings-mutation-gate.ts';
import {createDictionaryCredentialSession} from '../src/dictionary-credential-session.ts';
import {createDictionaryCredentialBackend} from '../src/dictionary-credential-backend.ts';
const provider='merriam-webster',status={providerId:provider,providerName:'Merriam-Webster',configured:true,credentialStore:'fixture'};
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return{promise,resolve,reject};};
function harness(){
 let visit=1,open=true,dead=false,exists=true,secret=' mock ',work=async()=>status;
 const calls=[],accepted=[],acks=[],messages=[],cleared=[],invalidated=[],changes=[],mutations=createSettingsMutationGate();
 const guard=()=>{if(dead)throw Error('released context');};
 const session=createDictionaryCredentialSession({backend:{save:(...args)=>{calls.push(['save',...args]);return work();},check:id=>{calls.push(['check',id]);return work();}},mutations,visit:()=>{guard();return visit;},isOpen:()=>{guard();return open;},capture:(id,clear)=>{calls.push(['capture',id]);return exists?{apiKey:clear?null:secret,acknowledge:()=>acks.push(id)}:null;},accepted:s=>accepted.push(s),invalidate:()=>invalidated.push(true),reload:async()=>calls.push(['reload']),changed:()=>changes.push(session.busy),message:t=>messages.push(t),clearMessage:t=>cleared.push(t),checkHint:'fixture store'});
 return{session,mutations,calls,accepted,acks,messages,cleared,invalidated,changes,visit:n=>visit=n,open:b=>open=b,work:f=>work=f,secret:s=>secret=s,exists:b=>exists=b,deny:()=>dead=true};
}
test('dictionary credential adapter preserves original provider and request shape',async()=>{
 const calls=[],backend=createDictionaryCredentialBackend(async(...args)=>{calls.push(args);return status;});await backend.save(provider,'mock',false);await backend.save(provider,null,true);await backend.check(provider);assert.deepEqual(calls,[['save_dictionary_credential',{request:{providerId:provider,apiKey:'mock',clear:false}}],['save_dictionary_credential',{request:{providerId:provider,apiKey:null,clear:true}}],['check_dictionary_credential',{providerId:provider}]]);
});
test('dictionary check never captures or clears input, and supersedes older snapshot reads',async()=>{
 const h=harness();await h.session.run('check',provider);assert.deepEqual(h.calls,[['check',provider]]);assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[status]);assert.equal(h.invalidated.length,2);
});
test('dictionary save/delete acknowledge only captured input and update existing status',async()=>{
 const h=harness();await h.session.run('save',provider);assert.deepEqual(h.calls,[['capture',provider],['save',provider,'mock',false]]);assert.deepEqual(h.acks,[provider]);assert.deepEqual(h.accepted,[status]);assert.match(h.messages.at(-1),/保存至 fixture/);await h.session.run('delete',provider);assert.deepEqual(h.calls.at(-1),['save',provider,null,true]);assert.match(h.messages.at(-1),/从 fixture 删除/);assert.equal(h.mutations.busy,false);
});
test('duplicate/alternate operations are serialized; shared ownership blocks secret capture',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('save',provider);await h.session.run('check',provider);await h.session.run('delete',provider);assert.equal(h.calls.length,2);assert.equal(h.mutations.acquire(),null);pending.resolve(status);await run;const release=h.mutations.acquire();await h.session.run('save',provider);await h.session.run('check',provider);assert.equal(h.calls.length,2);assert.match(h.messages.at(-1),/另一个配置或凭据/);release();
});
test('missing/empty input, unsupported provider/action and closed settings never persist',async()=>{
 const h=harness();await h.session.run('save','unsupported');await h.session.run('unknown',provider);h.open(false);await h.session.run('check',provider);h.open(true);h.exists(false);await h.session.run('delete',provider);h.exists(true);h.secret(' ');await h.session.run('save',provider);assert.equal(h.calls.some(c=>c[0]==='save'||c[0]==='check'),false);assert.equal(h.mutations.busy,false);assert.match(h.messages.at(-1),/先输入/);
});
test('failure preserves input and releases ownership for retry; mismatched status never updates another source',async()=>{
 const h=harness();h.work(async()=>{throw Error('fixture failure');});await h.session.run('save',provider);assert.deepEqual(h.acks,[]);assert.match(h.messages.at(-1),/操作失败/);assert.equal(h.mutations.busy,false);h.work(async()=>({...status,providerId:'wrong'}));await h.session.run('save',provider);assert.deepEqual(h.accepted,[]);assert.deepEqual(h.acks,[]);assert.match(h.messages.at(-1),/已保存，但状态更新失败/);h.work(async()=>status);await h.session.run('save',provider);assert.deepEqual(h.acks,[provider]);
});
test('close/reopen drops old success/error and only a committed write reloads fresh state',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('save',provider);h.visit(3);h.session.hidden();const messages=h.messages.length;if(fail)pending.reject(Error('old'));else pending.resolve(status);await run;assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[]);assert.equal(h.messages.length,messages);assert.equal(h.calls.some(c=>c[0]==='reload'),!fail);assert.equal(h.mutations.busy,false);}
});
test('old check neither publishes nor invalidates fresh reopened dictionary reads',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('check',provider);h.visit(3);h.session.hidden();pending.resolve(status);await run;assert.deepEqual(h.accepted,[]);assert.equal(h.invalidated.length,1);assert.equal(h.calls.some(c=>c[0]==='reload'),false);
});
test('check superseded by credential ownership change cannot overwrite current status',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('check',provider);const release=h.mutations.acquire();release();pending.resolve(status);await run;assert.deepEqual(h.accepted,[]);assert.equal(h.invalidated.length,1);assert.match(h.cleared[0],/正在检查/);
});
test('closed/disposed writes suppress late callbacks and release shared ownership',async()=>{
 for(const mode of ['closed','disposed-success','disposed-error']){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run('delete',provider);const changes=h.changes.length;if(mode==='closed'){h.visit(2);h.open(false);}else{h.session.dispose();h.deny();}if(mode.endsWith('error'))pending.reject(Error('late'));else pending.resolve(status);await run;assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[]);assert.equal(h.calls.some(c=>c[0]==='reload'),false);assert.equal(h.mutations.busy,false);if(mode!=='closed'){assert.equal(h.changes.length,changes);await h.session.run('save',provider);h.session.hidden();assert.equal(h.invalidated.length,1);}}
});
