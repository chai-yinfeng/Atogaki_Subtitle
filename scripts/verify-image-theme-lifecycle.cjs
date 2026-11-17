const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jN1sAAAAASUVORK5CYII=';
(async()=>{
 const browser=await webkit.launch({headless:true});
 try{
  for(const mode of ['normal','dispose','builtin','stale-error','new-apply','import','import-success','cancel-import']){
   const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
   page.on('pageerror',e=>errors.push(String(e)));
   await page.route('**/theme-fixture.png',route=>route.fulfill({contentType:'image/png',body:Buffer.from(png,'base64')}));
   await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
   await page.addInitScript(({mode})=>{
    localStorage.setItem('atogaki.appearance.shellVersion','2');
    const name='a'.repeat(64)+'.png',key='atogaki.appearance.imageTheme';
    if(!mode.includes('import'))localStorage.setItem(key,JSON.stringify({version:1,name,displayName:'合成主题',palette:{dominant:[210,220,230],accent:[50,90,130],swatches:[[210,220,230],[50,90,130]]},template:'system',fit:'contain',strength:.45,credit:'fixture',source:'',enabled:true}));
    const invoke=window.__TAURI_INTERNALS__.invoke,qa=window.__themeQA={loads:0,release:null,decodeRelease:null,urls:[],revoked:[]};
    window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
     if(command==='load_theme_image'){
      qa.loads++;if(qa.loads===1&&['dispose','builtin','stale-error','new-apply'].includes(mode)){await new Promise(resolve=>qa.release=resolve);if(mode==='stale-error'||mode==='new-apply')throw Error('old theme unavailable');}
      return '/theme-fixture.png';
     }
     if(command==='import_theme_image')return {name,display_name:'合成主题'};
     if(command==='theme_image_bytes'){
      const c=document.createElement('canvas');c.width=2;c.height=2;const x=c.getContext('2d');x.fillStyle='#5082aa';x.fillRect(0,0,2,2);
      return Uint8Array.from(atob(c.toDataURL().split(',')[1]),c=>c.charCodeAt(0)).buffer;
     }
     return invoke(command,args);
    };
    if(mode.includes('import')){
     const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL),decode=HTMLImageElement.prototype.decode;
     URL.createObjectURL=blob=>{const url=create(blob);qa.urls.push(url);return url;};
     URL.revokeObjectURL=url=>{qa.revoked.push(url);revoke(url);};
     if(mode!=='import-success')HTMLImageElement.prototype.decode=function(){return new Promise(resolve=>qa.decodeRelease=resolve).then(()=>decode.call(this));};
    }
   },{mode});
   await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
   const drain=()=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
   const config=()=>page.evaluate(()=>JSON.parse(localStorage.getItem('atogaki.appearance.imageTheme')||'null'));
   const studio=async()=>{await page.locator('#open-settings').click();await page.locator('[data-settings-tab="appearance"]').click();await page.locator('[data-open-image-studio]').click();};
   if(mode==='normal'){
    await page.waitForFunction(()=>document.documentElement.dataset.imageTheme==='palette');assert.equal((await config()).enabled,true);
    await studio();assert.equal(await page.locator('[data-preview-mode]').count(),2);
    await page.locator('[data-image-name]').fill('未应用名称');await page.keyboard.press('Escape');assert.equal((await config()).displayName,'合成主题');
    await page.emulateMedia({colorScheme:'dark'});await page.waitForFunction(()=>document.documentElement.dataset.theme==='dark');
    assert.equal((await config()).enabled,true);
    await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));assert.equal(await page.locator('.image-theme-editor,.image-theme-studio,.image-theme-credit').count(),0);
   }else if(mode==='dispose'){
    await page.waitForFunction(()=>window.__themeQA.release);const before=await config();
    await page.evaluate(()=>{window.dispatchEvent(new Event('pagehide'));window.__themeQA.release();});await drain();assert.deepEqual(await config(),before);
    assert.equal(await page.evaluate(()=>document.documentElement.dataset.imageTheme),undefined);
   }else if(mode==='builtin'||mode==='stale-error'){
    await page.waitForFunction(()=>window.__themeQA.release);await page.locator('#open-settings').click();await page.locator('[data-settings-tab="appearance"]').click();await page.locator('[data-app-theme="dark"]').click();
    await page.evaluate(()=>window.__themeQA.release());await drain();
    assert.equal(await page.evaluate(()=>document.documentElement.dataset.theme),'dark');assert.equal((await config()).enabled,false);
    assert.equal(await page.evaluate(()=>document.documentElement.dataset.imageTheme),undefined);assert.doesNotMatch(await page.locator('.image-theme-status').textContent(),/old theme unavailable/);
   }else if(mode==='new-apply'){
    await page.waitForFunction(()=>window.__themeQA.release);await studio();await page.locator('[data-apply-image]').click();
    await page.waitForFunction(()=>document.documentElement.dataset.imageTheme==='palette');await page.evaluate(()=>window.__themeQA.release());await drain();
    assert.equal((await config()).enabled,true);assert.doesNotMatch(await page.locator('.image-theme-status').textContent(),/old theme unavailable/);
   }else{
    await studio();await page.locator('[data-import-image]').click();
    if(mode==='import-success'){
     await page.waitForFunction(()=>document.querySelector('.image-studio-status').textContent.includes('配色已生成'));
     assert.equal(await config(),null);assert.equal(await page.evaluate(()=>document.documentElement.dataset.imageTheme),undefined);
     await page.locator('[data-image-name]').fill('我的电台');await page.locator('[data-image-template]').selectOption('dark');
     const applyBounds=await page.locator('[data-apply-image]').boundingBox();assert.ok(applyBounds.y+applyBounds.height<=760);
     await page.screenshot({path:'/tmp/atogaki-image-studio-preview.png'});
     await page.locator('[data-apply-image]').click();await page.waitForFunction(()=>document.documentElement.dataset.imageTheme==='palette');
     assert.equal((await config()).displayName,'我的电台');assert.equal(await page.locator('#image-theme-studio').evaluate(e=>e.open),false);
     await page.screenshot({path:'/tmp/atogaki-image-theme-applied.png'});
     await page.locator('[data-open-image-studio]').click();await page.locator('[data-reset-image]').click();
     assert.equal((await config()).enabled,false);assert.equal((await config()).displayName,'我的电台');
    }else{
     await page.waitForFunction(()=>window.__themeQA.decodeRelease);
     if(mode==='cancel-import')await page.keyboard.press('Escape');
     else await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));
     await page.evaluate(()=>window.__themeQA.decodeRelease());await drain();assert.equal(await config(),null);
    }
    const resources=await page.evaluate(()=>({urls:window.__themeQA.urls,revoked:window.__themeQA.revoked}));assert.equal(resources.urls.length,1);assert.deepEqual(resources.revoked,resources.urls);
   }
   assert.deepEqual(errors,[]);await page.close();
  }
  console.log('PASS: Material light/dark studio, preview before apply, names, cancel/reset, system mode, old data restore, newer decisions defeat stale replies, disposal and blob cleanup.');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
