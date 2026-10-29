// Paired production-UI diagnostics with synthetic IPC. Never an actual Tauri TTI claim.
const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {execFileSync}=require('node:child_process');
const output=process.env.ATOGAKI_EDITOR_QA_OUTPUT||'/tmp/atogaki-editor-version-pair';
const counts=(process.env.ATOGAKI_PAIR_COUNTS||'300,3000,10000').split(',').map(Number);
assert.ok(counts.every(n=>Number.isInteger(n)&&n>=30&&n<=10000));
const baselineRoot=process.env.ATOGAKI_BASELINE_ROOT;assert.ok(baselineRoot,'Set ATOGAKI_BASELINE_ROOT to the historical production checkout');
fs.mkdirSync(output,{recursive:true});assert.ok(!fs.existsSync(path.join(output,'results.json')),'refuse to overwrite evidence');
const versions=[{name:'baseline',url:process.env.ATOGAKI_BASELINE_URL||'http://127.0.0.1:1431'},{name:'current',url:process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430'}];
const hash=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
fs.writeFileSync(path.join(output,'tool-source.json'),JSON.stringify({files:Object.fromEntries(['scripts/measure-editor-version-pair.cjs','scripts/editor-ui-fixture.cjs'].map(f=>[f,hash(f)])),scope:'production WebKit, synthetic IPC and 20-second audio; 3h cue timeline; no native TTI or backend evidence-read timing'},null,2));
const roots={current:process.env.ATOGAKI_CURRENT_ROOT||process.cwd(),baseline:baselineRoot};
const provenance={};
for(const [name,root] of Object.entries(roots)){
 const files=['index.html',...fs.readdirSync(path.join(root,'ui/dist/assets')).map(f=>'assets/'+f)];
 provenance[name]={root,sourceCommit:execFileSync('git',['-C',root,'rev-parse','HEAD'],{encoding:'utf8'}).trim(),production:Object.fromEntries(files.map(f=>[f,hash(path.join(root,'ui/dist',f))]))};
}
fs.writeFileSync(path.join(output,'production-provenance.json'),JSON.stringify(provenance,null,2));
const results=[];const persist=()=>fs.writeFileSync(path.join(output,'results.json'),JSON.stringify(results,null,2));
(async()=>{
 for(const [index,count] of counts.entries())for(const version of index%2?[...versions].reverse():versions){
  const served=await (await fetch(version.url)).text();assert.equal(crypto.createHash('sha256').update(served).digest('hex'),provenance[version.name].production['index.html'],'preview must serve recorded production bundle');
  const browser=await webkit.launch({headless:true});const result={version:version.name,count,browserVersion:browser.version(),status:'started',protocol:2,phase:'startup',startedUtc:Date.now()};results.push(result);persist();
  const deadline=setTimeout(()=>{result.deadlineExceeded=true;browser.close().catch(()=>{});},180000);
  try{
   const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.setDefaultTimeout(60000);page.on('pageerror',e=>errors.push(String(e)));
   await require('./editor-ui-fixture.cjs').installEditorFixture(page,count,10800000);
   await page.addInitScript(()=>{
    window.__pair={frames:[],callbacks:[],active:false,armed:false,done:false,trustedWheels:0,longTasks:null};
    if(PerformanceObserver.supportedEntryTypes?.includes('longtask')){window.__pair.longTasks=[];new PerformanceObserver(list=>window.__pair.longTasks.push(...list.getEntries().map(e=>({start:e.startTime,duration:e.duration})))).observe({type:'longtask',buffered:true});}
    document.addEventListener('click',e=>{if(e.target.closest?.('.job-open')&&window.__pair.openStart===undefined)window.__pair.openStart=performance.now();},true);
    document.addEventListener('wheel',e=>{const state=window.__pair;if(e.isTrusted&&state.armed){state.armed=false;state.active=true;state.eventStart=performance.now();state.trustedWheels++;setTimeout(()=>{state.active=false;state.done=true;state.eventEnd=performance.now();},2000);}},{capture:true,passive:true});
    const tick=t=>{if(window.__pair.active){window.__pair.frames.push(t);window.__pair.callbacks.push(performance.now());}requestAnimationFrame(tick)};requestAnimationFrame(tick);
   });
   await page.goto(version.url);await page.locator('.job-open').first().click();
   if(version.name==='baseline')await page.getByRole('tab',{name:'字幕校对 播放与文字粗修'}).click();
   const source=version.name==='baseline'?'#subtitle-list .source-input':'#karaoke-current-source';
   const row=version.name==='baseline'?'#subtitle-list .subtitle-card':'.editor-cue-row';
   const host=version.name==='baseline'?'#subtitle-list':'#editor-cue-list';
   const search=version.name==='baseline'?'.subtitle-search input':'#editor-search';
   await page.locator(source).first().waitFor();assert.match(await page.locator(source).first().inputValue(),/第 1 句/);
   result.open=await page.evaluate(async()=>{await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return {click_to_editable_observed_ms:performance.now()-window.__pair.openStart,elements:document.querySelectorAll('*').length,textareas:document.querySelectorAll('textarea').length,longTaskApiSupported:window.__pair.longTasks!==null};});
   result.open.rows=await page.locator(row).count();result.phase='search';persist();
   result.search=[];
   for(const query of ['第 '+count+' 句','第']){
    const measure=await page.locator(search).evaluate(async(e,query)=>{const start=performance.now();e.value=query;e.dispatchEvent(new Event('input',{bubbles:true}));const dispatch=performance.now()-start;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return {query,dispatch_ms:dispatch,event_to_two_frames_ms:performance.now()-start,elements:document.querySelectorAll('*').length};},query);
    measure.rendered_matches=version.name==='baseline'?await page.locator('.subtitle-search-results [data-search-time]').count():await page.locator(row).count();
    if(query.startsWith('第 '))assert.equal(measure.rendered_matches,1);else assert.ok(version.name==='baseline'?measure.rendered_matches===count:measure.rendered_matches<40);
    result.search.push(measure);
   }
   result.phase='scroll';persist();await page.locator(search).fill('');result.scroll=[];
   for(let trial=0;trial<3;trial++){
    const setup=await page.locator(host).evaluate(async e=>{const before=getComputedStyle(e).scrollBehavior,inline=e.style.scrollBehavior;e.style.scrollBehavior='auto';e.scrollTop=Math.floor((e.scrollHeight-e.clientHeight)*.4);e.style.scrollBehavior=inline;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const first=e.scrollTop;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return {first,second:e.scrollTop,behaviorBefore:before,behaviorAfter:getComputedStyle(e).scrollBehavior};});
    assert.equal(setup.first,setup.second,'setup must finish before sampling');assert.equal(setup.behaviorBefore,setup.behaviorAfter,'restore original CSS behavior');
    await page.locator(host).hover();
    await page.evaluate(selector=>{const s=window.__pair;s.frames=[];s.callbacks=[];s.trustedWheels=0;s.active=false;s.done=false;s.armed=true;s.scrollStart=document.querySelector(selector).scrollTop;},host);
    // Public WebKit mouse.wheel prepares scrolling state and waits one RAF.
    // Start sampling in the trusted wheel handler, excluding that driver work.
    await page.mouse.wheel(0,840);await page.waitForFunction(()=>window.__pair.done,{},{polling:100,timeout:60000});
    const measurement=await page.evaluate(selector=>{const s=window.__pair;return {frames:s.frames,callbacks:s.callbacks,event_start_ms:s.eventStart,event_end_ms:s.eventEnd,trusted_wheels:s.trustedWheels,start:s.scrollStart,end:document.querySelector(selector).scrollTop,rows:document.querySelectorAll(selector+' '+(selector==='#subtitle-list'?'.subtitle-card':'.editor-cue-row')).length};},host);
    assert.equal(measurement.trusted_wheels,1);assert.ok(Math.abs(measurement.end-measurement.start-840)<=1,'actual scroll distance must match');result.scroll.push({...measurement,setup});persist();
   }
   result.phase='save';persist();
   await page.evaluate(selector=>window.__pair.rowBefore=document.querySelector(selector),row);
   result.input=await page.locator(source).first().evaluate((e)=>{const start=performance.now();e.value+=' · paired save';e.dispatchEvent(new Event('input',{bubbles:true}));window.__pair.inputStart=start;return {dispatch_ms:performance.now()-start};});
   await page.waitForFunction(()=>window.__test.cues[0].source_text.endsWith(' · paired save'));
   result.save=await page.evaluate(selector=>({input_to_mock_save_observed_ms:performance.now()-window.__pair.inputStart,same_first_row:window.__pair.rowBefore===document.querySelector(selector),elements:document.querySelectorAll('*').length,updates:window.__test.calls.filter(c=>c.cmd==='update_subtitle').length}),row);
   assert.equal(result.save.updates,1);assert.equal(result.save.same_first_row,true);
   result.phase='idle';persist();const before=await page.evaluate(()=>window.__test.calls.length);await page.waitForTimeout(2200);
   result.idle_calls=await page.evaluate(start=>window.__test.calls.slice(start).reduce((all,c)=>(all[c.cmd]=(all[c.cmd]||0)+1,all),{}),before);
   result.long_tasks=await page.evaluate(()=>window.__pair.longTasks);assert.deepEqual(errors,[]);result.errors=errors;result.status='passed';result.phase='complete';
   fs.writeFileSync(path.join(output,version.name+'-'+count+'.json'),JSON.stringify(result,null,2));console.log('PASS',version.name,count,'rows',result.open.rows,'elements',result.open.elements);
  }catch(error){result.status='failed';result.error=String(error);throw error;}
  finally{clearTimeout(deadline);persist();await browser.close();}
 }
 for(const [name,metadata] of Object.entries(provenance))for(const [file,expected] of Object.entries(metadata.production))assert.equal(hash(path.join(roots[name],'ui/dist',file)),expected,'production changed during measurement');
 fs.writeFileSync(path.join(output,'completed.json'),JSON.stringify({allCasesPassed:results.every(r=>r.status==='passed'),cases:results.length,productionUnchanged:true},null,2));
})().catch(e=>{console.error(e);process.exitCode=1});
