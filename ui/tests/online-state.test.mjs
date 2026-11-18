import test from 'node:test';
import assert from 'node:assert/strict';
import { DraftPresentation, OnlineCaptions, stableReadingMs } from '../src/online-state.ts';
const status=id=>({sessionId:id,phase:'running',error:null});
const event=(sequence,body,sessionId='s')=>({sequence,sessionId,event:body});
test('new window can recover the paired original from an actual MT input snapshot',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[event(300,{
  kind:'translation_final',group_id:'g',start_s:10,end_s:18,text:'译文',source_text:'对应的Qwen原文',source_revisions:{a:2,b:2}})]});
 assert.equal(state.tracks().sourceParts[0].text,'对应的Qwen原文');
 assert.equal(state.tracks().sourceParts[0].kind,'source_final');
});

test('late final ASR cannot replace the next segment draft; MT owns a separate group',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'source_draft',segment_id:'next',start_s:5,text:'next'}));
 state.accept(event(2,{kind:'source_final',segment_id:'old',start_s:0,text:'old final'}));
 state.accept(event(3,{kind:'translation_draft',group_id:'g',start_s:0,text:'whole group',source_text:'old and next'}));
 assert.equal(state.tracks().source.text,'next');assert.equal(state.tracks().translation.text,'whole group');
});
test('snapshot arriving after a newer event preserves the newer version',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(3,{kind:'translation_final',group_id:'g',text:'final'}));
 state.snapshot({status:status('s'),events:[event(2,{kind:'translation_draft',group_id:'g',text:'draft'})]});
 assert.equal(state.tracks().translation.text,'final');
});
test('new session clears old captions and rejects late events from the old session',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[event(1,{kind:'source_final',segment_id:'a',text:'old'})]});
 state.snapshot({status:status('new'),events:[]});state.accept(event(2,{kind:'source_final',segment_id:'a',text:'late'}));
 assert.equal(state.tracks().source,null);
});

test('prefix rewrites wait two seconds and replace the entire latest snapshot',()=>{
 const gate=new DraftPresentation();
 const draft=text=>({kind:'translation_draft',group_id:'g',text});
 assert.equal(gate.select(draft('我想'),0).text,'我想');
 assert.equal(gate.select(draft('我想知道'),100).text,'我想知道');
 assert.equal(gate.select(draft('你想知道'),300).text,'我想知道');
 assert.equal(gate.select(draft('他想知道结果'),1900).text,'我想知道');
 assert.equal(gate.select(draft('他想知道结果'),2000).text,'他想知道结果');
 assert.equal(gate.select(draft('他想知道结果了'),2100).text,'他想知道结果了');
 assert.equal(gate.select({...draft('最终结果'),kind:'translation_final'},2200).text,'最终结果');
});
test('new groups and clearing the activity bypass the prefix gate',()=>{
 const gate=new DraftPresentation();
 gate.select({kind:'source_draft',segment_id:'a',text:'old'},0);
 assert.equal(gate.select({kind:'source_draft',segment_id:'b',text:'new'},10).text,'new');
 assert.equal(gate.select(null,20),null);
 assert.equal(gate.select({kind:'source_draft',segment_id:'b',text:'reset'},30).text,'reset');
});

test('terminal translation failure replaces draft immediately, keeps Qwen pair and survives snapshots',()=>{
 const model=new OnlineCaptions();model.snapshot({status:status('s'),events:[]});
 const source=event(1,{kind:'source_final',segment_id:'a',start_s:0,end_s:2,text:'原文'});
 const draft=event(2,{kind:'translation_draft',group_id:'g',start_s:0,end_s:2,text:'旧草稿',source_revisions:{a:1}});
 const failed=event(3,{kind:'translation_failed',group_id:'g',start_s:0,end_s:2,source_text:'原文',source_revisions:{a:1}});
 model.accept(source);model.accept(draft);
 const gate=new DraftPresentation();gate.select(draft.event,0);
 model.accept(failed);
 const shown=model.tracks().translationParts[0];
 assert.equal(shown.text,'本段翻译失败');assert.equal(gate.select(shown,10).kind,'translation_failed');
 assert.equal(model.tracks().sourceParts[0].kind,'source_final');
 assert.equal(model.translationFailures,1);assert.equal(failed.event.text,undefined);
 model.accept(event(4,{...draft.event,text:'晚到草稿'}));
 assert.equal(model.tracks().translationParts[0].kind,'translation_failed');
 const restored=new OnlineCaptions();restored.snapshot({status:status('s'),events:[failed]});
 assert.equal(restored.tracks().sourceParts[0].kind,'source_final');
 assert.equal(restored.translationFailures,1);
 restored.snapshot({status:status('other'),events:[]});assert.equal(restored.translationFailures,0);
});

