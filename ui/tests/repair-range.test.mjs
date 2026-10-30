import test from 'node:test';
import assert from 'node:assert/strict';
import { alignRepairRange } from '../src/repair-range.ts';
const cues = [{ start_ms: 10000, end_ms: 15000 }, { start_ms: 20000, end_ms: 25000 }];
test('outward alignment uses nearest boundaries in gaps and complete cue edges inside cues', () => {
 assert.deepEqual(alignRepairRange(cues, {startMs:17000,endMs:23000},30000),{startMs:15000,endMs:25000});
 assert.deepEqual(alignRepairRange(cues, {startMs:17000,endMs:18000},30000),{startMs:15000,endMs:20000});
 assert.deepEqual(alignRepairRange(cues, {startMs:11000,endMs:23000},30000),{startMs:10000,endMs:25000});
 assert.deepEqual(alignRepairRange(cues, {startMs:15000,endMs:20000},30000),{startMs:15000,endMs:20000});
});
test('outward alignment reaches media ends when no further cue boundary exists', () => {
 assert.deepEqual(alignRepairRange(cues, {startMs:2000,endMs:28000},30000),{startMs:0,endMs:30000});
 assert.deepEqual(alignRepairRange([], {startMs:2000,endMs:28000},30000),{startMs:0,endMs:30000});
});
