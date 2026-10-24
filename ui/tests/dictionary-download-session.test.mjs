import test from 'node:test';
import assert from 'node:assert/strict';
import {createDictionaryDownloadBackend} from '../src/dictionary-download-backend.ts';
import {createDictionaryDownloadSession} from '../src/dictionary-download-session.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return{promise,resolve,reject};};
const state=(id,status)=>({dictionaryId:id,status,error:status==='failed'?'fixture failure':null});
function harness(){
 let visit=1,open=true,dead=false,downloads=[],network=async()=>{},start=async id=>state(id,'queued'),read=async()=>downloads,reload=async()=>{},newer=false,available=true;
 const calls=[],accepted=[],messages=[],cleared=[],acks=[],changes=[],timers=new Map(),invalidations=[];let next=0;
 const guard=()=>{if(dead)throw Error('released context');};
 const request={proxyMode:'environment',proxyUrl:null,modelMirrorUrl:'https://fixture.test'};
 const session=createDictionaryDownloadSession({backend:{network:r=>{calls.push(['network',r]);return network();},start:id=>{calls.push(['start',id]);return start(id);},states:()=>{calls.push(['states']);return read();}},visit:()=>{guard();return visit;},isOpen:()=>{guard();return open;},catalog:()=>{guard();return [{id:'a'},{id:'b'}];},downloads:()=>{guard();return downloads;},accepted:s=>{guard();downloads=s;accepted.push(s);},changed:()=>changes.push(session.busy),captureNetwork:()=>available?{request,acknowledge:()=>{acks.push(true);return newer;}}:null,invalidateLoads:()=>{guard();invalidations.push(true);},reload:async()=>{guard();calls.push(['reload']);return reload();},message:t=>messages.push(t),clearMessage:t=>cleared.push(t),schedule:fn=>{timers.set(++next,fn);return next;},cancelTimer:id=>timers.delete(id)});
 return{session,request,calls,accepted,messages,cleared,acks,changes,timers,invalidations,downloads:s=>downloads=s,network:f=>network=f,start:f=>start=f,read:f=>read=f,reload:f=>reload=f,newer:b=>newer=b,available:b=>available=b,visit:n=>visit=n,open:b=>open=b,deny:()=>dead=true};
}
test('dictionary download adapter preserves three existing commands and shapes',async()=>{
 const calls=[],b=createDictionaryDownloadBackend(async(...args)=>calls.push(args)),request={proxyMode:'direct',proxyUrl:null,modelMirrorUrl:null};await b.network(request);await b.start('a');await b.states();assert.deepEqual(calls,[['save_download_network_settings',{request}],['start_dictionary_download',{dictionaryId:'a'}],['dictionary_download_states',{}]]);
});
test('dictionary observation arms only while visible and active, including resolving',()=>{
 const h=harness();h.session.observe();assert.equal(h.timers.size,0);h.downloads([state('a','resolving')]);h.open(false);h.session.observe();assert.equal(h.timers.size,0);h.open(true);h.session.observe();assert.equal(h.timers.size,1);const old=[...h.timers.values()][0];h.open(false);h.visit(2);h.session.hidden();assert.equal(h.timers.size,0);h.open(true);h.session.observe();old();assert.deepEqual(h.calls,[]);assert.equal(h.timers.size,1);
});
test('overlapping dictionary status requests merge; next poll waits for completion',async()=>{
 const h=harness(),pending=deferred();h.downloads([state('a','downloading')]);h.read(()=>pending.promise);const one=h.session.refresh(),two=h.session.refresh();assert.equal(h.calls.length,1);assert.equal(h.timers.size,0);pending.resolve([state('a','resolving')]);await Promise.all([one,two]);assert.equal(h.accepted.length,1);assert.equal(h.timers.size,1);
});
test('new settings visit reads independently and ignores older success or failure',async()=>{
 for(const fail of [false,true]){const h=harness(),old=deferred();h.read(()=>old.promise);const one=h.session.refresh();h.visit(3);h.session.hidden();h.read(async()=>[state('b','queued')]);await h.session.refresh();if(fail)old.reject(Error('old'));else old.resolve([state('a','done')]);await one;assert.deepEqual(h.accepted,[[state('b','queued')]]);assert.deepEqual(h.messages,[]);}
});
test('download start freezes network, prevents duplicate intent and acknowledges only captured fields',async()=>{
 const h=harness(),pending=deferred();h.network(()=>pending.promise);const one=h.session.start('a');h.request.modelMirrorUrl='https://new.test';h.newer(true);await h.session.start('b');assert.equal(h.calls[0][1].modelMirrorUrl,'https://fixture.test');pending.resolve();await one;assert.deepEqual(h.acks,[true]);assert.equal(h.calls.filter(c=>c[0]==='start').length,1);assert.match(h.messages.at(-1),/新的网络输入尚未保存/);assert.equal(h.timers.size,1);await h.session.start('a');assert.equal(h.calls.filter(c=>c[0]==='start').length,1);
});
test('closing during preflight preserves explicit download but stops UI acknowledgement and hidden reads',async()=>{
 const h=harness(),pending=deferred();h.network(()=>pending.promise);const one=h.session.start('a');h.open(false);h.visit(2);h.session.hidden();pending.resolve();await one;assert.equal(h.calls.some(c=>c[0]==='start'),true);assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[]);assert.equal(h.timers.size,0);assert.equal(h.cleared.length,1);
});
test('late start reloads a reopened visit without applying old result or notice',async()=>{
 const h=harness(),pending=deferred();h.start(()=>pending.promise);const one=h.session.start('a');await new Promise(resolve=>setImmediate(resolve));h.visit(3);h.session.hidden();pending.resolve(state('a','queued'));await one;assert.deepEqual(h.accepted,[]);assert.equal(h.calls.filter(c=>c[0]==='reload').length,1);assert.equal(h.messages.length,1);
});
test('network failure and start failure after saved network are distinct and permit retry',async()=>{
 const h=harness();h.network(async()=>{throw Error('preflight');});await h.session.start('a');assert.equal(h.calls.some(c=>c[0]==='start'),false);assert.deepEqual(h.acks,[]);h.network(async()=>{});h.start(async()=>{throw Error('download');});await h.session.start('a');assert.match(h.messages.at(-1),/网络配置已保存，但无法开始词典下载/);assert.equal(h.session.busy,false);h.start(async id=>state(id,'queued'));await h.session.start('a');assert.equal(h.timers.size,1);
});
test('completion belongs to the previously active dictionary and does not report unrelated failure',async()=>{
 const h=harness();h.downloads([state('a','downloading'),state('b','failed')]);h.read(async()=>[state('a','done'),state('b','failed')]);await h.session.refresh();assert.equal(h.calls.filter(c=>c[0]==='reload').length,1);assert.match(h.messages.at(-1),/词典包已校验/);assert.equal(h.timers.size,0);
});
test('missing active dictionary never claims installation from an unrelated cached state',async()=>{
 const h=harness();h.downloads([state('a','downloading')]);h.read(async()=>[state('b','done')]);await h.session.refresh();assert.match(h.messages.at(-1),/未取得本次下载状态/);
});
test('cache completion immediately reloads, and committed refresh failure is precise',async()=>{
 const h=harness();h.start(async id=>state(id,'done'));await h.session.start('a');assert.equal(h.timers.size,0);assert.equal(h.calls.filter(c=>c[0]==='reload').length,1);assert.match(h.messages.at(-1),/词典包已校验/);h.reload(async()=>false);await h.session.start('a');assert.match(h.messages.at(-1),/下载操作已完成，但状态刷新失败/);
});
test('invalid catalog, hidden state or missing network fields never start a backend download',async()=>{
 const h=harness();await h.session.start('unknown');h.open(false);await h.session.start('a');h.open(true);h.available(false);await h.session.start('a');assert.deepEqual(h.calls,[]);assert.equal(h.session.busy,false);
});
test('status read failures do not stack observers and can retry',async()=>{
 const h=harness();h.downloads([state('a','downloading')]);h.read(async()=>{throw Error('fixture read');});await h.session.refresh();assert.match(h.messages.at(-1),/无法读取词典下载状态/);assert.equal(h.timers.size,1);h.read(async()=>[state('a','failed')]);await h.session.refresh();assert.equal(h.timers.size,0);assert.match(h.messages.at(-1),/下载失败/);
});
test('old status response cannot overwrite a newer download start',async()=>{
 const h=harness(),pending=deferred();h.read(()=>pending.promise);const read=h.session.refresh();await h.session.start('a');pending.resolve([state('b','done')]);await read;assert.deepEqual(h.accepted,[[state('a','queued')]]);
});
test('dispose during preflight stops further calls; late reads/start callbacks ignore released context',async()=>{
 for(const mode of ['network','start','read']){const h=harness(),pending=deferred();if(mode==='network')h.network(()=>pending.promise);else if(mode==='start')h.start(()=>pending.promise);else h.read(()=>pending.promise);const one=mode==='read'?h.session.refresh():h.session.start('a');await new Promise(resolve=>setImmediate(resolve));h.session.dispose();h.deny();const changes=h.changes.length;pending.resolve(mode==='read'?[state('a','done')]:state('a','queued'));await one;h.session.observe();await h.session.refresh();await h.session.start('a');h.session.hidden();assert.deepEqual(h.accepted,[]);assert.equal(h.changes.length,changes);assert.equal(h.timers.size,0);if(mode==='network')assert.equal(h.calls.some(c=>c[0]==='start'),false);}
});
