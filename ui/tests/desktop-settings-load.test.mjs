import test from 'node:test';
import assert from 'node:assert/strict';
import { createDesktopSettingsLoad } from '../src/desktop-settings-load.ts';
import { createDesktopSettingsBackend } from '../src/desktop-settings-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const dictionary=id=>({catalog:[],downloads:[],credentials:[],directory:id});
function harness(){
 let visit=0,dead=false,catalog=[],read=async()=>({needsOnboarding:false}),dict=async()=>dictionary('current');
 const accepted=[],dictionaries=[],messages=[],dictionaryMessages=[],calls=[];
 const loader=createDesktopSettingsLoad({backend:{settings:()=>{calls.push('settings');return read();},models:async()=>{calls.push('models');return [{id:'model'}];},downloads:async()=>[],profiles:async()=>[],dictionaries:()=>{calls.push('dictionary');return dict();}},visit:()=>{if(dead)throw Error('disposed visit read');return visit;},catalog:()=>{if(dead)throw Error('disposed catalog read');return catalog;},accepted:snapshot=>{accepted.push(snapshot);catalog=snapshot.catalog;},dictionariesAccepted:value=>dictionaries.push(value),onboard:()=>calls.push('onboard'),message:value=>messages.push(value),dictionaryMessage:value=>dictionaryMessages.push(value)});
 return {loader,accepted,dictionaries,messages,dictionaryMessages,calls,read:f=>read=f,dict:f=>dict=f,visit:n=>visit=n,deny:()=>dead=true};
}

test('settings reads use original commands; dictionary snapshot waits for all four results',async()=>{
 const calls=[],directory=deferred(),values={desktop_settings:{needsOnboarding:false},model_catalog:[],model_download_states:[],translation_profiles:[],dictionary_catalog:[],dictionary_download_states:[],dictionary_credential_statuses:[]};
 const backend=createDesktopSettingsBackend(async(cmd,args)=>{calls.push([cmd,args]);return cmd==='dictionary_directory'?directory.promise:values[cmd];});
 await backend.settings();await backend.models();await backend.downloads();await backend.profiles();const result=backend.dictionaries();directory.resolve('/dict');
 assert.deepEqual(await result,dictionary('/dict'));assert.deepEqual(calls.map(c=>c[0]),Object.keys(values).concat('dictionary_directory'));assert.equal(calls.every(c=>JSON.stringify(c[1])==='{}'),true);
});

test('accepted settings reuse the existing catalog without another catalog request',async()=>{
 const h=harness();assert.equal(await h.loader.load(),true);assert.equal(await h.loader.load(),true);
 assert.equal(h.calls.filter(c=>c==='models').length,1);assert.equal(h.accepted.length,2);assert.equal(h.dictionaries.length,2);
});

test('new settings request owns snapshots, errors and onboarding',async()=>{
 const h=harness(),old=deferred();let calls=0;h.read(()=>++calls===1?old.promise:Promise.resolve({needsOnboarding:false}));
 const first=h.loader.load(true);await h.loader.load();old.reject(Error('stale startup failure'));assert.equal(await first,false);
 assert.equal(h.accepted.length,1);assert.deepEqual(h.messages,[]);assert.equal(h.calls.includes('onboard'),false);
});

test('closing a settings visit ignores a late core snapshot and never opens onboarding',async()=>{
 const h=harness(),pending=deferred();h.read(()=>pending.promise);const first=h.loader.load(true);h.visit(1);h.loader.invalidate();pending.resolve({needsOnboarding:true});
 assert.equal(await first,false);assert.equal(h.accepted.length,0);assert.equal(h.dictionaries.length,0);assert.equal(h.calls.includes('onboard'),false);
});

test('closing during dictionary load suppresses its late status or error',async()=>{
 for(const failure of [false,true]){
  const h=harness(),pending=deferred();h.read(async()=>({needsOnboarding:true}));h.dict(()=>pending.promise);const first=h.loader.load(true);
  await new Promise(resolve=>setImmediate(resolve));assert.equal(h.accepted.length,1);h.visit(1);h.loader.invalidate();
  if(failure)pending.reject(Error('late dictionary failure'));else pending.resolve(dictionary('old'));
  assert.equal(await first,false);assert.equal(h.dictionaries.length,0);assert.deepEqual(h.dictionaryMessages,[]);assert.equal(h.calls.includes('onboard'),false);
 }
});

