const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),{stripTypeScriptTypes}=require('node:module');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:960,height:640}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await page.setContent('<style>.hidden{display:none!important} .model-card{padding:10px} .model-card span{display:block}</style><input id="other" aria-label="Other input"><div id="models"></div>');
  await page.addStyleTag({path:'ui/src/styles.css'});
  await page.evaluate(async source=>{
   const url=URL.createObjectURL(new Blob([source],{type:'text/javascript'}));const {mountModelManagementView}=await import(url);URL.revokeObjectURL(url);
   const state=window.__view={catalog:['a','b'].map(id=>({id,kind:'whisper',name:'Model <'+id+'>',sizeLabel:'1 KiB',recommendedFor:'fixture <text>'})),downloads:[{modelId:'a',status:'downloading',downloadedBytes:1,totalBytes:4},{modelId:'b',status:'done',downloadedBytes:4,totalBytes:4,active:false}],busy:false,visit:1,dead:false,calls:[],errors:[],release:null,reject:null};
   const guard=()=>{if(state.dead)throw Error('disposed context');};
   state.view=mountModelManagementView(document.querySelector('#models'),{visit:()=>{guard();return state.visit;},state:()=>{guard();return state;},formatBytes:n=>n+' bytes',error:text=>state.errors.push(text),action:async(action,id)=>{state.calls.push([action,id]);state.busy=true;state.view.render();try{await new Promise((resolve,reject)=>{state.release=resolve;state.reject=reject;});}finally{state.busy=false;state.view.render();}}});
   state.nodes={card:document.querySelector('[data-model-card="a"]'),button:document.querySelector('[data-model-id="a"][data-primary]'),progress:document.querySelector('[data-model-card="a"] progress')};
  },stripTypeScriptTypes(fs.readFileSync('ui/src/model-management-view.ts','utf8')));
  const primary=id=>page.locator(`[data-model-card="${id}"] [data-primary]`),status=id=>page.locator(`[data-model-card="${id}"] [data-status]`);
  await page.locator('[data-model-group="transcription"] > summary').click();
  assert.equal(await page.locator('[data-model-card="a"] [data-name]').textContent(),'Model <a>');assert.equal(await page.locator('#models script').count(),0);
  await primary('a').focus();await page.evaluate(()=>{window.__view.downloads[0].downloadedBytes=3;window.__view.view.render();});assert.equal(await primary('a').evaluate(e=>e===window.__view.nodes.button&&document.activeElement===e),true);assert.equal(await page.locator('[data-model-card="a"] progress').evaluate(e=>e===window.__view.nodes.progress&&e.value===.75),true);
  assert.equal(await page.evaluate(()=>{const observer=new MutationObserver(()=>{});observer.observe(document.querySelector('#models'),{subtree:true,attributes:true,childList:true,characterData:true});window.__view.view.render();const count=observer.takeRecords().length;observer.disconnect();return count;}),0);
  await primary('a').press('Enter');assert.equal(await status('a').evaluate(e=>document.activeElement===e),true);assert.equal(await primary('b').isDisabled(),true);await page.locator('#other').fill('keep my focus');await page.evaluate(()=>{window.__view.downloads[0]={modelId:'a',status:'cancelled',downloadedBytes:3,totalBytes:4};window.__view.release();});await page.waitForFunction(()=>!window.__view.busy);assert.equal(await page.locator('#other').evaluate(e=>document.activeElement===e),true);assert.equal(await primary('a').getAttribute('data-model-action'),'download');assert.equal(await primary('a').evaluate(e=>e===window.__view.nodes.button),true);
  await primary('a').focus();await primary('a').press('Enter');await page.evaluate(()=>window.__view.reject(Error('fixture operation failed')));await page.waitForFunction(()=>window.__view.errors.length===1);assert.equal(await primary('a').evaluate(e=>document.activeElement===e),true);
  await primary('b').focus();await primary('b').press('Enter');await page.evaluate(()=>{window.__view.downloads[1].active=true;window.__view.release();});await page.waitForFunction(()=>!window.__view.busy);assert.equal(await primary('b').isVisible(),false);assert.equal(await status('b').evaluate(e=>document.activeElement===e),true);assert.equal(await page.locator('[data-model-card="b"] [data-model-action="uninstall"]').evaluate(e=>document.activeElement===e),false);
  const uninstall=page.locator('[data-model-card="b"] [data-model-action="uninstall"]');await uninstall.focus();await uninstall.press('Enter');await page.evaluate(()=>{window.__view.downloads=window.__view.downloads.filter(item=>item.modelId!=='b');window.__view.release();});await page.waitForFunction(()=>!window.__view.busy);assert.equal(await primary('b').getAttribute('data-model-action'),'download');assert.equal(await primary('b').evaluate(e=>document.activeElement===e),true);
  await primary('a').focus();await page.evaluate(()=>{window.__view.catalog.reverse();window.__view.view.render();});assert.equal(await primary('a').evaluate(e=>document.activeElement===e),true);await page.evaluate(()=>{window.__view.catalog=window.__view.catalog.filter(item=>item.id==='a');window.__view.view.render();});assert.equal(await page.locator('[data-model-card]').count(),1);assert.equal(await page.locator('[data-model-card="a"]').evaluate(e=>e===window.__view.nodes.card),true);
  await primary('a').press('Enter');const before=await page.evaluate(()=>{window.__view.view.dispose();window.__view.dead=true;return {calls:window.__view.calls.length,errors:window.__view.errors.length};});await page.evaluate(()=>{window.__view.reject(Error('late failure'));window.__view.view.render();});await page.evaluate(()=>new Promise(requestAnimationFrame));assert.equal(await page.locator('[data-model-card]').count(),0);assert.deepEqual(await page.evaluate(()=>({calls:window.__view.calls.length,errors:window.__view.errors.length})),before);assert.deepEqual(errors,[]);
  const catalog=JSON.parse(fs.readFileSync('src-tauri/src/model-catalog.json','utf8')).map(({files,bundle,...item})=>item);
  await page.evaluate(async({source,catalog})=>{
   const url=URL.createObjectURL(new Blob([source],{type:'text/javascript'}));const {mountModelManagementView}=await import(url);URL.revokeObjectURL(url);
   const state=window.__grouped={catalog,downloads:[],busy:false,calls:[]};
   state.view=mountModelManagementView(document.querySelector('#models'),{visit:()=>1,state:()=>state,formatBytes:n=>n+' bytes',error:text=>{throw Error(text)},action:async(...args)=>state.calls.push(args)});
  },{source:stripTypeScriptTypes(fs.readFileSync('ui/src/model-management-view.ts','utf8')),catalog});
  assert.equal(await page.locator('#models > [data-model-group]').count(),4);
  assert.equal(await primary('whisper-medium').isVisible(),false);
  await page.locator('[data-model-group="transcription"] > summary').click();
  assert.equal(await primary('whisper-medium').isVisible(),true);
  assert.equal(await primary('whisper-small').isVisible(),false);
  assert.equal(await page.locator('[data-model-card="hy-mt2-7b-q4-k-m"]').count(),0);
  const qwen=page.locator('[data-model-group="transcription/Qwen3-ASR"]');
  assert.equal(await primary('qwen-0.6b').isVisible(),false);await qwen.locator(':scope > summary').click();
  assert.equal(await primary('qwen-1.7b-8bit').isVisible(),true);
  assert.equal(await primary('qwen-0.6b').isVisible(),false);
  await page.locator('[data-model-group="transcription/Qwen3-ASR/other"] > summary').click();
  for(const id of ['qwen-0.6b','qwen-0.6b-4bit','qwen-1.7b-8bit'])assert.equal(await primary(id).isVisible(),true);
  await primary('qwen-0.6b-4bit').click();assert.deepEqual(await page.evaluate(()=>window.__grouped.calls),[['start','qwen-0.6b-4bit']]);
  await page.evaluate(()=>{window.__grouped.downloads=[{modelId:'qwen-0.6b-4bit',status:'done',active:false}];window.__grouped.view.render()});
  assert.equal(await primary('qwen-0.6b-4bit').isVisible(),false);
  assert.equal(await page.locator('[data-model-card="qwen-0.6b-4bit"] [data-model-action="uninstall"]').isVisible(),true);
  assert.equal(await qwen.evaluate(e=>e.open),true);
  await page.locator('[data-model-group="draft"] > summary').click();
  assert.equal(await primary('draft-ja').isDisabled(),true);
  assert.equal(await page.locator('[data-model-card="draft-ja"] [data-model-action="uninstall"]').isVisible(),false);
  await page.evaluate(()=>window.__grouped.view.dispose());
  assert.deepEqual(errors,[]);
  console.log('PASS: stable model controls and focus; four collapsed categories, Medium first, Qwen variants, installed package actions and environment-managed drafts');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
