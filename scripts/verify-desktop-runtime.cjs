const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  for(const delayed of [false,true]) {
   const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
   page.on('pageerror',e=>errors.push(String(e)));
   await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
   await page.addInitScript(delayed=>{
    const ipc=window.__TAURI_INTERNALS__,invoke=ipc.invoke,repeat=window.setInterval.bind(window),cancel=window.clearInterval.bind(window);
    const callbacks=new Map();let next=0;
    const qa=window.__runtimeQA={listeners:[],unlistens:[],intervals:[],cleared:[],release:null};
    ipc.transformCallback=fn=>{callbacks.set(++next,fn);return next;};
    ipc.unregisterCallback=id=>callbacks.delete(id);
    window.setInterval=(fn,delay,...args)=>{const id=repeat(fn,delay,...args);qa.intervals.push({id,delay,owner:new Error().stack});return id;};
    window.clearInterval=id=>{qa.cleared.push(id);cancel(id);};
    ipc.invoke=async(command,args={})=>{
     const result=await invoke(command,args);
     if(command==='plugin:event|listen') {
      const entry={event:args.event,id:++next,callback:callbacks.get(args.handler)};qa.listeners.push(entry);
      if(delayed&&args.event==='subtitle-overlay-visibility')await new Promise(resolve=>qa.release=resolve);
      return entry.id;
     }
     if(command==='plugin:event|unlisten')qa.unlistens.push({event:args.event,id:args.eventId});
     return result;
    };
   },delayed);
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
   const names=['subtitle-overlay-visibility','subtitle-overlay-playback-action','tauri://close-requested'];
   await page.waitForFunction(names=>names.every(name=>window.__runtimeQA.listeners.some(item=>item.event===name)),names);
   if(delayed)await page.waitForFunction(()=>window.__runtimeQA.release);
   await page.locator('.job-open').first().click();await page.locator('#karaoke-current-source').waitFor({state:'visible'});
   // Page lifecycle dispatch exercises the actual application disposer with mocked IPC.
   const intervals=await page.evaluate(()=>{window.dispatchEvent(new Event('pagehide'));return window.__runtimeQA.intervals;});
   if(delayed)await page.evaluate(()=>{window.__runtimeQA.release();window.__runtimeQA.release=null;});
   await page.waitForFunction(names=>names.every(name=>window.__runtimeQA.unlistens.some(item=>item.event===name)),names);
   const state=await page.evaluate(()=>({cleared:window.__runtimeQA.cleared,unlistens:window.__runtimeQA.unlistens}));
   assert.ok(intervals.some(t=>t.delay===2000));assert.ok(intervals.some(t=>t.delay===1000));
   for(const timer of intervals.filter(timer=>!timer.owner.includes('/@vite/client')))assert.ok(state.cleared.includes(timer.id),'pagehide must release each mounted startup interval: '+JSON.stringify({timer,cleared:state.cleared}));
   for(const name of names)assert.equal(state.unlistens.filter(e=>e.event===name).length,1,'desktop event must be unregistered exactly once');
   assert.deepEqual(errors,[]);await page.close();
  }
  console.log('PASS: pagehide releases desktop intervals and all three event listeners; a late registration is immediately unregistered');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
