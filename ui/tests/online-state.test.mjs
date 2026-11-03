import test from 'node:test';
import assert from 'node:assert/strict';
import { OnlineCaptions } from '../src/online-state.ts';
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
