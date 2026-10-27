const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jN1sAAAAASUVORK5CYII=';
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  for(const mode of ['normal','dispose','builtin','stale-error','new-apply','import','import-success']) {
   const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
   page.on('pageerror',e=>errors.push(String(e)));
   await page.route('**/theme-fixture.png',route=>route.fulfill({status:200,contentType:'image/png',body:Buffer.from(png,'base64')}));
   await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
   await page.addInitScript(({mode,png})=>{
    const name='a'.repeat(64)+'.png',key='atogaki.appearance.imageTheme';
    if(!mode.startsWith('import'))localStorage.setItem(key,JSON.stringify({version:1,name,displayName:'合成主题',palette:{dominant:[210,220,230],accent:[50,90,130],swatches:[[210,220,230],[50,90,130]]},template:'moonlight',fit:'contain',strength:.45,credit:'fixture',source:'',enabled:true}));
    const invoke=window.__TAURI_INTERNALS__.invoke,qa=window.__themeQA={loads:0,release:null,decodeRelease:null,urls:[],revoked:[]};
    window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
     if(command==='load_theme_image'){
      qa.loads++;if(qa.loads===2&&['dispose','builtin','stale-error'].includes(mode)||qa.loads===1&&mode==='new-apply'){await new Promise(resolve=>qa.release=resolve);if(mode==='stale-error'||mode==='new-apply')throw Error('old theme unavailable');}
      return '/theme-fixture.png';
     }
     if(command==='import_theme_image')return {name,display_name:'合成主题',path:'/theme-fixture.png'};
     if(command==='theme_image_bytes'){const canvas=document.createElement('canvas');canvas.width=2;canvas.height=2;const context=canvas.getContext('2d');context.fillStyle='#5082aa';context.fillRect(0,0,2,2);return Uint8Array.from(atob(canvas.toDataURL().split(',')[1]),c=>c.charCodeAt(0)).buffer;}
     return invoke(command,args);
    };
    if(mode.startsWith('import')){
     const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL),decode=HTMLImageElement.prototype.decode;
     URL.createObjectURL=blob=>{const url=create(blob);qa.urls.push(url);return url;};
     URL.revokeObjectURL=url=>{qa.revoked.push(url);revoke(url);};
     if(mode==='import')HTMLImageElement.prototype.decode=function(){return new Promise(resolve=>qa.decodeRelease=resolve).then(()=>decode.call(this));};
    }
   },{mode,png});
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
   const drain=()=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
   const config=()=>page.evaluate(()=>JSON.parse(localStorage.getItem('atogaki.appearance.imageTheme')||'null'));
   if(mode==='normal') {
    await page.waitForFunction(()=>document.documentElement.dataset.imageTheme==='contain');assert.equal((await config()).enabled,true);assert.match(await page.locator('.image-theme-credit').innerText(),/fixture/);
    await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));assert.equal(await page.locator('.image-theme-editor,.image-theme-credit').count(),0);
   } else if(mode==='dispose') {
    await page.waitForFunction(()=>window.__themeQA.release);const before=await config();
    await page.evaluate(()=>{window.dispatchEvent(new Event('pagehide'));window.__themeQA.release();});await drain();
    assert.equal(await page.locator('.image-theme-editor,.image-theme-credit').count(),0);assert.deepEqual(await config(),before);assert.equal(await page.evaluate(()=>document.documentElement.dataset.imageTheme),undefined);
    await page.evaluate(()=>{document.documentElement.dataset.theme='letter';window.dispatchEvent(new Event('atogaki-theme-change'));});assert.deepEqual(await config(),before,'disposed window listener must not save preferences');
   } else if(mode==='builtin'||mode==='stale-error') {
    await page.waitForFunction(()=>window.__themeQA.release);await page.locator('#open-settings').click();await page.locator('[data-settings-tab="appearance"]').click();await page.locator('[data-app-theme="letter"]').click();
    await page.evaluate(()=>window.__themeQA.release());await drain();assert.equal(await page.evaluate(()=>document.documentElement.dataset.theme),'letter');assert.equal(await page.evaluate(()=>document.documentElement.dataset.imageTheme),undefined);assert.equal((await config()).enabled,false);assert.doesNotMatch(await page.locator('.image-theme-status').textContent(),/old theme unavailable|已恢复本机图片主题/);
   } else if(mode==='new-apply') {
    await page.waitForFunction(()=>window.__themeQA.release);await page.locator('#open-settings').click();await page.locator('[data-settings-tab="appearance"]').click();await page.locator('[data-apply-image]').click();
    await page.waitForFunction(()=>document.documentElement.dataset.imageTheme==='contain');await page.evaluate(()=>window.__themeQA.release());await drain();
    assert.equal(await page.evaluate(()=>document.documentElement.dataset.imageTheme),'contain');assert.equal((await config()).enabled,true);assert.doesNotMatch(await page.locator('.image-theme-status').textContent(),/old theme unavailable/);
   } else if(mode==='import-success') {
    await page.locator('#open-settings').click();await page.locator('[data-settings-tab="appearance"]').click();await page.locator('[data-import-image]').click();
    await page.waitForFunction(()=>document.documentElement.dataset.imageTheme==='contain');assert.equal((await config()).enabled,true);assert.ok((await config()).palette.swatches.length>0);
    await page.locator('[data-refresh-palette]').click();await page.waitForFunction(()=>document.querySelector('.image-theme-status').textContent.includes('已提取'));
    const resources=await page.evaluate(()=>({urls:window.__themeQA.urls,revoked:window.__themeQA.revoked}));assert.equal(resources.urls.length,2);assert.deepEqual(resources.revoked,resources.urls);
   } else {
    await page.locator('#open-settings').click();await page.locator('[data-settings-tab="appearance"]').click();await page.locator('[data-import-image]').click();await page.waitForFunction(()=>window.__themeQA.decodeRelease);
    assert.equal(await page.evaluate(()=>window.__themeQA.urls.length),1);await page.evaluate(()=>{window.dispatchEvent(new Event('pagehide'));window.__themeQA.decodeRelease();});await drain();
    const resources=await page.evaluate(()=>({urls:window.__themeQA.urls,revoked:window.__themeQA.revoked}));assert.deepEqual(resources.revoked,resources.urls);assert.equal(await config(),null);assert.equal(await page.locator('.image-theme-editor,.image-theme-credit').count(),0);
   }
   assert.deepEqual(errors,[]);await page.close();
  }
  console.log('PASS: theme restore/import/palette refresh remain functional; builtin choices and newer applies defeat stale success/error; disposal removes listeners/nodes and revokes an in-flight analysis URL without saving');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