test('standalone dictionary refresh and settings reload share one result generation',async()=>{
 const h=harness(),old=deferred();let calls=0;h.dict(()=>++calls===1?old.promise:Promise.resolve(dictionary('new')));
 const first=h.loader.load();await new Promise(resolve=>setImmediate(resolve));await h.loader.loadDictionaries();old.resolve(dictionary('old'));await first;
 assert.deepEqual(h.dictionaries.map(d=>d.directory),['new']);
});

test('save invalidation prevents late settings and nested dictionaries from replacing new values',async()=>{
 const h=harness(),old=deferred();h.read(()=>old.promise);const first=h.loader.load();h.loader.invalidate();old.resolve({needsOnboarding:false});await first;assert.equal(h.accepted.length,0);
 const pending=deferred();h.read(async()=>({needsOnboarding:false}));h.dict(()=>pending.promise);const second=h.loader.load();await new Promise(resolve=>setImmediate(resolve));h.loader.invalidate();pending.resolve(dictionary('old'));await second;assert.equal(h.dictionaries.length,0);
});

test('current dictionary failure preserves core settings and permits onboarding and retry',async()=>{
 const h=harness();h.read(async()=>({needsOnboarding:true}));h.dict(async()=>{throw Error('dictionary unavailable');});assert.equal(await h.loader.load(true),true);
 assert.equal(h.accepted.length,1);assert.match(h.dictionaryMessages[0],/无法读取词典配置/);assert.equal(h.calls.includes('onboard'),true);assert.deepEqual(h.messages,[]);
 h.dict(async()=>dictionary('recovered'));assert.equal(await h.loader.loadDictionaries(),true);assert.equal(h.dictionaries[0].directory,'recovered');
});

test('current settings failure reports once and can be retried',async()=>{
 const h=harness();h.read(async()=>{throw Error('settings unavailable');});assert.equal(await h.loader.load(),false);assert.match(h.messages[0],/无法读取启动配置/);assert.equal(h.accepted.length,0);
 h.read(async()=>({needsOnboarding:false}));assert.equal(await h.loader.load(),true);assert.equal(h.accepted.length,1);
});

test('dispose ignores late results and future loads without accessing released context',async()=>{
 for(const phase of ['core','dictionary']){
  const h=harness(),pending=deferred();if(phase==='core')h.read(()=>pending.promise);else h.dict(()=>pending.promise);
  const first=h.loader.load(true);if(phase==='dictionary')await new Promise(resolve=>setImmediate(resolve));
  h.loader.dispose();h.deny();const count=h.accepted.length;pending.reject(Error('late disposed failure'));await first;
  assert.equal(await h.loader.load(),false);assert.equal(await h.loader.loadDictionaries(),false);assert.equal(h.accepted.length,count);assert.equal(h.dictionaries.length,0);assert.deepEqual(h.messages,[]);assert.deepEqual(h.dictionaryMessages,[]);
 }
});

test('dictionary invalidation drops in-flight snapshots and errors without blocking a fresh read',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.dict(()=>pending.promise);const old=h.loader.loadDictionaries();h.loader.invalidateDictionaries();h.dict(async()=>dictionary('/fresh'));await h.loader.loadDictionaries();if(fail)pending.reject(Error('old credential snapshot'));else pending.resolve(dictionary('/old'));assert.equal(await old,false);assert.deepEqual(h.dictionaries,[dictionary('/fresh')]);assert.deepEqual(h.dictionaryMessages,[]);}
});
test('standalone dictionary snapshots are bound to the settings visit',async()=>{
 const h=harness(),pending=deferred();h.dict(()=>pending.promise);const old=h.loader.loadDictionaries();h.visit(3);pending.resolve(dictionary('/old-visit'));assert.equal(await old,false);assert.deepEqual(h.dictionaries,[]);
});
