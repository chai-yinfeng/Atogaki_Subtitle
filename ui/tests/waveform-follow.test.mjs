import test from 'node:test';
import assert from 'node:assert/strict';
import { waveformFollowAnchor } from '../src/waveform-follow.ts';
const windowAt=(start,end,duration=120000)=>({start_ms:start,end_ms:end,duration_ms:duration,peaks:[]});
test('waveform edges do not reload a viewport clamped at either media boundary',()=>{
 for(const ms of [0,1,100,3599])assert.equal(waveformFollowAnchor(ms,windowAt(0,30000),30000),null);
 for(const ms of [116401,119999,120000])assert.equal(waveformFollowAnchor(ms,windowAt(90000,120000),30000),null);
 for(const ms of [0,500,19500,20000])assert.equal(waveformFollowAnchor(ms,windowAt(0,20000,20000),30000),null);
});
test('waveform follows movable edges and jumps outside viewport, leaving interior unchanged',()=>{
 const waveform=windowAt(30000,60000);
 assert.equal(waveformFollowAnchor(30000,waveform,30000),30000);
 assert.equal(waveformFollowAnchor(59999,waveform,30000),59999);
 assert.equal(waveformFollowAnchor(0,waveform,30000),0);
 assert.equal(waveformFollowAnchor(120000,waveform,30000),120000);
 for(const ms of [33600,45000,56400])assert.equal(waveformFollowAnchor(ms,waveform,30000),null);
});
