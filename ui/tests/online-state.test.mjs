import test from 'node:test';
import assert from 'node:assert/strict';
import { DraftPresentation, OnlineCaptions } from '../src/online-state.ts';
const status=id=>({sessionId:id,phase:'running',error:null});
const event=(sequence,body,sessionId='s')=>({sequence,sessionId,event:body});
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

test('three bilingual finals survive the event ring while the next group changes',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 for(let i=1;i<=4;i++)state.accept(event(i,{kind:'translation_final',group_id:`g${i}`,start_s:i,end_s:i+1,source_text:`source ${i}`,text:`translation ${i}`}));
 for(let i=5;i<310;i++)state.accept(event(i,{kind:'translation_draft',group_id:'active',start_s:6,end_s:7,text:`draft ${i}`}));
 assert.deepEqual(state.tracks().history.map(e=>e.source_text),['source 2','source 3','source 4']);
 assert.equal(state.tracks().activeTranslation.text,'draft 309');
 state.accept(event(310,{kind:'translation_final',group_id:'active',start_s:6,end_s:7,source_text:'paired input',text:'final'}));
 assert.equal(state.tracks().activeTranslation,null);
 assert.deepEqual(state.tracks().history.map(e=>e.group_id),['g3','g4','active']);
});
test('settled translation hides its covered ASR but keeps the next source active',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'source_final',segment_id:'a',start_s:0,end_s:3,text:'a'}));
 state.accept(event(2,{kind:'translation_final',group_id:'g',start_s:0,end_s:3,source_text:'paired a',text:'A'}));
 assert.equal(state.tracks().activeSource,null);
 state.accept(event(3,{kind:'source_draft',segment_id:'b',start_s:3,end_s:4,text:'b'}));
 assert.equal(state.tracks().activeSource.text,'b');
});
test('without MT, the last three ASR finals are retained separately from the draft',()=>{
 const state=new OnlineCaptions();state.snapshot({status:{...status('s'),translationEnabled:false},events:[]});
 for(let i=1;i<=4;i++)state.accept(event(i,{kind:'source_final',segment_id:`s${i}`,start_s:i,text:`source ${i}`}));
 state.accept(event(5,{kind:'source_draft',segment_id:'next',start_s:5,text:'draft'}));
 assert.deepEqual(state.tracks().history.map(e=>e.text),['source 2','source 3','source 4']);
 assert.equal(state.tracks().activeSource.text,'draft');
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
