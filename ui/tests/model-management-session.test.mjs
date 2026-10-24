import test from 'node:test';
import assert from 'node:assert/strict';
import { createModelManagementSession } from '../src/model-management-session.ts';
import { createModelManagementBackend } from '../src/model-management-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
const state=(id,status)=>({modelId:id,status,downloadedBytes:1,totalBytes:2,path:null,error:null,source:null,active:false});
function harness(){
 let open=true,visit=1,dead=false,downloads=[],read=async()=>downloads,network=async()=>{},start=async id=>state(id,'queued'),cancel=async id=>state(id,'cancelling'),select=async()=>{},uninstall=async()=>{},reload=async()=>true,newer=false;
 const calls=[],messages=[],accepted=[],timers=new Map(),acks=[];let timerId=0;
 const guard=()=>{if(dead)throw Error('disposed context');};
 const session=createModelManagementSession({backend:{network:r=>{calls.push(['network',r]);return network(r);},start:id=>{calls.push(['start',id]);return start(id);},states:()=>{calls.push(['states']);return read();},cancel:id=>{calls.push(['cancel',id]);return cancel(id);},select:id=>{calls.push(['select',id]);return select(id);},uninstall:id=>{calls.push(['uninstall',id]);return uninstall(id);}},visit:()=>{guard();return visit;},isOpen:()=>{guard();return open;},catalog:()=>[{id:'a',kind:'whisper'},{id:'b',kind:'hy-mt2'}],downloads:()=>{guard();return downloads;},accepted:value=>{accepted.push(value);downloads=value;},changed:()=>{},captureNetwork:()=>({request:{proxyMode:'direct',proxyUrl:null,modelMirrorUrl:null},acknowledge:()=>{acks.push('network');return newer;}}),captureSelection:kind=>({acknowledge:()=>{acks.push(kind);return newer;}}),invalidateLoads:()=>calls.push(['invalidate']),reload:()=>{calls.push(['reload']);return reload();},message:text=>messages.push(text),schedule:fn=>{timers.set(++timerId,fn);return timerId;},cancelTimer:id=>timers.delete(id)});
 return {session,calls,messages,accepted,timers,acks,open:x=>open=x,visit:x=>visit=x,downloads:x=>downloads=x,read:f=>read=f,network:f=>network=f,start:f=>start=f,cancel:f=>cancel=f,select:f=>select=f,uninstall:f=>uninstall=f,reload:f=>reload=f,newer:x=>newer=x,deny:()=>dead=true};
}

test('model adapter preserves six IPC commands and network envelope',async()=>{
 const calls=[],b=createModelManagementBackend(async(...args)=>calls.push(args)),network={proxyMode:'direct',proxyUrl:null,modelMirrorUrl:null};await b.network(network);await b.start('a');await b.states();await b.cancel('a');await b.select('a');await b.uninstall('a');assert.deepEqual(calls,[['save_download_network_settings',{request:network}],['start_model_download',{modelId:'a'}],['model_download_states',{}],['cancel_model_download',{modelId:'a'}],['select_model',{modelId:'a'}],['uninstall_model',{modelId:'a'}]]);
});

test('model observer only arms while visible and active; hidden timer callbacks stay invalid',async()=>{
 const h=harness();h.downloads([state('a','downloading')]);h.open(false);h.session.observe();assert.equal(h.timers.size,0);h.open(true);h.session.observe();assert.equal(h.timers.size,1);const old=[...h.timers.values()][0];h.open(false);h.visit(2);h.session.hidden();assert.equal(h.timers.size,0);h.open(true);h.session.observe();old();assert.deepEqual(h.calls,[]);assert.equal(h.timers.size,1);
});

test('overlapping model status requests are merged and next poll waits for completion',async()=>{
 const h=harness(),pending=deferred();h.downloads([state('a','downloading')]);h.read(()=>pending.promise);const one=h.session.refresh(),two=h.session.refresh();assert.equal(h.calls.filter(c=>c[0]==='states').length,1);assert.equal(h.timers.size,0);pending.resolve([state('a','downloading')]);await Promise.all([one,two]);assert.equal(h.timers.size,1);assert.equal(h.accepted.length,1);
});

test('reopened model observer does not wait for old read or accept its late result',async()=>{
 const h=harness(),old=deferred();h.read(()=>old.promise);const one=h.session.refresh();h.visit(2);h.session.hidden();h.read(async()=>[state('b','queued')]);await h.session.refresh();old.resolve([state('a','done')]);await one;assert.deepEqual(h.accepted,[[state('b','queued')]]);
});

test('download start captures network once, blocks direct duplicates and preserves newer-input acknowledgement',async()=>{
 const h=harness(),pending=deferred();h.network(()=>pending.promise);const one=h.session.run('start','a');await h.session.run('start','b');assert.equal(h.session.busy,true);h.newer(true);pending.resolve();await one;assert.deepEqual(h.acks,['network']);assert.equal(h.calls.filter(c=>c[0]==='start').length,1);assert.match(h.messages.at(-1),/新的网络输入尚未保存/);assert.equal(h.timers.size,1);assert.equal(h.session.busy,false);
});

