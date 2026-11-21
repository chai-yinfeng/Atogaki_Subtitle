// Exercise the two user actions with synthetic IPC; no download or installation.
const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');

(async()=>{
 const browser=await webkit.launch();
 try{
  const page=await browser.newPage();
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,10);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke;
   window.__updateQA={enabled:false,installs:[],unblock:null};
   window.__TAURI_INTERNALS__.invoke=async(cmd,args)=>{
    const qa=window.__updateQA;
    if(cmd==='check_desktop_update')return {enabled:qa.enabled,currentVersion:'0.1.3',availableVersion:qa.enabled?'0.1.4':null};
    if(cmd==='install_desktop_update'){
     qa.installs.push(args.version);
     await new Promise(resolve=>qa.unblock=resolve);
     throw Error('测试签名校验失败');
    }
    return invoke(cmd,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  await page.locator('#open-usage-guide').click();
  await page.getByText('App 更新与系统音频授权',{exact:true}).click();
  assert.equal(await page.locator('#usage-guide #check-app-update').count(),0);
  await page.locator('#close-usage-guide').click();
  await page.locator('#open-settings').click();
  await page.locator('[data-settings-tab="updates"]').click();
  const check=page.locator('#check-app-update'),install=page.locator('#install-app-update');
  await check.click();
  await page.getByText('当前 v0.1.3 未启用应用内更新。此功能需要包含发行公钥的 Apple Silicon 版本。',{exact:true}).waitFor();
  assert.equal(await install.isVisible(),false);
  await page.evaluate(()=>window.__updateQA.enabled=true);
  await check.click();
  await page.getByText('当前 v0.1.3，可更新到 v0.1.4。',{exact:true}).waitFor();
  assert.deepEqual(await page.evaluate(()=>window.__updateQA.installs),[]);
  await install.click();
  await page.waitForFunction(()=>window.__updateQA.unblock!==null);
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('#settings-dialog').evaluate(e=>e.open),true);
  assert.deepEqual(await page.evaluate(()=>window.__updateQA.installs),['0.1.4']);
  await page.evaluate(()=>window.__updateQA.unblock());
  await page.getByText(/安装未完成.*测试签名校验失败/).waitFor();
  await page.locator('#close-settings').click();
  assert.equal(await page.locator('#settings-dialog').evaluate(e=>e.open),false);
  console.log('PASS: updater availability, separate install confirmation, busy sheet and error recovery');
 }finally{await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
