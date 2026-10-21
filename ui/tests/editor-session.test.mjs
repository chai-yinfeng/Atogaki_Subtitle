import test from 'node:test';
import assert from 'node:assert/strict';
import { EditorSession, mergeDraggedCueTiming } from '../src/editor-session.ts';
import { readEditorLayout, defaultEditorLayout } from '../src/editor-layout.ts';
const cue = (id, source_text = id, translated_text = '译文') => ({ id, start_ms:0, end_ms:1000, source_text, translated_text, translation_stale:false });
test('playback, filtering and same-task reopen preserve the editing cue', () => {
  const session = new EditorSession(), cues = [cue('a'),cue('b')];
  session.open('job-a',cues); session.selectedCueId='b'; session.playbackCueId='a';
  session.query='a'; assert.deepEqual(session.visible(cues,new Set()).map(c=>c.id),['a']);
  assert.equal(session.selectedCueId,'b');
  session.open('job-a',cues); assert.equal(session.selectedCueId,'b');
  session.open('job-b',[cue('other')]); assert.equal(session.selectedCueId,'other');
  assert.equal(session.query,''); assert.equal(session.playbackCueId,null);
});
test('navigation follows filtered order; revision staleness and unresolved review stay distinct', () => {
  const session = new EditorSession(), cues = [cue('a','first',''),cue('b','second'),cue('c','third')];
  session.open('job',cues); session.filter='untranslated';
  assert.deepEqual(session.visible(cues,new Set()).map(c=>c.id),['a']);
  session.filter='stale'; assert.deepEqual(session.visible(cues,new Set(),['c']).map(c=>c.id),['c']);
  session.filter='review'; const filtered=session.visible(cues,new Set(['b','c']));
  assert.equal(session.adjacent(filtered,1),'b'); session.selectedCueId='b';
  assert.equal(session.adjacent(filtered,1),'c'); assert.equal(session.adjacent(filtered,-1),'b');
  session.reconcile([cues[0],cues[2]]); assert.equal(session.selectedCueId,'a');
  session.reconcile(cues,'c'); assert.equal(session.selectedCueId,'c');
});
test('layout rejects corrupt and future settings, and clamps finite ratios', () => {
  for (const raw of [null,'bad','{"version":2,"video":false}']) assert.deepEqual(readEditorLayout(raw), defaultEditorLayout);
  const saved=readEditorLayout(JSON.stringify({version:1,video:false,waveform:false,videoRatio:999,topRatio:-10,waveformRatio:32}));
  assert.deepEqual(saved,{version:1,video:false,waveform:false,videoRatio:55,topRatio:28,waveformRatio:32});
  assert.equal(readEditorLayout(JSON.stringify({version:1,video:'false',videoRatio:null})).video,true);
});

test('timing gesture preserves text saved during the drag and rejects changed boundaries', () => {
  const initial=cue('a'), other=cue('b'), saved={...initial,source_text:'输入中的新文字',translated_text:'人工译文'};
  const result=mergeDraggedCueTiming([saved,other],initial,{...initial,start_ms:100,end_ms:1100});
  assert.equal(result[0].source_text,'输入中的新文字'); assert.equal(result[0].translated_text,'人工译文');
  assert.equal(result[0].start_ms,100); assert.deepEqual(result[1],other);
  assert.throws(()=>mergeDraggedCueTiming([{...saved,end_ms:900}],initial,{...initial,start_ms:100}),/时间已变化/);
  assert.throws(()=>mergeDraggedCueTiming([],initial,initial),/时间已变化/);
});
