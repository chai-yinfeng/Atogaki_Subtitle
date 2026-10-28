const {test}=require('node:test'),assert=require('node:assert/strict');
const {parseProcesses,newWebKitProcesses,sampleIdentities}=require('./webkit-process-attribution.cjs');
test('association excludes preexisting WebKit and preserves start identity and command spaces',()=>{
 const rows=parseProcesses('10 1 7000 Wed Oct 28 12:00:00 2026 /System/Library/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent\n11 10 90 Wed Oct 28 12:00:01 2026 /tmp/App With Spaces/Browser\n12 1 8000 Wed Oct 28 12:00:02 2026 /System/Library/com.apple.WebKit.GPU.xpc/Contents/MacOS/com.apple.WebKit.GPU');
 assert.equal(rows[1].command,'/tmp/App With Spaces/Browser');assert.equal(rows[0].startedAt,'Wed Oct 28 12:00:00 2026');
 assert.deepEqual(newWebKitProcesses(rows.slice(0,1),rows),[rows[2]]);
});
test('same-name PID reuse and missing renderer reject the sample',()=>{
 const old={pid:12,command:'/System/com.apple.WebKit.GPU',startedAt:'Wed Oct 28 12:00:00 2026',rssKiB:10};
 assert.throws(()=>sampleIdentities([old],[]),/exited or changed identity/);
 assert.throws(()=>sampleIdentities([old],[{...old,startedAt:'Wed Oct 28 12:00:01 2026'}]),/exited or changed identity/);
 assert.deepEqual(sampleIdentities([old],[{...old,rssKiB:20}]),[{...old,rssKiB:20}]);
 assert.deepEqual(newWebKitProcesses([old],[{...old,startedAt:'Wed Oct 28 12:00:01 2026'}]),[{...old,startedAt:'Wed Oct 28 12:00:01 2026'}]);
});
