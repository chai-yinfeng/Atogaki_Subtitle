import test from 'node:test';
import assert from 'node:assert/strict';
import { timelineGeometry, timelineCueHit } from '../src/editor-timeline-view.ts';
test('compact and expanded waveforms keep the cue track inside the canvas and below audio',()=>{
 for(const height of [64,75,99,100,160,260,640]) {
  const g=timelineGeometry(height);
  assert.ok(g.waveformTop>=20);
  assert.ok(g.waveformHeight>=12);
  assert.ok(g.trackTop>g.waveformTop+g.waveformHeight);
  assert.ok(g.trackHeight>=14);
  assert.ok(g.trackTop+g.trackHeight<=height);
 }
});

test('adjacent short-gap handles choose the nearest boundary and preserve body drags',()=>{
 const cues=[{start_ms:0,end_ms:2800},{start_ms:3000,end_ms:5800}];
 const hit=timelineCueHit(cues,44,500,0,30000);
 assert.equal(hit.segmentIndex,0);assert.equal(hit.mode,'end');
 assert.equal(timelineCueHit(cues,52,500,0,30000).segmentIndex,1);
 assert.equal(timelineCueHit(cues,23,500,0,30000).mode,'move');
 assert.equal(timelineCueHit(cues,200,500,0,30000),null);
 assert.equal(timelineCueHit(cues,0,0,0,30000),null);
});
