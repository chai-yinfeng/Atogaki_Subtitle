import test from 'node:test';
import assert from 'node:assert/strict';
import { cueWindow, cueRowHeight } from '../src/editor-virtual-window.ts';
test('mounted rows depend on viewport rather than program length', () => {
  for(const count of [300,3000,10000]) for(const top of [0,1000,count*cueRowHeight]) {
    const {start,end}=cueWindow(count,top,252);
    assert.ok(start>=0 && end<=count && end-start<=Math.ceil(252/cueRowHeight)+17);
    assert.ok(start<end);
  }
});
test('late-hour navigation, viewport resize and empty filtering clamp safely', () => {
  assert.deepEqual(cueWindow(0,9999,300),{start:0,end:0});
  assert.equal(cueWindow(10000,10000*cueRowHeight,200).end,10000);
  assert.deepEqual(cueWindow(1,9999,500),{start:0,end:1});
  assert.equal(cueWindow(10000,-20,200).start,0);
  const before=cueWindow(10000,100*cueRowHeight,200),after=cueWindow(10000,100*cueRowHeight,500);
  assert.equal(before.start,after.start);assert.ok(after.end>before.end);
});
