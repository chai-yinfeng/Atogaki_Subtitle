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

test('locked MT prefix survives the event ring beside its revisable tail',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'translation_final',group_id:'locked',start_s:0,end_s:2,text:'white prefix'}));
 for(let i=2;i<310;i++)state.accept(event(i,{kind:'translation_draft',group_id:'tail',start_s:2,end_s:5,text:`blue tail ${i}`}));
 const parts=state.tracks().translationParts;
 assert.deepEqual(parts.map(e=>e.kind),['translation_final','translation_draft']);
 assert.equal(parts[0].text,'white prefix');
 assert.equal(parts[1].text,'blue tail 309');
 state.accept(event(310,{kind:'translation_final',group_id:'tail',start_s:2,end_s:5,text:'locked tail'}));
 assert.deepEqual(state.tracks().translationParts.map(e=>e.kind),['translation_final','translation_final']);
});
test('ASR locks independently while MT still revises the same audio range',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'source_final',segment_id:'a',start_s:0,end_s:3,text:'Qwen final'}));
 state.accept(event(2,{kind:'source_draft',segment_id:'b',start_s:3,end_s:5,text:'SenseVoice draft'}));
 state.accept(event(3,{kind:'translation_draft',group_id:'g',start_s:0,end_s:5,text:'still mutable'}));
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.kind),['source_final','source_draft']);
 assert.deepEqual(state.tracks().translationParts.map(e=>e.kind),['translation_draft']);
});
test('reading range advances by audio time at whole-unit boundaries, not a three-item count',()=>{
 const state=new OnlineCaptions();state.snapshot({status:{...status('s'),translationEnabled:false},events:[]});
 for(let i=1;i<=9;i++)state.accept(event(i,{kind:'source_final',segment_id:`s${i}`,start_s:i-1,end_s:i,text:`source ${i}`}));
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.segment_id),['s5','s6','s7','s8','s9']);
 assert.deepEqual(state.tracks().translationParts,[]);
 state.accept(event(10,{kind:'source_draft',segment_id:'next',start_s:9,end_s:16,text:'long draft'}));
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.segment_id),['s9','next']);
 state.accept(event(11,{kind:'source_final',segment_id:'next',start_s:9,end_s:16,text:'Qwen result'}));
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.segment_id),['next']);
});
test('committing a split MT prefix replaces its former whole-group draft',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'translation_draft',group_id:'g1',start_s:0,end_s:5,text:'old whole draft'}));
 state.accept(event(2,{kind:'translation_final',group_id:'g1',start_s:0,end_s:2,text:'prefix only'}));
 state.accept(event(3,{kind:'translation_draft',group_id:'g2',start_s:2,end_s:5,text:'tail only'}));
 assert.deepEqual(state.tracks().translationParts.map(e=>e.text),['prefix only','tail only']);
 state.accept(event(4,{kind:'translation_draft',group_id:'g1',start_s:0,end_s:5,text:'late old draft'}));
 assert.equal(state.tracks().translationParts[0].text,'prefix only');
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

test('splitting a long MT draft never brings expired text back into the reading frame',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 for(let i=1;i<=3;i++)state.accept(event(i,{kind:'translation_final',group_id:`old${i}`,start_s:i-1,end_s:i,text:`old ${i}`}));
 state.accept(event(4,{kind:'translation_draft',group_id:'g',start_s:3,end_s:10,text:'whole draft'}));
 state.accept(event(5,{kind:'translation_final',group_id:'g',start_s:3,end_s:6,text:'locked prefix'}));
 assert.deepEqual(state.tracks().translationParts.map(e=>e.text),['locked prefix']);
});
