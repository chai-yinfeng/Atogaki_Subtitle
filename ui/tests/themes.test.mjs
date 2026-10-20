import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeTheme, candidateLabel } from '../src/themes.ts';
test('unknown or missing saved themes fall back to the supported moonlight preset', () => {
  assert.equal(normalizeTheme(null),'moonlight');
  assert.equal(normalizeTheme('old-theme'),'moonlight');
  assert.equal(normalizeTheme('letter'),'letter');
});
test('candidate labels distinguish runs and avoid exposing complete model paths', () => {
  const run = {id:'12345678-0000',provider_name:'Whisper.cpp',model_identity:'C:\\Users\\person\\models\\medium.bin',scope_start_ms:40000,scope_end_ms:48000,created_at_unix:1700000000,is_candidate:true};
  const name = candidateLabel(run);
  assert.ok(name.includes('00:40–00:48候选'));
  assert.ok(name.includes('medium'));
  assert.ok(!name.includes('Users'));
  assert.notEqual(name,candidateLabel({...run,id:'87654321-0000'}));
  assert.ok(candidateLabel({...run,is_candidate:false}).startsWith('原始识别'));
});
