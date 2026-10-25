// Trusted WebKit wheel input on the production UI; synthetic IPC/media only.
// Frame intervals, quantized listener wall time and DOM writes are separate metrics.
const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const {installEditorFixture}=require('./editor-ui-fixture.cjs');
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),crypto=require('node:crypto');
const output=process.env.ATOGAKI_EDITOR_QA_OUTPUT||'/tmp/atogaki-editor-wheel';
const label=process.env.ATOGAKI_MEASUREMENT_LABEL||'wheel';
const percentile=(values,p)=>values.length?[...values].sort((a,b)=>a-b)[Math.min(values.length-1,Math.floor(values.length*p))]:null;
(async()=>{
 fs.mkdirSync(output,{recursive:true});const file=path.join(output,label+'-wheel.json');if(fs.existsSync(file))throw Error('Refusing to overwrite evidence: '+file);
 const browser=await webkit.launch({headless:true}),results=[];
 try{
  for(const count of [300,3000,10000]){
   const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));await installEditorFixture(page,count,10800000);
   await page.addInitScript(()=>{
    const add=EventTarget.prototype.addEventListener;
    window.__wheelMeasurement={active:false,listenerTimes:[],trusted:0,untrusted:0};
    EventTarget.prototype.addEventListener=function(type,listener,options){
     if(type!=='wheel'||this.id!=='editor-cue-list'||!listener)return add.call(this,type,listener,options);
     return add.call(this,type,function(event){const state=window.__wheelMeasurement;if(!state.active)return typeof listener==='function'?listener.call(this,event):listener.handleEvent(event);const start=performance.now();try{return typeof listener==='function'?listener.call(this,event):listener.handleEvent(event);}finally{state.listenerTimes.push(performance.now()-start);if(event.isTrusted)state.trusted++;else state.untrusted++;}},options);
    };
   });
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();await page.locator('.editor-cue-row').first().waitFor();
   const host=page.locator('#editor-cue-list');await host.focus();await host.press('End');
   const rounds=[];
   for(let round=0;round<3;round++){
    await host.evaluate(element=>element.scrollTop=(element.scrollHeight-element.clientHeight)/2);await page.waitForTimeout(100);
    const box=await host.boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);
    await page.evaluate(()=>{
     const host=document.querySelector('#editor-cue-list'),state=window.__wheelMeasurement;
     state.listenerTimes=[];state.trusted=0;state.untrusted=0;state.controlMutations=0;state.frames=[];state.rows=[];state.scrollStart=host.scrollTop;state.active=true;
     let previous=performance.now();const tick=now=>{if(!state.active)return;state.frames.push(now-previous);previous=now;state.rows.push(host.querySelectorAll('.editor-cue-row').length);requestAnimationFrame(tick);};requestAnimationFrame(tick);
     const controls=[document.querySelector('[data-editor-action="follow"]'),document.querySelector('[data-editor-action="reveal-playing"]'),document.querySelector('#editor-selection-message')];
     state.observers=controls.map(element=>{const observer=new MutationObserver(records=>state.controlMutations+=records.length);observer.observe(element,{attributes:true,childList:true,characterData:true,subtree:true});return observer;});
     const deltas=[];let last=performance.now();for(let i=0;i<50000;i++){const now=performance.now();if(now>last)deltas.push(now-last);last=now;}state.clockStepMs=deltas.length?Math.min(...deltas):null;
    });
    for(let i=0;i<60;i++){await page.mouse.wheel(0,round%2?-14:14);await page.waitForTimeout(20);}
    await page.waitForTimeout(150);
    const measurement=await page.evaluate(()=>{const state=window.__wheelMeasurement;state.active=false;for(const observer of state.observers)observer.disconnect();return{listenerTimes:state.listenerTimes,frames:state.frames,controlMutations:state.controlMutations,trusted:state.trusted,untrusted:state.untrusted,clockStepMs:state.clockStepMs,maxRows:Math.max(...state.rows),scrollStart:state.scrollStart,scrollEnd:document.querySelector('#editor-cue-list').scrollTop,selected:document.querySelector('#karaoke-segment-time').textContent};});
    if(!measurement.trusted||measurement.untrusted||measurement.scrollStart===measurement.scrollEnd)throw Error('Wheel scenario did not scroll with trusted input');
    rounds.push({...measurement,wheelHandlerTotalMs:measurement.listenerTimes.reduce((a,b)=>a+b,0),wheelHandlerP95Ms:percentile(measurement.listenerTimes,.95),frameP95Ms:percentile(measurement.frames,.95),framesOver50Ms:measurement.frames.filter(ms=>ms>50).length});
   }
   if(errors.length)throw Error(errors.join('\n'));results.push({count,errors,rounds});console.log(JSON.stringify({count,rounds:rounds.map(({listenerTimes,frames,...summary})=>summary)}));await page.close();
  }
  const sources=Object.fromEntries(['ui/src/editor-follow.ts','ui/src/editor-cue-list.ts','ui/src/main.ts'].map(file=>[file,crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')]));
  fs.writeFileSync(file,JSON.stringify({label,engine:'WebKit',browserVersion:browser.version(),platform:process.platform,arch:process.arch,loadavg:os.loadavg(),viewport:{width:1180,height:760},requestedWheelsPerRound:60,wheelDeltaPx:14,sources,results},null,2)+'\n',{flag:'wx'});
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
