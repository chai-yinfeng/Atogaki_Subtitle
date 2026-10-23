import test from 'node:test';
import assert from 'node:assert/strict';
import {createSubtitleStyleSession} from '../src/subtitle-style-session.ts';
import {createSubtitleStyleBackend} from '../src/subtitle-style-backend.ts';
const flush=()=>new Promise(r=>setImmediate(r));
const styles={source:{font_family:'A'},translation:{font_family:'B'}};
const state={styles,font_report:{source:{},translation:{}}},fonts=[{family:'A'}],image={output_path:'/qa/frame.png',timestamp_ms:1000,font_report:state.font_report,font_events:[]};
function harness(){
 let context={jobId:'a',navigation:1,timestampMs:1000.4},valid=true,draft=structuredClone(styles);const pending=[],events=[],timers=[];
 const request=(kind,args)=>new Promise((resolve,reject)=>pending.push({kind,args,resolve,reject}));
 const tool=createSubtitleStyleSession({context:()=>context,available:()=>true,backend:Object.fromEntries(['load','fonts','save','preview'].map(kind=>[kind,(...args)=>request(kind,args)])),view:{open:()=>events.push(['open']),close:()=>events.push(['close']),valid:()=>valid,read:()=>({styles:structuredClone(draft),track:'bilingual'}),loaded:(...args)=>events.push(['loaded',...args]),saved:value=>events.push(['saved',value]),previewed:(...args)=>events.push(['previewed',...args]),message:value=>events.push(['message',value]),busy:(...args)=>events.push(['busy',...args])},later:(fn,delay)=>{const timer={fn,delay,active:true};timers.push(timer);return()=>timer.active=false;}});
 const tick=()=>{const t=timers.findLast(t=>t.active);assert(t);t.active=false;t.fn();};
 const open=async()=>{const run=tool.open();pending.at(-2).resolve(state);pending.at(-1).resolve(fonts);await run;};
 return {tool,pending,events,timers,tick,open,context,setContext:v=>context=v,setValid:v=>valid=v,setDraft:v=>draft=v};
}
test('style backend keeps all four wire contracts',async()=>{
 const calls=[],b=createSubtitleStyleBackend(async(...a)=>calls.push(a));await b.load('a');await b.fonts();await b.save('a',styles);await b.preview('a',styles,'source',12);
 assert.deepEqual(calls,[['get_subtitle_style_state',{jobId:'a'}],['list_subtitle_fonts'],['save_subtitle_styles',{request:{jobId:'a',styles}}],['preview_subtitle_styles',{request:{jobId:'a',styles,subtitleTrack:'source',timestampMs:12}}]]);
});
test('open loads once, caches fonts, enables only ready fields and schedules initial preview',async()=>{
 const h=harness();await h.open();assert.deepEqual(h.events.filter(e=>e[0]==='busy'),[['busy',true,false],['busy',false,true]]);assert.equal(h.timers.at(-1).delay,0);
 await h.tool.open();assert.equal(h.pending.length,2);h.tick();assert.deepEqual(h.pending[2].args,['a',styles,'bilingual',1000]);h.pending[2].resolve(image);await flush();assert.equal(h.events.find(e=>e[0]==='previewed')[3],true);
 h.tool.close();const run=h.tool.open();assert.equal(h.pending.at(-1).kind,'load');h.pending.at(-1).resolve(state);await run;assert.equal(h.pending.filter(r=>r.kind==='fonts').length,1);
});
test('rapid input debounces and invalid input never renders',async()=>{
 const h=harness();await h.open();h.tool.changed();h.tool.changed();assert.equal(h.timers.filter(t=>t.active).length,1);assert.equal(h.timers.at(-1).delay,700);h.setValid(false);h.tick();await flush();assert.equal(h.pending.length,2);assert.match(h.events.at(-1)[1],/修正/);
});
test('superseded preview is hidden and only one latest render is queued',async()=>{
 const h=harness();await h.open();h.tick();h.tool.changed();h.tool.changed();h.tick();assert.equal(h.pending.length,3);h.pending[2].resolve(image);await flush();assert(!h.events.some(e=>e[0]==='previewed'));assert.equal(h.timers.at(-1).delay,150);h.tick();assert.equal(h.pending.length,4);h.pending[3].resolve({...image,output_path:'/qa/latest.png'});await flush();assert.equal(h.events.find(e=>e[0]==='previewed')[1].output_path,'/qa/latest.png');assert.equal(h.events.find(e=>e[0]==='previewed')[3],false);
});
test('close and reopen reject stale load/finally without unlocking new visit',async()=>{
 const h=harness(),first=h.tool.open();h.tool.close();h.context.navigation=2;const second=h.tool.open();h.pending[0].resolve(state);h.pending[1].resolve(fonts);await first;assert(!h.events.some(e=>e[0]==='loaded'));assert.deepEqual(h.events.at(-1),['message','正在读取任务样式和本机字体…']);h.pending[2].resolve(state);h.pending[3].resolve(fonts);await second;assert.equal(h.events.filter(e=>e[0]==='loaded').length,1);
});
test('save freezes request, does not overwrite input and distinguishes later unsaved edits',async()=>{
 const h=harness();await h.open();h.tool.changed();const run=h.tool.save();assert.deepEqual(h.pending[2].args,['a',styles]);h.setDraft({...styles,source:{font_family:'new'}});h.tool.changed();h.pending[2].resolve(state);await run;assert(!h.events.some(e=>e[0]==='saved'));assert.match(h.events.filter(e=>e[0]==='message').at(-2)[1],/新修改尚未保存/);h.tick();h.pending[3].resolve(image);await flush();assert.equal(h.events.find(e=>e[0]==='previewed')[3],false);
});
test('saved preview reports saved state and failed save permits retry',async()=>{
 const h=harness();await h.open();h.tool.changed();const first=h.tool.save();h.pending[2].reject(Error('disk full'));await first;assert(h.events.some(e=>e[0]==='message'&&e[1].includes('disk full')));
 const next=h.tool.save();h.pending[3].resolve(state);await next;h.tick();h.pending[4].resolve(image);await flush();assert.equal(h.events.find(e=>e[0]==='previewed')[3],true);
});
test('close/dispose suppress late save and preview results and cancels timers',async()=>{
 for(const kind of ['save','preview']){
  const h=harness();await h.open();const run=kind==='save'?h.tool.save():h.tool.preview();h.tool.dispose();const count=h.events.length;h.pending[2].resolve(kind==='save'?state:image);await run;assert.equal(h.events.length,count);h.tool.close();assert.equal(h.events.length,count);assert(!h.timers.some(t=>t.active));await h.tool.open();assert.equal(h.events.length,count);
 }
});
test('load failure does not enable uninitialized form and same-task new navigation rejects errors',async()=>{
 const h=harness(),run=h.tool.open();h.pending[0].reject(Error('read failure'));h.pending[1].resolve(fonts);await run;assert.deepEqual(h.events.at(-1),['busy',false,false]);await h.tool.save();assert.equal(h.pending.length,2);
 h.tool.close();await h.open();const preview=h.tool.preview();h.context.navigation++;const count=h.events.length;h.pending.at(-1).reject(Error('old view'));await preview;assert.equal(h.events.length,count);
});
