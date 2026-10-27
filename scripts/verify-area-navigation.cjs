const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.addInitScript(()=>{
   const invoke=window.__TAURI_INTERNALS__.invoke,scroll=window.scrollTo.bind(window);
   window.__areaQA={hold:false,release:null,scrolls:0};
   window.scrollTo=(...args)=>{window.__areaQA.scrolls++;return scroll(...args);};
   window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
    const qa=window.__areaQA;
    if(command==='list_learning_items'&&qa.hold){qa.hold=false;await new Promise(resolve=>qa.release=resolve);}
    return invoke(command,args);
   };
  });
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');
  await page.locator('.job-open').first().waitFor({state:'visible'});
  const settle=()=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  for(const returnToLearning of [false,true]) {
   await page.evaluate(()=>window.__areaQA.hold=true);await page.locator('#show-learning').click();
   await page.waitForFunction(()=>window.__areaQA.release);
   await page.locator('#show-workbench').click();await page.locator('.job-open').first().waitFor({state:'visible'});
   if(returnToLearning){await page.locator('#show-learning').click();await page.locator('#learning-item-list').waitFor({state:'visible'});}
   await settle();await page.evaluate(()=>{window.__areaQA.scrolls=0;window.__areaQA.release();window.__areaQA.release=null;});await settle();
   assert.equal(await page.evaluate(()=>window.__areaQA.scrolls),0,'old learning visit must not scroll the new area or a newer learning visit');
   if(returnToLearning){await page.locator('#show-workbench').click();await page.locator('.job-open').first().waitFor({state:'visible'});}
  }
  await page.locator('.job-open').first().click();await page.locator('#karaoke-current-source').waitFor({state:'visible'});
  assert.match(await page.locator('#karaoke-current-source').inputValue(),/第 1 句/);
  assert.deepEqual(errors,[]);
  console.log('PASS: late learning completion cannot scroll another area or a newer same-area visit; child editor navigation still opens normally');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
