import test from 'node:test';
import assert from 'node:assert/strict';
import { suggestSubtitleInsertionRange as suggest } from '../src/subtitle-insertion-range.ts';
const cues=Object.freeze([Object.freeze({start_ms:0,end_ms:1000}),Object.freeze({start_ms:3000,end_ms:4000}),Object.freeze({start_ms:4000,end_ms:5000})]);
test('playing cue suggests its following gap, clipped to the next cue',()=>{
 assert.deepEqual(suggest(cues,500,10000),{startMs:1000,endMs:3000,durationMs:10000});
});
test('gap and empty timelines start at the snapped playhead and suggest at most two seconds',()=>{
 assert.deepEqual(suggest(cues,1506,10000),{startMs:1510,endMs:3000,durationMs:10000});
 assert.deepEqual(suggest([],1506,10000),{startMs:1510,endMs:3510,durationMs:10000});
});
test('adjacent cues present an empty range instead of silently skipping to a later gap',()=>{
 assert.deepEqual(suggest(cues,3500,10000),{startMs:4000,endMs:4000,durationMs:10000});
});
test('cue boundaries and media end stay within the media even when ten-ms snapping rounds past it',()=>{
 assert.deepEqual(suggest(cues,1000,10000),{startMs:1000,endMs:3000,durationMs:10000});
 assert.deepEqual(suggest([],10003,10004),{startMs:10000,endMs:10004,durationMs:10004});
 assert.deepEqual(suggest([],10006,10007),{startMs:10007,endMs:10007,durationMs:10007});
 assert.deepEqual(suggest(cues,4500,4997),{startMs:4997,endMs:4997,durationMs:4997});
});
test('unknown or invalid media time cannot produce a suggestion',()=>{
 for(const duration of [0,-1,NaN,Infinity])assert.equal(suggest(cues,0,duration),null);
 for(const position of [-1,NaN,Infinity])assert.equal(suggest(cues,position,10000),null);
});
