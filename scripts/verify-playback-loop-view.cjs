const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  const audio=page.locator('#karaoke-media-host audio');await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1);
  const seek=async seconds=>audio.evaluate(async(a,seconds)=>{a.pause();if(Math.abs(a.currentTime-seconds)<.001)return;await new Promise(resolve=>{a.addEventListener('seeked',resolve,{once:true});a.currentTime=seconds;});},seconds);
  const click=action=>page.locator('#karaoke-view .loop-tools [data-loop="'+action+'"]').click();
  const message=()=>page.locator('#karaoke-view .loop-state').textContent();
  await seek(3.5);await click('sentence');assert.match(await message(),/循环 00:00:03.000 → 00:00:05.800/);
  await page.waitForFunction(()=>{const a=document.querySelector('#karaoke-media-host audio');return !a.seeking&&!a.paused&&a.currentTime>=3&&a.currentTime<3.4;});
  await audio.evaluate(a=>a.currentTime=5.9);await page.waitForFunction(()=>{const a=document.querySelector('#karaoke-media-host audio');return a.currentTime>=3&&a.currentTime<3.4;});
  await click('off');assert.equal(await message(),'循环已关闭');
  await click('b');assert.match(await message(),/请先设置早于 B 的 A 点/);assert.ok((await page.locator('.loop-state').allTextContents()).some(text=>text==='循环已关闭'),'invalid B message stays local');
  await seek(3.5);await click('a');assert.equal(await message(),'A: 00:00:03.500');await seek(4.5);await click('b');assert.equal(await message(),'循环 00:00:03.500 → 00:00:04.500');await click('off');
  await seek(3.5);await click('head');assert.equal(await message(),'试听 00:00:02.500 → 00:00:04.000');await audio.evaluate(a=>a.currentTime=4.1);await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio').paused);
  await seek(3.5);await click('tail');assert.equal(await message(),'试听 00:00:04.800 → 00:00:06.300');await audio.evaluate(a=>a.currentTime=6.4);await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio').paused);
  await click('off');await seek(19);await click('tail');assert.equal(await message(),'试听 00:00:19.800 → 00:00:20.000');
  await page.locator('[data-editor-action="back"]').click();await page.locator('.job-open').first().waitFor({state:'visible'});assert.ok((await page.locator('.loop-state').allTextContents()).every(text=>text==='循环已关闭'));
  await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));assert.equal(await page.locator('.loop-tools').count(),0);
  const result=await page.evaluate(async()=>{
   const {mountPlaybackLoopView}=await import('/src/playback-loop-view.ts');const root=document.createElement('section');root.innerHTML='<div class="playback-tools"></div><div class="playback-tools"></div><div class="playback-tools"></div>';document.body.append(root);const calls=[];
   const action=value=>{calls.push(value);return {message:value==='b'?'local error':'shared '+value,local:value==='b'};};
   let view=mountPlaybackLoopView(root.querySelectorAll('.playback-tools'),action);const groups=root.querySelectorAll('.loop-tools');groups[0].querySelector('[data-loop="sentence"]').click();let labels=[...root.querySelectorAll('.loop-state')].map(e=>e.textContent);if(labels.some(text=>text!=='shared sentence'))throw Error('shared state');groups[1].querySelector('[data-loop="b"]').click();labels=[...root.querySelectorAll('.loop-state')].map(e=>e.textContent);if(JSON.stringify(labels)!==JSON.stringify(['shared sentence','local error','shared sentence']))throw Error('local error');view.reset();if([...root.querySelectorAll('.loop-state')].some(e=>e.textContent!=='循环已关闭'))throw Error('reset');
   const retained=groups[0].querySelector('button'),count=calls.length;view.dispose();view.dispose();retained.click();view.show('disposed');if(calls.length!==count||root.querySelector('.loop-tools'))throw Error('disposed');view=mountPlaybackLoopView(root.querySelectorAll('.playback-tools'),action);if(root.querySelectorAll('.loop-tools').length!==3)throw Error('remount count');root.querySelector('[data-loop="off"]').click();if(calls.length!==count+1)throw Error('double dispatch');view.dispose();root.remove();return true;
  });
  assert.equal(result,true);assert.deepEqual(errors,[]);
  console.log('PASS: sentence repeat, A/B validation, head/tail audition stop and duration clipping, shared/local labels, leave reset and disposable/remountable loop tools');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
