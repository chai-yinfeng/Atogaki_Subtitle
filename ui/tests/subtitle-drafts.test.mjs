import test from 'node:test';
import assert from 'node:assert/strict';
import { SubtitleDrafts, cueKey } from '../src/subtitle-drafts.ts';
const cue = (id, job='job-a') => ({job_id:job,id,source_text:'原文',translated_text:'译文'});
const pending = () => { let resolve, reject; const promise = new Promise((yes,no)=>{resolve=yes;reject=no;}); return {promise,resolve,reject}; };
test('saving one cue retains another draft and serializes typing during a write', async () => {
  const calls=[], waits=[];
  const store=new SubtitleDrafts(async (before,value)=>{calls.push([before.id,{...value}]);const wait=pending();waits.push(wait);await wait.promise;return {...before,source_text:value.source,translated_text:value.translation};},()=>{},60000);
  const a=cue('a'),b=cue('b');
  store.edit(a,{source:'A1',translation:'译文'});store.edit(b,{source:'B1',translation:'译文'});
  const flight=store.flush(cueKey(a));store.edit(a,{source:'A2',translation:'新译'});
  assert.equal(store.ensure(b).value.source,'B1');
  waits[0].resolve(); await new Promise(resolve=>setImmediate(resolve));
  assert.equal(calls.length,2); assert.equal(calls[1][1].source,'A2');
  waits[1].resolve();await flight;
  assert.equal(store.entries.get(cueKey(a)).saved.source_text,'A2');
  assert.equal(store.ensure(b).value.source,'B1');store.discard(b);
});
test('failure retains text and explicit retry succeeds without parallel writes', async () => {
  let fail=true;
  const store=new SubtitleDrafts(async(before,value)=>{if(fail)throw new Error('disk full');return {...before,source_text:value.source};},()=>{},60000);
  const a=cue('a');store.edit(a,{source:'保留草稿',translation:'译文'});
  await assert.rejects(store.flush(cueKey(a)),/disk full/);
  assert.equal(store.ensure(a).value.source,'保留草稿');assert.ok(store.ensure(a).error.includes('disk full'));
  fail=false;await Promise.all([store.flush(cueKey(a)),store.flush(cueKey(a))]);
  assert.equal(store.entries.get(cueKey(a)).saved.source_text,'保留草稿');
});
test('restart recovery is task scoped and preserves conflicting drafts for review', async () => {
  const writes=[];
  const source=new SubtitleDrafts(async()=>{},()=>{},60000);
  const a=cue('a');source.edit(a,{source:'恢复草稿',translation:'译文'});
  const snapshot=source.snapshot();source.discard(a);
  const restored=new SubtitleDrafts(async(before,value)=>{writes.push(value);return {...before,source_text:value.source};},()=>{},60000);
  restored.recover(snapshot);await restored.flushAll();assert.equal(writes.length,0);
  const conflict=restored.ensure({...a,source_text:'别处修改'});
  assert.ok(conflict.error);assert.equal(conflict.value.source,'恢复草稿');assert.equal(writes.length,0);
  assert.equal(restored.ensure(cue('a','job-b')).value.source,'原文');
  await assert.rejects(restored.flushAll(),/字幕已变化/);assert.equal(writes.length,0);restored.discard(a);
});
test('IME composition does not commit partial input and clean refresh accepts an undo', async () => {
  const writes=[];
  const store=new SubtitleDrafts(async(before,value)=>{writes.push(value.source);return {...before,source_text:value.source};},()=>{},60000);
  const a=cue('a');store.edit(a,{source:'に',translation:'译文'},true);
  await store.flush(cueKey(a));assert.equal(writes.length,0);
  store.edit(a,{source:'日本語',translation:'译文'},false);await store.flush(cueKey(a));
  assert.deepEqual(writes,['日本語']);assert.equal(store.ensure(a).value.source,'原文');
});

test('automatic debounce commits independent cues without manual save', async () => {
  const writes=[];
  const store=new SubtitleDrafts(async(before,value)=>{writes.push(before.id);return {...before,source_text:value.source};},()=>{},5);
  store.edit(cue('a'),{source:'A',translation:'译文'});
  store.edit(cue('b'),{source:'B',translation:'译文'});
  await new Promise(resolve=>setTimeout(resolve,30));
  assert.deepEqual(writes.sort(),['a','b']);
  assert.equal([...store.entries.values()].some(draft=>store.pending(draft)),false);
});