test('closing settings preserves the explicit download intent without clearing input or observing in background',async()=>{
 const h=harness(),pending=deferred();h.network(()=>pending.promise);const one=h.session.run('start','a');h.visit(2);h.open(false);h.session.hidden();pending.resolve();await one;assert.equal(h.calls.some(c=>c[0]==='start'),true);assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[]);assert.equal(h.timers.size,0);
});

test('late model start may finish in backend but only reloads a reopened visit',async()=>{
 const h=harness(),pending=deferred();h.start(()=>pending.promise);const one=h.session.run('start','a');await new Promise(resolve=>setImmediate(resolve));h.visit(3);h.session.hidden();pending.resolve(state('a','queued'));await one;assert.deepEqual(h.accepted,[]);assert.equal(h.calls.filter(c=>c[0]==='reload').length,1);assert.equal(h.messages.length,1);
});

test('model select confirms only submitted path version and reports preserved new input',async()=>{
 const h=harness(),pending=deferred();h.select(()=>pending.promise);const one=h.session.run('select','a');h.newer(true);pending.resolve(state('a','done'));await one;assert.deepEqual(h.acks,['whisper']);assert.match(h.messages.at(-1),/新的路径输入尚未保存/);assert.equal(h.calls.filter(c=>c[0]==='reload').length,1);
});

test('failed network start and committed uninstall refresh failure are reported distinctly',async()=>{
 const h=harness();h.network(async()=>{throw Error('network rejected');});await h.session.run('start','a');assert.equal(h.calls.some(c=>c[0]==='start'),false);assert.deepEqual(h.acks,[]);
 h.network(async()=>{});h.start(async()=>{throw Error('start rejected');});await h.session.run('start','a');assert.match(h.messages.at(-1),/网络配置已保存，但无法开始下载/);
 h.reload(async()=>false);await h.session.run('uninstall','a');assert.match(h.messages.at(-1),/模型操作已完成，但状态刷新失败/);assert.equal(h.session.busy,false);
});

test('model cancellation invalidates old reads and resumes a single observer',async()=>{
 const h=harness(),old=deferred();h.downloads([state('a','downloading')]);h.read(()=>old.promise);const one=h.session.refresh();await h.session.run('cancel','a');assert.deepEqual(h.accepted,[[state('a','cancelling')]]);assert.equal(h.timers.size,1);old.resolve([state('a','downloading')]);await one;assert.deepEqual(h.accepted,[[state('a','cancelling')]]);
});

test('terminal polling reports the active download failure rather than another installed model',async()=>{
 const h=harness();h.downloads([state('b','done'),state('a','downloading')]);h.read(async()=>[state('b','done'),{...state('a','failed'),error:'fixture failure'}]);await h.session.refresh();assert.match(h.messages.at(-1),/下载失败：fixture failure/);assert.equal(h.timers.size,0);assert.equal(h.calls.filter(c=>c[0]==='reload').length,1);
});

test('model dispose releases timer and ignores late callbacks without accessing released context',async()=>{
 const h=harness(),pending=deferred();h.downloads([state('a','downloading')]);h.session.observe();const old=[...h.timers.values()][0];h.read(()=>pending.promise);const one=h.session.refresh();h.session.dispose();h.deny();old();pending.resolve([state('a','done')]);await one;await h.session.refresh();await h.session.run('start','a');h.session.observe();assert.equal(h.timers.size,0);assert.deepEqual(h.accepted,[]);
});

test('cached model completion refreshes settings immediately without leaving a polling timer',async()=>{
 const h=harness();h.start(async id=>state(id,'done'));await h.session.run('start','a');assert.equal(h.timers.size,0);assert.equal(h.calls.filter(c=>c[0]==='reload').length,1);assert.equal(h.messages.at(-1),'模型已校验并安装。');
});


test('disposing the frontend during network preflight sends no further calls or view callbacks',async()=>{
 const h=harness(),pending=deferred();h.network(()=>pending.promise);const one=h.session.run('start','a');h.session.dispose();h.deny();pending.resolve();await one;assert.equal(h.calls.some(c=>c[0]==='start'),false);assert.deepEqual(h.acks,[]);assert.deepEqual(h.accepted,[]);
});

test('late model mutation receipts cannot invalidate or present after dispose',async()=>{
 for(const action of ['start','select','cancel','uninstall']){
  const h=harness(),pending=deferred();h[action](()=>pending.promise);const one=h.session.run(action,'a');await new Promise(resolve=>setImmediate(resolve));const calls=h.calls.length,messages=h.messages.length;h.session.dispose();h.deny();pending.resolve(state('a','done'));await one;assert.equal(h.calls.length,calls);assert.equal(h.messages.length,messages);assert.deepEqual(h.accepted,[]);
 }
});
