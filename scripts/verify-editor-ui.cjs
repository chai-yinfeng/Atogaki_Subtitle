const { chromium } = require(process.env.ATOGAKI_PLAYWRIGHT_MODULE || 'playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const output = process.env.ATOGAKI_EDITOR_QA_OUTPUT || '/tmp/atogaki-editor-ui';
fs.mkdirSync(output, {recursive:true});
(async()=>{
const browser=await chromium.launch({...(process.env.ATOGAKI_CHROME ? {executablePath:process.env.ATOGAKI_CHROME} : {channel:'chrome'}),headless:true});
const context=await browser.newContext({viewport:{width:1180,height:760}});
const page=await context.newPage();
// Generated silence tests playback without user media, models or network providers.
const wav=Buffer.alloc(44+16000*20);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(8000,24);wav.writeUInt32LE(16000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
const errors=[];page.on('pageerror',e=>errors.push(String(e)));
await page.addInitScript(audioUrl=>{
  window.__qaAudio=audioUrl;
  const cues=Array.from({length:300},(_,i)=>({id:`cue-${i}`,job_id:'editor-test',segment_index:i,start_ms:i*3000,end_ms:i*3000+2800,source_text:`第 ${i+1} 句：今日は音楽と日常について話します。`,translated_text:i%3?'今天聊聊音乐与日常。':null,translation_stale:i%7===0,source_edited:false,translation_edited:false,timing_edited:false}));
  const job={job_id:'editor-test',display_name:'编辑器隔离测试 · 音乐与日常',input_path:'/test/program.wav',storage_dir:'/test',status:'done',message:'完成',source_language:'ja',target_language:'zh-Hans',created_at_unix:1,updated_at_unix:1,segment_count:cues.length,translated_segment_count:200,stale_translation_count:43,translation_status:'stale'};
  window.__test={cues,job,calls:[],fail:false,delay:0};
  window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback:()=>{},convertFileSrc:path=>path==='/fixture.wav'?window.__qaAudio:path,invoke:async(cmd,args={})=>{
    window.__test.calls.push({cmd,args});
    if(cmd==='get_job_detail')return {job,segments:cues,translation_runs:[],translation_context_stale_ids:[],playback_path:'/fixture.wav',audio_fallback_path:null};
    if(cmd==='list_jobs')return [job];
    if(cmd==='desktop_settings')return {translationProviderId:'none',translationStyleInstruction:'',modelsDirectory:'/test/models',networkProxyMode:'environment',credentialStore:'test',needsOnboarding:false};
    if(cmd==='translation_status')return {provider_id:'none',provider:'测试翻译',configured:false,model:null,endpoint_kind:'none',configuration_hint:'测试'};
    if(cmd==='get_waveform_window'){const {startMs,endMs,pointCount}=args.request;return {start_ms:startMs,end_ms:endMs,duration_ms:900000,peaks:Array.from({length:pointCount},(_,i)=>({min:-Math.abs(Math.sin(i/8))*.7,max:Math.abs(Math.sin(i/8))*.7}))};}
    if(cmd==='list_asr_quality_signals')return [{id:'signal',signal_kind:'boundary_repetition',start_ms:3000,end_ms:5800,severity:'warning',message:'测试提示',latest_decision:null}];
    if(cmd==='update_subtitle') { await new Promise(resolve=>setTimeout(resolve,window.__test.delay)); if(window.__test.fail)throw Error('disk full'); const r=args.request,c=cues.find(c=>c.id===r.segmentId);Object.assign(c,{source_text:r.sourceText,translated_text:r.translatedText,start_ms:r.startMs,end_ms:r.endMs,source_edited:true});return {...c}; }
    if(cmd==='save_subtitle_timing'){cues.splice(0,cues.length,...args.request.afterSegments);return cues;}
    if(cmd==='data_directory')return '/test/isolated';
    if(cmd==='theme_artwork_settings')return null;
    if(cmd.startsWith('plugin:'))return 1;
    if(['translation_profiles','model_catalog','model_download_states','dictionary_catalog','dictionary_download_states','dictionary_credential_statuses','list_glossaries','task_presets','list_video_renders','list_asr_runs'].includes(cmd))return [];
    return [];
  }};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
}, 'data:audio/wav;base64,'+wav.toString('base64'));
await page.goto(process.env.ATOGAKI_EDITOR_URL || 'http://127.0.0.1:1420');await page.locator('.job-open').first().click();
await page.locator('#editor-cue-list .editor-cue-row').first().waitFor();
await page.screenshot({path:path.join(output,'editor-1180.png')});
console.log('startup errors',errors);
console.log('layout',await page.evaluate(()=>({viewport:[innerWidth,innerHeight],height:document.documentElement.scrollHeight,panels:[...document.querySelectorAll('.editor-top,#editor-wave-panel,.editor-list-panel')].map(e=>({class:e.className,rect:{h:e.clientHeight,w:e.clientWidth}})),rows:document.querySelectorAll('.editor-cue-row').length})));
assert.equal(await page.locator('#karaoke-current-source').inputValue(), '第 1 句：今日は音楽と日常について話します。');
await page.locator('[data-cue-id="cue-0"]').focus();await page.keyboard.press('ArrowDown');
assert.match(await page.locator('[data-cue-id="cue-1"]').getAttribute('class'),/selected/);
await page.keyboard.press('ArrowUp');
assert.match(await page.locator('[data-cue-id="cue-0"]').getAttribute('class'),/selected/);
await page.locator('#karaoke-current-source').fill('第一段修改');
await page.locator('[data-cue-id="cue-1"]').click();
await page.locator('#karaoke-current-source').fill('第二段修改');
await page.waitForTimeout(800);
assert.equal(await page.evaluate(()=>window.__test.cues[0].source_text),'第一段修改');
assert.equal(await page.evaluate(()=>window.__test.cues[1].source_text),'第二段修改');
await page.locator('#editor-search').fill('第一段修改');
assert.equal(await page.locator('.editor-cue-row').count(),1);
assert.match(await page.locator('#editor-selection-message').innerText(),/不在筛选/);
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第二段修改');
await page.locator('#editor-search').fill('');
await page.locator('#editor-filter').selectOption('review');assert.equal(await page.locator('.editor-cue-row').count(),1);
await page.locator('#editor-filter').selectOption('all');
await page.locator('[data-editor-toggle="video"]').click();assert.equal(await page.locator('#editor-video-panel').getAttribute('aria-hidden'),null);
await page.locator('[data-editor-toggle="waveform"]').click();assert.equal(await page.locator('#editor-wave-panel').isVisible(),false);
await page.locator('#karaoke-open-workspace').click();await page.locator('.editor-task-tools').waitFor();
await page.locator('#back-to-jobs').click();assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第二段修改');
await page.locator('[data-editor-reset]').click();
await page.setViewportSize({width:960,height:640});await page.screenshot({path:path.join(output,'editor-960.png')});
assert.ok(await page.evaluate(()=>document.documentElement.scrollHeight<=innerHeight));
console.log('small layout',await page.evaluate(()=>({height:document.documentElement.scrollHeight,view:document.querySelector('#karaoke-view').getBoundingClientRect().toJSON(),listHeight:document.querySelector('#editor-cue-list').clientHeight})));
// Playback must never replace the selected editor, and folding cannot remount media.
await page.locator('#karaoke-media-host audio').evaluate(async audio=>{ if(audio.readyState<1)await new Promise(resolve=>audio.addEventListener('loadedmetadata',resolve,{once:true}));await new Promise(resolve=>{audio.addEventListener('seeked',resolve,{once:true});audio.currentTime=7;}); audio.dispatchEvent(new Event('timeupdate')); });
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'第二段修改');
console.log('playback', await page.evaluate(()=>({time:document.querySelector('#karaoke-media-host audio').currentTime,clock:document.querySelector('#karaoke-current-time').textContent,playing:[...document.querySelectorAll('.playing')].map(e=>e.dataset.cueId),errors:document.querySelector('#karaoke-timing-message').textContent})));
assert.match(await page.locator('[data-cue-id="cue-2"]').getAttribute('class'),/playing/);
const beforePosition=await page.locator('#karaoke-media-host audio').evaluate(audio=>{window.__qaMedia=audio; return audio.currentTime;});
await page.locator('[data-editor-toggle="video"]').click();
assert.ok(await page.locator('#karaoke-media-host audio').evaluate(audio=>audio===window.__qaMedia));
assert.equal(await page.locator('#karaoke-media-host audio').evaluate(audio=>audio.currentTime),beforePosition);
await page.locator('[data-editor-toggle="video"]').click();
await page.locator('[data-editor-resize="videoRatio"]').focus();await page.keyboard.press('ArrowRight');
assert.equal(await page.locator('[data-editor-resize="videoRatio"]').getAttribute('aria-valuenow'),'36');
// Explicit time save preserves typed values while playback advances.
await page.locator('#editor-cue-start').fill('00:00:03.100');
await page.locator('#editor-cue-end').fill('00:00:05.700');
await page.locator('[data-editor-action="save-time"]').click();
assert.equal(await page.evaluate(()=>window.__test.cues[1].start_ms),3100);
// Large text remains contained; panels provide independent scrolling.
await page.evaluate(()=>document.querySelector('#karaoke-view').style.fontSize='20px');
assert.ok(await page.evaluate(()=>document.documentElement.scrollHeight<=innerHeight));
// IME composition keeps its cue selected until compositionend.
await page.locator('#karaoke-current-source').evaluate(input=>{input.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));input.value='に';input.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true}));});
await page.locator('[data-cue-id="cue-2"]').click();
assert.match(await page.locator('[data-cue-id="cue-1"]').getAttribute('class'),/selected/);
await page.locator('#karaoke-current-source').evaluate(input=>{input.value='日本語確定';input.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}));});
await page.waitForTimeout(750);
assert.equal(await page.evaluate(()=>window.__test.cues[1].source_text),'日本語確定');
await page.evaluate(()=>window.__test.fail=true);await page.locator('#karaoke-current-source').fill('失败草稿');await page.waitForTimeout(800);
assert.equal(await page.locator('#karaoke-current-source').inputValue(),'失败草稿');assert.match(await page.locator('#karaoke-timing-message').innerText(),/disk full/);
await page.locator('[data-cue-id="cue-2"]').click();await page.locator('[data-cue-id="cue-1"]').click();assert.equal(await page.locator('#karaoke-current-source').inputValue(),'失败草稿');
await page.evaluate(()=>window.__test.fail=false);await page.locator('#karaoke-save-text').click();await page.waitForTimeout(100);
assert.equal(await page.evaluate(()=>window.__test.cues[1].source_text),'失败草稿');
// Reload validates cross-session layout preferences and restores the same panel sizes.
await page.reload();await page.locator('.job-open').first().click();
await page.locator('#editor-cue-list .editor-cue-row').first().waitFor();
assert.equal(await page.locator('[data-editor-resize="videoRatio"]').getAttribute('aria-valuenow'),'36');
assert.equal(await page.locator('#subtitle-list textarea').count(),0);
assert.deepEqual(errors,[]);console.log('PASS: selection, independent drafts, filters, tool return, folding, playback, time save, IME, failure retry, layout persistence');
await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});
