import test from 'node:test';
import assert from 'node:assert/strict';
import {createNetworkTestSession} from '../src/network-test-session.ts';
import {createNetworkTestBackend} from '../src/network-test-backend.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return{promise,resolve,reject};};
const row={label:'fixture',requestedUrl:'https://example.test',ok:true,status:200,resolvedHost:'example.test',error:null};
function harness(){
 let visit=1,open=true,eligible=true,exists=true,dead=false,work=async()=>[row];
 const request={proxyMode:'manual',proxyUrl:'http://fixture.test',modelMirrorUrl:null},calls=[],changes=[];
 const guard=()=>{if(dead)throw Error('released context');};
 const session=createNetworkTestSession({backend:{test:r=>{calls.push(r);return work();}},visit:()=>{guard();return visit;},isOpen:()=>{guard();return open;},capture:()=>exists?{request,current:()=>{guard();return eligible;}}:null,changed:()=>changes.push(session.state.phase)});
 return {session,request,calls,changes,work:f=>work=f,visit:n=>visit=n,open:b=>open=b,eligible:b=>eligible=b,exists:b=>exists=b,deny:()=>dead=true};
}
test('network diagnostic adapter preserves exact request without persisting it',async()=>{
 const calls=[],backend=createNetworkTestBackend(async(...args)=>{calls.push(args);return[row];}),request={proxyMode:'environment',proxyUrl:null,modelMirrorUrl:null};
 assert.deepEqual(await backend.test(request),[row]);assert.deepEqual(calls,[['test_network_connection',{request}]]);
});
test('network test freezes parameters, prevents duplicate requests and copies diagnostics',async()=>{
 const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run();h.request.proxyUrl='new';await h.session.run();assert.equal(h.calls.length,1);assert.equal(h.calls[0].proxyUrl,'http://fixture.test');assert.equal(h.session.busy,true);
 pending.resolve([row]);await run;assert.equal(h.session.state.phase,'result');const snapshot=h.session.state;snapshot.checks[0].label='changed';assert.equal(h.session.state.checks[0].label,'fixture');assert.equal(h.session.busy,false);
});
test('input while pending invalidates successful and failed old diagnostics, allowing retry after completion',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run();h.session.parametersChanged();assert.equal(h.session.state.phase,'stale');await h.session.run();assert.equal(h.calls.length,1);if(fail)pending.reject(Error('old error'));else pending.resolve([row]);await run;assert.equal(h.session.state.phase,'stale');assert.equal(h.session.busy,false);h.work(async()=>[row]);await h.session.run();assert.equal(h.session.state.phase,'result');}
});
test('programmatically changed fields reject late success and failure even without DOM events',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run();h.eligible(false);if(fail)pending.reject(Error('outdated'));else pending.resolve([row]);await run;assert.equal(h.session.state.phase,'stale');assert.deepEqual(h.session.state.checks,[]);}
});
test('current failure is visible and an explicit retry can succeed',async()=>{
 const h=harness();h.work(async()=>{throw Error('fixture invalid proxy');});await h.session.run();assert.equal(h.session.state.phase,'error');assert.match(h.session.state.error,/fixture invalid proxy/);h.work(async()=>[row]);await h.session.run();assert.equal(h.session.state.phase,'result');h.session.parametersChanged();assert.equal(h.session.state.phase,'stale');assert.deepEqual(h.session.state.checks,[]);
});
test('close and reopen discard old diagnostics and do not start a concurrent network test',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run();h.visit(3);h.session.hidden();await h.session.run();assert.equal(h.calls.length,1);if(fail)pending.reject(Error('old failure'));else pending.resolve([row]);await run;assert.equal(h.session.state.phase,'idle');assert.equal(h.session.busy,false);h.work(async()=>[row]);await h.session.run();assert.equal(h.calls.length,2);}
});
test('closed settings or unavailable inputs issue no network request; idle changes remain idle',async()=>{
 const h=harness();h.session.parametersChanged();assert.equal(h.session.state.phase,'idle');h.open(false);await h.session.run();h.open(true);h.exists(false);await h.session.run();assert.equal(h.calls.length,0);
});
test('dispose suppresses late success, failure and future requests without reading context',async()=>{
 for(const fail of [false,true]){const h=harness(),pending=deferred();h.work(()=>pending.promise);const run=h.session.run();h.session.dispose();h.deny();const changes=h.changes.length;if(fail)pending.reject(Error('late'));else pending.resolve([row]);await run;await h.session.run();h.session.hidden();h.session.parametersChanged();assert.equal(h.changes.length,changes);assert.equal(h.session.state.phase,'idle');assert.equal(h.session.busy,false);}
});
