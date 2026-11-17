import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeTheme, candidateLabel, readTheme, applyTheme } from '../src/themes.ts';
test('unknown or missing saved themes fall back to light', () => {
  assert.equal(normalizeTheme(null),'light');
  assert.equal(normalizeTheme('old-theme'),'light');
  assert.equal(normalizeTheme('letter'),'light');
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

test('new shell selects light once and preserves recoverable legacy theme settings',()=>{
 const previous=Object.getOwnPropertyDescriptor(globalThis,'localStorage');
 const image=JSON.stringify({version:1,name:'saved.png',enabled:true});
 const saved=new Map([['atogaki.appearance.theme','letter'],['atogaki.appearance.imageTheme',image]]);
 Object.defineProperty(globalThis,'localStorage',{configurable:true,value:{getItem:key=>saved.get(key)??null,setItem:(key,value)=>saved.set(key,value)}});
 try {
  assert.equal(readTheme(),'light');
  assert.deepEqual(JSON.parse(saved.get('atogaki.appearance.beforeShellV2')),{theme:'letter',image});
  assert.equal(JSON.parse(saved.get('atogaki.appearance.imageTheme')).enabled,false);
  saved.set('atogaki.appearance.theme','moonlight');
  assert.equal(readTheme(),'light');
  assert.equal(JSON.parse(saved.get('atogaki.appearance.beforeShellV2')).theme,'letter');
  assert.equal(JSON.parse(saved.get('atogaki.appearance.beforeThemeV3')).theme,'moonlight');
  saved.set('atogaki.appearance.theme','system');assert.equal(readTheme(),'system');
 }finally{if(previous)Object.defineProperty(globalThis,'localStorage',previous);else delete globalThis.localStorage;}
});
