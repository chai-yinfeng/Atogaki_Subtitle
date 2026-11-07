import test from 'node:test';
import assert from 'node:assert/strict';
import { DraftPresentation, OnlineCaptions } from '../src/online-state.ts';
const status=id=>({sessionId:id,phase:'running',error:null});
const event=(sequence,body,sessionId='s')=>({sequence,sessionId,event:body});
test('MT range retains matching Qwen text beside only a small live ASR tail',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'source_final',segment_id:'a',start_s:10,end_s:14,text:'原文A'}));
 state.accept(event(2,{kind:'source_final',segment_id:'b',start_s:14,end_s:18,text:'原文B'}));
 state.accept(event(3,{kind:'translation_final',group_id:'g',start_s:10,end_s:18,text:'译文AB',source_revisions:{a:1,b:1}}));
 state.accept(event(4,{kind:'source_final',segment_id:'c',start_s:18,end_s:20,text:'尚未翻译C'}));
 state.accept(event(5,{kind:'source_draft',segment_id:'d',start_s:20,end_s:22,text:'最新草稿D'}));
 assert.deepEqual(state.tracks().translationParts.map(e=>e.text),['译文AB']);
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.text),['原文A','原文B','…','最新草稿D']);
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.kind),['source_final','source_final','source_gap','source_draft']);
 state.accept(event(6,{kind:'translation_draft',group_id:'next',start_s:18,end_s:22,text:'译文CD',source_revisions:{c:1,d:1}}));
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.text),['原文A','原文B','尚未翻译C','最新草稿D']);
});

test('moving the paired reading range evicts old original and translation together',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'source_final',segment_id:'a',start_s:0,end_s:4,text:'old original'}));
 state.accept(event(2,{kind:'translation_draft',group_id:'old',start_s:0,end_s:4,text:'old draft',source_revisions:{a:1}}));
 state.accept(event(3,{kind:'source_final',segment_id:'b',start_s:20,end_s:26,text:'new original'}));
 state.accept(event(4,{kind:'translation_draft',group_id:'new',start_s:20,end_s:26,text:'new draft',source_revisions:{b:1}}));
 assert.deepEqual(state.tracks().translationParts.map(e=>e.text),['new draft']);
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.text),['new original']);
});

test('paired sources survive ASR map eviction and do not turn blue while MT waits',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'source_final',segment_id:'a',start_s:0,end_s:4,text:'paired source'}));
 state.accept(event(2,{kind:'translation_draft',group_id:'g',start_s:0,end_s:4,text:'translation',source_revisions:{a:1}}));
 for(let i=1;i<=70;i++)state.accept(event(i+2,{kind:'source_final',segment_id:`s${i}`,start_s:i+4,end_s:i+5,text:`source ${i}`}));
 assert.equal(state.tracks().sourceParts[0].text,'paired source');
 assert.equal(state.tracks().sourceParts[0].kind,'source_final');
 assert.ok(state.tracks().sourceParts.length<=4);
 state.accept(event(73,{kind:'translation_final',group_id:'next',start_s:73,end_s:75,text:'latest translation',source_revisions:{s69:1,s70:1}}));
 assert.deepEqual(state.tracks().translationParts.map(e=>e.text),['latest translation']);
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.text),['source 69','source 70']);
});

test('new window can recover the paired original from an actual MT input snapshot',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[event(300,{
  kind:'translation_final',group_id:'g',start_s:10,end_s:18,text:'译文',source_text:'对应的Qwen原文',source_revisions:{a:2,b:2}})]});
 assert.equal(state.tracks().sourceParts[0].text,'对应的Qwen原文');
 assert.equal(state.tracks().sourceParts[0].kind,'source_final');
});

test('source pairing follows the MT snapshot actually shown by the prefix gate',()=>{
 const state=new OnlineCaptions();state.snapshot({status:status('s'),events:[]});
 state.accept(event(1,{kind:'source_final',segment_id:'a',start_s:0,end_s:3,text:'A'}));
 const shown={kind:'translation_draft',group_id:'g',start_s:0,end_s:3,text:'old translation',source_revisions:{a:1}};
 state.accept(event(2,shown));
 state.accept(event(3,{kind:'source_final',segment_id:'b',start_s:3,end_s:6,text:'B'}));
 state.accept(event(4,{kind:'source_draft',segment_id:'c',start_s:6,end_s:8,text:'C'}));
 state.accept(event(5,{...shown,end_s:8,text:'new translation',source_revisions:{a:1,b:1,c:1}}));
 assert.deepEqual(state.tracks([shown]).sourceParts.map(e=>e.text),['A','…','C']);
 assert.deepEqual(state.tracks().sourceParts.map(e=>e.text),['A','B','C']);
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
