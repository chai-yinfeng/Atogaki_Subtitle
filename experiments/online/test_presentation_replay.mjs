import {test} from 'node:test';
import assert from 'node:assert/strict';
import {replay} from './replay_presentation.mjs';

const source = (time,text,kind='source_draft') => ({kind,emission_s:time,segment_id:'s',start_s:0,end_s:time,text});
test('production gate delays prefix rewrite but not final; records skipped versions', () => {
  const result = replay([source(0,'甲'),source(.2,'乙'),source(.3,'丙'),source(2.3,'丁','source_final')]);
  assert.equal(result.versions[1].first_same_text_shown_s,null);
  assert.equal(result.versions[2].first_same_text_shown_s,2);
  assert.equal(result.versions[3].extra_show_wait_s,0);
});
test('append is immediate and no transport or paint latency is invented', () => {
  const result = replay([source(0,'甲'),source(.2,'甲乙')]);
  assert.equal(result.versions[1].extra_show_wait_s,0);
});

test('invalid clocks fail rather than silently replaying backwards', () => {
  assert.throws(()=>replay([source(1,'甲'),source(0,'乙')]));
  assert.throws(()=>replay([],0));
});
