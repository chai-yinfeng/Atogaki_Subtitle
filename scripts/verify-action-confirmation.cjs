const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),{stripTypeScriptTypes}=require('node:module');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  const page=await browser.newPage({viewport:{width:960,height:640}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await page.setContent('<main><button id="trigger">open</button><div id="frozen" inert>already inert</div><dialog id="parent"><button id="nested">nested action</button><button id="other">other</button></dialog><dialog id="confirmation-dialog"><form><h2 id="confirmation-title"></h2><p id="confirmation-message"></p><button id="cancel-confirmation" type="button">取消</button><button id="accept-confirmation" type="submit"></button></form></dialog></main>');
  await page.evaluate(async sources=>{
   const load=source=>URL.createObjectURL(new Blob([source],{type:'text/javascript'}));const tools=load(sources.tools),view=load(sources.view.replace('"./task-tools"',JSON.stringify(tools)));
   const {mountActionConfirmation}=await import(view);URL.revokeObjectURL(view);URL.revokeObjectURL(tools);
   const state=window.__confirm={results:[],compact:true,dead:false,request:0};
   state.view=mountActionConfirmation(document.querySelector('#confirmation-dialog'),()=>{if(state.dead)throw Error('disposed callback');return state.compact;});
   state.ask=value=>{const request=++state.request;state.view.ask({title:'确认 '+request,message:'<script>literal text</script>',confirmLabel:'确定',danger:true,...value}).then(result=>state.results.push({request,result}));};
  },{tools:stripTypeScriptTypes(fs.readFileSync('ui/src/task-tools.ts','utf8')),view:stripTypeScriptTypes(fs.readFileSync('ui/src/action-confirmation.ts','utf8'))});
  assert.equal(await page.locator('#confirmation-dialog').count(),0);
  await page.locator('#trigger').focus();await page.evaluate(()=>window.__confirm.ask({}));
  const dialog=page.locator('#confirmation-dialog'),cancel=page.locator('#cancel-confirmation'),accept=page.locator('#accept-confirmation');
  assert.equal(await dialog.getAttribute('role'),'region');assert.equal(await dialog.getAttribute('aria-label'),'确认 1');assert.equal(await dialog.evaluate(d=>d.matches(':modal')),false);assert.equal(await dialog.evaluate(d=>d.classList.contains('editor-confirmation')),true);
  assert.equal(await page.locator('#confirmation-message').innerText(),'<script>literal text</script>');assert.equal(await page.locator('main > #trigger').evaluate(e=>e.inert),true);
  await cancel.press('Shift+Tab');assert.equal(await accept.evaluate(e=>e===document.activeElement),true);await accept.press('Tab');assert.equal(await cancel.evaluate(e=>e===document.activeElement),true);
  await dialog.press('Escape');assert.deepEqual(await page.evaluate(()=>window.__confirm.results),[{request:1,result:false}]);assert.equal(await page.locator('#trigger').evaluate(e=>e===document.activeElement&&!e.inert),true);assert.equal(await page.locator('#frozen').evaluate(e=>e.inert),true);
  await page.evaluate(()=>{const s=window.__confirm;s.ask({});s.ask({danger:false});});
  await page.waitForFunction(()=>window.__confirm.results.length===2);assert.equal(await dialog.getAttribute('aria-label'),'确认 3');assert.equal(await accept.evaluate(e=>e.classList.contains('danger')),false);
  await accept.click();await page.waitForFunction(()=>window.__confirm.results.length===3);assert.deepEqual(await page.evaluate(()=>window.__confirm.results.slice(1)),[{request:2,result:false},{request:3,result:true}]);
  await page.evaluate(()=>{const parent=document.querySelector('#parent');parent.showModal();document.querySelector('#nested').focus();window.__confirm.compact=false;window.__confirm.ask({});});
  assert.equal(await dialog.evaluate(d=>d.parentElement.id),'parent');assert.equal(await dialog.evaluate(d=>!!d.closest('[inert]')),false);assert.equal(await page.locator('#other').evaluate(e=>e.inert),true);assert.equal(await cancel.evaluate(e=>e===document.activeElement),true);
  const box=await dialog.boundingBox();assert.ok(box&&box.width>0);await cancel.click();assert.equal(await page.locator('#parent').evaluate(d=>d.open&&d.matches(':modal')),true);assert.equal(await page.locator('#nested').evaluate(e=>e===document.activeElement&&!e.inert),true);assert.equal(await page.locator('#other').evaluate(e=>e.inert),false);
  await page.evaluate(()=>window.__confirm.ask({}));await accept.click();await page.waitForFunction(()=>window.__confirm.results.length===5);assert.deepEqual(await page.evaluate(()=>window.__confirm.results.slice(3)),[{request:4,result:false},{request:5,result:true}]);
  await page.evaluate(()=>document.querySelector('#parent').close());await page.locator('#trigger').focus();await page.evaluate(()=>window.__confirm.ask({}));await page.evaluate(()=>document.querySelector('#confirmation-dialog').close());await page.waitForFunction(()=>window.__confirm.results.length===6);assert.equal(await dialog.count(),0);
  await page.evaluate(()=>{const s=window.__confirm;s.ask({});s.view.dispose();s.view.dispose();s.dead=true;s.ask({});});await page.waitForFunction(()=>window.__confirm.results.length===8);assert.deepEqual(await page.evaluate(()=>window.__confirm.results.slice(5)),[{request:6,result:false},{request:7,result:false},{request:8,result:false}]);assert.equal(await dialog.count(),0);assert.equal(await page.locator('#trigger').evaluate(e=>!e.inert),true);assert.deepEqual(errors,[]);
  console.log('PASS: named confirmation, literal text, focus loop/return, replacement, nested native modal, cancel/accept, external close and disposal');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