function fixture() {
 let now=0;
 const model=new OnlineCaptions(()=>now);
 model.snapshot({status:status('s'),events:[]});
 return {model, at(value){now=value;return model.tracks();}};
}
const source=(id,start,end,text=id,kind='source_final')=>({kind,segment_id:id,start_s:start,end_s:end,text});
const translation=(id,start,end,text=id,kind='translation_final',ids=[])=>({kind,group_id:id,start_s:start,end_s:end,text,source_revisions:Object.fromEntries(ids.map(id=>[id,1]))});
test('A retires after stable reading time without any B source or translation',()=>{
 const {model,at}=fixture();
 model.accept(event(1,source('a',0,4)));
 model.accept(event(2,translation('A',0,4,'译文A','translation_final',['a'])));
 assert.deepEqual(at(0).sourceParts.map(e=>e.text),['a']);
 assert.equal(at(stableReadingMs-1).translationParts[0].text,'译文A');
 assert.deepEqual(at(stableReadingMs).translationParts,[]);
 assert.deepEqual(at(stableReadingMs).sourceParts,[]);
 assert.equal(model.tracks().translation.text,'译文A'); // Display is not processing history.
});
test('B accumulates in background and appears at its latest revision with no A tail',()=>{
 const {model,at}=fixture();
 model.accept(event(1,source('a',0,4)));
 model.accept(event(2,translation('A',0,4,'A final','translation_final',['a'])));
 at(0);
 model.accept(event(3,source('b',4,12,'long B','source_draft')));
 model.accept(event(4,translation('B',4,8,'B short','translation_draft',['b'])));
 model.accept(event(5,translation('B',4,12,'B latest complete snapshot','translation_draft',['b'])));
 assert.deepEqual(at(stableReadingMs-1).sourceParts.map(e=>e.text),['a']);
 const frame=at(stableReadingMs);
 assert.deepEqual(frame.sourceParts.map(e=>e.text),['long B']);
 assert.deepEqual(frame.translationParts.map(e=>e.text),['B latest complete snapshot']);
});
test('B may have source only when A leaves; old translation is not carried across',()=>{
 const {model,at}=fixture();
 model.accept(event(1,source('a',0,4)));
 model.accept(event(2,translation('A',0,4,'A','translation_final',['a'])));at(0);
 model.accept(event(3,source('b',4,9,'B original','source_draft')));
 const frame=at(stableReadingMs);
 assert.deepEqual(frame.sourceParts.map(e=>e.text),['B original']);
 assert.deepEqual(frame.translationParts,[]);
});
test('a queued final starts its reading time when displayed, not when received',()=>{
 const {model,at}=fixture();
 model.accept(event(1,translation('A',0,4)));
 model.accept(event(2,translation('B',4,8)));at(0);
 assert.equal(at(stableReadingMs).translationParts[0].group_id,'B');
 assert.equal(at(2*stableReadingMs-1).translationParts[0].group_id,'B');
 assert.deepEqual(at(2*stableReadingMs).translationParts,[]);
});
test('late MT final has its full reading time even after a long audio pause',()=>{
 const {model,at}=fixture();
 model.accept(event(1,translation('A',0,4,'preview','translation_draft')));at(0);
 assert.equal(at(20000).translationParts[0].text,'preview');
 model.accept(event(2,translation('A',0,4,'final')));
 assert.equal(at(25000).translationParts[0].text,'final');
 assert.equal(at(25000+stableReadingMs-1).translationParts[0].text,'final');
 assert.deepEqual(at(25000+stableReadingMs).translationParts,[]);
});
test('an unchanged final uses time already spent reading the visible draft',()=>{
 const {model,at}=fixture();
 model.accept(event(1,translation('A',0,4,'同一句','translation_draft')));
 model.observeTranslation(at(0).translationParts[0]);
 model.accept(event(2,translation('A',0,4,'同一句')));
 assert.equal(at(800).translationParts[0].kind,'translation_final');
 assert.deepEqual(at(stableReadingMs).translationParts,[]);
});
test('an unchanged late final advances immediately to the next ready group',()=>{
 const {model,at}=fixture();
 model.accept(event(1,translation('A',0,4,'同一句','translation_draft')));
 model.observeTranslation(at(0).translationParts[0]);
 model.accept(event(2,translation('A',0,4,'同一句')));
 model.accept(event(3,translation('B',4,8,'下一句','translation_draft')));
 assert.equal(at(2500).translationParts[0].group_id,'B');
});
test('the same words with a new boundary still get one full reading window',()=>{
 const {model,at}=fixture();
 model.accept(event(1,translation('A',0,6,'同一句','translation_draft')));
 model.observeTranslation(at(0).translationParts[0]);
 model.accept(event(2,translation('A',0,4,'同一句')));
 assert.equal(at(2500).translationParts[0].kind,'translation_final');
 assert.equal(at(2500+stableReadingMs-1).translationParts[0].group_id,'A');
 assert.deepEqual(at(2500+stableReadingMs).translationParts,[]);
});
test('snapshots do not restart a reading timer or resurrect an expired page',()=>{
 const {model,at}=fixture();
 const final=event(1,translation('A',0,4));model.accept(final);at(0);
 model.snapshot({status:status('s'),events:[final]});at(stableReadingMs-1);
 assert.deepEqual(at(stableReadingMs).translationParts,[]);
 model.snapshot({status:status('s'),events:[final,event(2,translation('A',0,4,'late draft','translation_draft'))]});
 assert.deepEqual(at(4000).translationParts,[]);
});
test('split MT final moves the display boundary only after its own reading time',()=>{
 const {model,at}=fixture();
 model.accept(event(1,source('a',0,3)));
 model.accept(event(2,source('b',3,8)));
 model.accept(event(3,translation('A',0,8,'whole preview','translation_draft',['a','b'])));at(0);
 model.accept(event(4,translation('A',0,3,'prefix final','translation_final',['a'])));
 model.accept(event(5,translation('B',3,8,'tail preview','translation_draft',['b'])));
 assert.deepEqual(at(1000).sourceParts.map(e=>e.text),['a']);
 assert.deepEqual(at(4000).sourceParts.map(e=>e.text),['b']);
 assert.deepEqual(at(4000).translationParts.map(e=>e.text),['tail preview']);
});
test('displayed MT snapshot controls its source pairing while prefix edits are gated',()=>{
 const {model,at}=fixture();
 model.accept(event(1,source('a',0,3)));
 const shown=translation('A',0,3,'old','translation_draft',['a']);model.accept(event(2,shown));at(0);
 model.accept(event(3,source('b',3,8)));
 model.accept(event(4,translation('A',0,8,'new','translation_draft',['a','b'])));
 assert.deepEqual(model.tracks([shown]).sourceParts.map(e=>e.text),['a','b']);
});
test('paired source remains available after live ASR map eviction',()=>{
 const {model,at}=fixture();
 model.accept(event(1,source('a',0,4)));
 model.accept(event(2,translation('A',0,4,'A','translation_final',['a'])));at(0);
 for(let i=1;i<=70;i++)model.accept(event(i+2,source('s'+i,i+4,i+5)));
 assert.deepEqual(at(stableReadingMs-1).sourceParts.map(e=>e.text),['a']);
});
test('new session resets both the page boundary and reading clock',()=>{
 const {model,at}=fixture();model.accept(event(1,translation('A',0,4)));at(0);at(stableReadingMs);
 model.snapshot({status:status('new'),events:[event(1,translation('N',0,2),'new')]});
 assert.equal(at(4000).translationParts[0].group_id,'N');
 assert.deepEqual(at(7000).translationParts,[]);
});
test('source-only sessions retain the existing whole-unit reading range',()=>{
 const {model}=fixture();model.snapshot({status:{...status('s'),translationEnabled:false},events:[]});
 for(let i=1;i<=9;i++)model.accept(event(i,source('s'+i,i-1,i)));
 assert.deepEqual(model.tracks().sourceParts.map(e=>e.segment_id),['s5','s6','s7','s8','s9']);
 assert.deepEqual(model.tracks().translationParts,[]);
});

test('opening a window restores the latest group instead of replaying retained history',()=>{
 let now=10000;const model=new OnlineCaptions(()=>now);
 model.snapshot({status:status('s'),events:[event(1,translation('old',0,4)),event(2,translation('latest',4,8))]});
 assert.deepEqual(model.tracks().translationParts.map(e=>e.group_id),['latest']);
 now=13000;assert.deepEqual(model.tracks().translationParts,[]);
});
