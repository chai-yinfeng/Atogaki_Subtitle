const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE),assert=require('node:assert/strict');
const {installProcessingFixture}=require('./editor-processing-fixture.cjs');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  await installProcessingFixture(page);await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  const open=async index=>{await page.locator('.job-open').nth(index).click();await page.locator('#karaoke-media-host audio').waitFor();await require('./load-editor-test-media.cjs')(page);await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1,{},{polling:50,timeout:10000})};
  const back=()=>page.locator('[data-editor-action="back"]').click();
  const seek=async time=>{await page.locator('#karaoke-media-host audio').evaluate((a,time)=>{a.pause();a.currentTime=time},time);await page.waitForFunction(time=>Math.abs(document.querySelector('#karaoke-media-host audio').currentTime-time)<.1,time,{polling:50,timeout:10000})};
  await open(0);await seek(12);await back();await open(1);
  assert.equal(await page.locator('#karaoke-media-host audio').evaluate(a=>a.currentTime),0,'a new task must not inherit the previous task position');
  assert.equal(await page.locator('#karaoke-current-source').inputValue(),'任务 B 第 1 段');
  await seek(8);await back();await open(1);await page.waitForFunction(()=>Math.abs(document.querySelector('#karaoke-media-host audio').currentTime-8)<.1,{},{polling:50,timeout:10000});
  assert.equal(await page.locator('#karaoke-current-source').inputValue(),'任务 B 第 3 段');
  await back();await open(0);assert.equal(await page.locator('#karaoke-media-host audio').evaluate(a=>a.currentTime),0);
  assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第 1 句：今日は音楽と日常について話します。');
  assert.deepEqual(errors,[]);console.log('PASS: cross-task playback position isolation and same-task paused resume');
 } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
