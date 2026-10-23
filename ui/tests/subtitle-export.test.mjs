import test from 'node:test';
import assert from 'node:assert/strict';
import { createSubtitleExport } from '../src/subtitle-export.ts';
const flush=()=>new Promise(resolve=>setImmediate(resolve));
function harness(){
 let context={jobId:'a',navigation:1,contextStaleIds:[],inputPath:'C:\\media\\a.mp4',segments:[{translation_stale:false,translated_text:'translation'}]},dirty=false,available=true;
 const pending=[],events=[];
 const request=(kind,args)=>new Promise((resolve,reject)=>pending.push({kind,args,resolve,reject}));
 const tool=createSubtitleExport({context:()=>context,available:()=>available,dirty:()=>dirty,fileManager:'Finder',
  backend:Object.fromEntries(['pickDirectory','preview','export','reveal'].map(kind=>[kind,(...args)=>request(kind,args)])),
  view:{choose:()=>request('choose',[]),overwrite:names=>request('overwrite',names),cancel:()=>events.push(['cancel'])},
  busyChanged:v=>events.push(['busy',v]),status:(...args)=>events.push(['status',...args]),resultChanged:v=>events.push(['result',v])});
 return {tool,pending,events,context,setContext:v=>context=v,setDirty:v=>dirty=v,setAvailable:v=>available=v};
}
const output={source_srt:'/out/a.srt',translated_srt:'/out/a.zh.srt',bilingual_srt:'/out/a.bi.srt',bilingual_ass:'/out/a.ass',missing_translation_count:0};
test('source-only export remains possible with missing translations and preserves wire values',async()=>{
 const h=harness();h.context.segments[0].translated_text=null;const run=h.tool.export();h.pending[0].resolve(['source_srt']);await flush();
 assert.equal(h.pending[1].kind,'pickDirectory');assert.deepEqual(h.pending[1].args,['C:\\media']);h.pending[1].resolve('/out');await flush();
 assert.deepEqual(h.pending[2].args,['a','/out',['source_srt']]);h.pending[2].resolve({existing_files:[]});await flush();
 assert.deepEqual(h.pending[3].args,['a','/out',['source_srt'],false]);h.pending[3].resolve(output);await run;
 assert.equal(h.tool.lastPath,'/out/a.srt');assert.deepEqual(h.events.filter(e=>e[0]==='busy'),[['busy',true],['busy',false]]);
 const reveal=h.tool.reveal();assert.deepEqual(h.pending[4].args,['/out/a.srt']);h.pending[4].resolve();await reveal;
});
test('dirty drafts and stale/missing translations stop before directory and export requests',async()=>{
 const h=harness();h.setDirty(true);await h.tool.export();assert.equal(h.pending.length,0);
 h.setDirty(false);h.context.segments[0].translation_stale=true;const run=h.tool.export();h.pending[0].resolve(['bilingual_srt']);await run;
 assert.equal(h.pending.length,1);assert.match(h.events.at(-1)[1],/待重译 1/);
 const next=h.tool.export();h.setDirty(true);h.pending[1].resolve(['source_srt']);await next;assert.equal(h.pending.length,2);
});
test('overwrite cancellation never writes and approval is attached to the frozen task',async()=>{
 for(const accepted of [false,true]){
  const h=harness();const run=h.tool.export();h.pending[0].resolve(['bilingual_ass']);await flush();h.pending[1].resolve('/out');await flush();
  h.pending[2].resolve({existing_files:['C:\\out\\<name>.ass','/out/second.ass']});await flush();
  assert.deepEqual(h.pending[3].args,['<name>.ass','second.ass']);h.pending[3].resolve(accepted);await flush();
  if(accepted){assert.deepEqual(h.pending[4].args,['a','/out',['bilingual_ass'],true]);h.pending[4].resolve(output);}
  await run;assert.equal(h.pending.filter(r=>r.kind==='export').length,Number(accepted));
 }
});
test('reset/new task prevents late picker from previewing or clearing a new lock',async()=>{
 const h=harness(),old=h.tool.export();h.pending[0].resolve(['source_srt']);await flush();h.tool.reset();
 h.setContext({...h.context,jobId:'b',navigation:2});const next=h.tool.export();h.pending[2].resolve(['source_srt']);await flush();
 const count=h.events.length;h.pending[1].resolve('/old');await old;assert.equal(h.events.length,count);assert.equal(h.pending.filter(r=>r.kind==='preview').length,0);
 h.pending[3].resolve(null);await next;
});
test('context changes during overwrite prevent an export even without reset',async()=>{
 const h=harness(),run=h.tool.export();h.pending[0].resolve(['source_srt']);await flush();h.pending[1].resolve('/out');await flush();h.pending[2].resolve({existing_files:['/out/a.srt']});await flush();
 h.context.jobId='b';h.context.navigation=2;h.pending[3].resolve(true);await run;assert.equal(h.pending.some(r=>r.kind==='export'),false);assert.equal(h.tool.lastPath,null);
});
test('late completion and errors after dispose never publish paths or messages',async()=>{
 for(const fail of [false,true]){
  const h=harness(),run=h.tool.export();h.pending[0].resolve(['source_srt']);await flush();h.pending[1].resolve('/out');await flush();h.pending[2].resolve({existing_files:[]});await flush();
  h.tool.dispose();const count=h.events.length;if(fail)h.pending[3].reject(Error('late failure'));else h.pending[3].resolve(output);await run;
  assert.equal(h.events.length,count);assert.equal(h.tool.lastPath,null);await h.tool.export();assert.equal(h.pending.length,4);
 }
});
test('duplicate starts are suppressed; backend failure unlocks and allows retry',async()=>{
 const h=harness(),run=h.tool.export();await h.tool.export();assert.equal(h.pending.length,1);h.pending[0].resolve(['source_srt']);await flush();h.pending[1].reject(Error('picker failed'));await run;
 assert.match(h.events.find(e=>e[0]==='status'&&e[1].includes('失败'))[1],/picker failed/);assert.deepEqual(h.events.at(-1),['busy',false]);
 h.setAvailable(false);await h.tool.export();assert.equal(h.pending.length,2);
 h.setAvailable(true);const next=h.tool.export();h.pending[2].resolve(null);await next;
});

test('context-only stale translation blocks translated export but allows source-only',async()=>{
 const h=harness();h.context.segments[0].id='c';h.context.contextStaleIds=['c'];const translated=h.tool.export();h.pending[0].resolve(['bilingual_srt']);await translated;assert.equal(h.pending.length,1);assert.match(h.events.at(-1)[1],/待重译 1/);
 const source=h.tool.export();h.pending[1].resolve(['source_srt']);await flush();assert.equal(h.pending[2].kind,'pickDirectory');h.pending[2].resolve(null);await source;
});
