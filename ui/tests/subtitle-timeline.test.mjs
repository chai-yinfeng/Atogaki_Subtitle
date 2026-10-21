import test from 'node:test';
import assert from 'node:assert/strict';
import { SubtitleTimeline } from '../src/subtitle-timeline.ts';

test('playback preserves first-match semantics for overlaps, gaps, ties and exclusive ends', () => {
  const cues = [
    { id:'a', start_ms:0, end_ms:10000 },
    { id:'b', start_ms:2000, end_ms:3000 },
    { id:'c', start_ms:11000, end_ms:12000 },
    { id:'d', start_ms:11000, end_ms:14000 },
  ];
  const index = new SubtitleTimeline(cues);
  for (const time of [-1,0,2000,3000,9999,10000,10500,11000,11999,12000,14000]) {
    assert.equal(index.at(time), cues.find(c => c.start_ms <= time && c.end_ms > time));
  }
  assert.deepEqual(index.visible(2500,11500), cues);
  assert.equal(index.get('b'), cues[1]);
  assert.equal(index.position('missing'), -1);
});

test('late-hour playback and visible-window queries do not scan the whole program', () => {
  let reads = 0;
  const cues = Array.from({length:10000}, (_, i) => ({
    id:String(i), get start_ms() { reads++; return i*2000; }, get end_ms() { reads++; return i*2000+1500; },
  }));
  const index = new SubtitleTimeline(cues);
  reads = 0;
  assert.equal(index.at(18000010)?.id, '9000');
  assert.deepEqual(index.visible(18000000,18010000).map(c => c.id), ['9000','9001','9002','9003','9004','9005']);
  assert.ok(reads < 60, `bounded reads, got ${reads}`);
});


test('text replacement updates playback and visible lookups without changing timing order', () => {
  const cues = [{id:'a',start_ms:0,end_ms:1000,text:'old'},{id:'b',start_ms:2000,end_ms:3000,text:'later'}];
  const index = new SubtitleTimeline(cues);
  const edited = {...cues[0],text:'new'};
  assert.equal(index.replaceText(edited),true);
  assert.equal(index.get('a'),edited);
  assert.equal(index.at(500),edited);
  assert.equal(index.visible(0,500)[0],edited);
  assert.equal(index.position('b'),1);
  assert.equal(index.replaceText({...edited,end_ms:2500}),false);
  assert.equal(index.replaceText({...edited,id:'missing'}),false);
  assert.equal(index.at(1500),undefined);
});
