import test from 'node:test';
import assert from 'node:assert/strict';
import { updateTimingPreview } from '../src/timeline-timing.ts';
const cues=[{id:'a',start_ms:0,end_ms:900,text:'a'},{id:'b',start_ms:1000,end_ms:2000,text:'keep edited text'},{id:'c',start_ms:2100,end_ms:3000,text:'c'}];
const drag=mode=>({jobId:'job',pointerId:1,segmentIndex:1,mode,pointerStartX:100,width:300,rangeMs:3000,before:cues.map(c=>({...c})),draft:cues.map(c=>({...c})),moved:false});
test('whole-cue preview snaps to 10 ms, preserves duration/text and clamps to neighboring cues',()=>{
 const d=drag('move');updateTimingPreview(d,104.2,3000);
 assert.equal(d.draft[1].start_ms,1040);assert.equal(d.draft[1].end_ms,2040);
 updateTimingPreview(d,400,3000);assert.equal(d.draft[1].end_ms,2100);
 updateTimingPreview(d,-400,3000);assert.equal(d.draft[1].start_ms,900);assert.equal(d.draft[1].end_ms,1900);
 assert.equal(d.draft[1].text,'keep edited text');assert.deepEqual(d.before,cues);
 assert.deepEqual(d.draft[0],cues[0]);assert.deepEqual(d.draft[2],cues[2]);
 updateTimingPreview(d,100,3000);assert.equal(d.moved,false);assert.deepEqual(d.draft,cues);
});
test('trim preview keeps at least 10 ms and cannot cross neighbors or media end',()=>{
 const start=drag('start');updateTimingPreview(start,500,3000);assert.equal(start.draft[1].start_ms,1990);
 updateTimingPreview(start,-500,3000);assert.equal(start.draft[1].start_ms,900);assert.equal(start.draft[1].end_ms,2000);
 const end=drag('end');updateTimingPreview(end,-500,3000);assert.equal(end.draft[1].end_ms,1010);
 updateTimingPreview(end,500,3000);assert.equal(end.draft[1].end_ms,2100);
 const last={...drag('end'),segmentIndex:2};updateTimingPreview(last,500,3500);assert.equal(last.draft[2].end_ms,3500);
});
