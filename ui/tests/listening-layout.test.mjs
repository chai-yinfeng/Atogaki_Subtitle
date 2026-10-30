import test from 'node:test';
import assert from 'node:assert/strict';
import { readListeningLayout } from '../src/listening-layout.ts';
test('listening layout migration retains transcript position/width and allocates height to stacked video',()=>{
 assert.deepEqual(readListeningLayout(JSON.stringify({version:1,placement:'bottom',right:55,bottom:52})),{version:2,placement:'bottom',right:55,bottom:72,video:85});
 const stored={version:2,placement:'hidden',right:62,bottom:74,video:66};
 assert.deepEqual(readListeningLayout(JSON.stringify(stored)),stored);
});
