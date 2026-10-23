const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');const assert=require('node:assert/strict');
(async()=>{const browser=await webkit.launch({headless:true});try{
 const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
 await page.addInitScript(()=>{
  const invoke=window.__TAURI_INTERNALS__.invoke;
  const style={font_family:'Fixture Sans',font_size:30,primary_color:'#FFFFFFFF',outline_color:'#00000080',outline_width:2,shadow:1,background_color:null,bold:false,italic:false,alignment:2,margin_left:20,margin_right:20,margin_vertical:20,letter_spacing:0};
  const coverage={requested_family:'Fixture Sans',installed:true,matched_family:'Fixture Sans',post_script_name:'FixtureSans',missing_characters:[],needs_fallback:false};
  const state={styles:{schema_version:1,source:{...style},translation:{...style}},font_report:{source:coverage,translation:coverage}};
  window.__style={state,hold:false,pending:[],applied:[]};
  window.__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
   if(!['get_subtitle_style_state','list_subtitle_fonts','save_subtitle_styles','preview_subtitle_styles'].includes(cmd))return structuredClone(await invoke(cmd,args));
   window.__test.calls.push({cmd,args});
   if(cmd==='get_subtitle_style_state')return structuredClone(state);
   if(cmd==='list_subtitle_fonts')return [{family:'Fixture Sans',aliases:['fixture'],face_count:2,monospaced:false},{family:'<unsafe>',aliases:[],face_count:1,monospaced:true}];
   if(cmd==='save_subtitle_styles'){state.styles=structuredClone(args.request.styles);return structuredClone(state);}
   const preview={output_path:'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZ1kAAAAASUVORK5CYII=',timestamp_ms:args.request.timestampMs,font_report:state.font_report,font_events:[`size=${args.request.styles.source.font_size}`]};
   if(window.__style.hold)return new Promise(resolve=>window.__style.pending.push(()=>resolve(preview)));
   return structuredClone(preview);
  };
 });
 await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();await page.locator('[data-editor-action="export"]').click();await page.locator('#edit-subtitle-styles').click();
 const dialog=page.locator('#subtitle-style-dialog'),size=dialog.locator('[data-style-editor="source"] [data-style-field="font_size"]');
 await page.waitForFunction(()=>document.querySelector('#subtitle-style-message').textContent.includes('当前样式已保存'));assert.equal(await size.inputValue(),'30');
 await page.evaluate(()=>new MutationObserver(()=>window.__style.applied.push(document.querySelector('#subtitle-font-events pre').textContent)).observe(document.querySelector('#subtitle-font-events pre'),{childList:true,subtree:true,characterData:true}));
 await size.fill('42');await page.waitForFunction(()=>document.querySelector('#subtitle-font-events pre').textContent==='size=42');assert.match(await page.locator('#subtitle-style-message').innerText(),/尚未保存/);
 await page.locator('#save-subtitle-style').click();await page.waitForFunction(()=>document.querySelector('#subtitle-style-message').textContent.includes('当前样式已保存'));assert.equal(await page.evaluate(()=>window.__style.state.styles.source.font_size),42);
 await page.evaluate(()=>window.__style.hold=true);await size.fill('45');await page.locator('#preview-subtitle-style').click();await page.waitForFunction(()=>window.__style.pending.length===1);await size.fill('46');
 await page.evaluate(()=>{window.__style.hold=false;window.__style.pending.shift()();});await page.waitForFunction(()=>document.querySelector('#subtitle-font-events pre').textContent==='size=46');assert.equal(await page.evaluate(()=>window.__style.applied.includes('size=45')),false);
 // A response from a closed visit cannot paint into a freshly opened one.
 await page.evaluate(()=>window.__style.hold=true);await page.locator('#preview-subtitle-style').click();await page.waitForFunction(()=>window.__style.pending.length===1);await page.locator('#close-subtitle-style').click();await page.evaluate(()=>window.__style.hold=false);await page.locator('#edit-subtitle-styles').click();await page.waitForFunction(()=>document.querySelector('#subtitle-font-events pre').textContent==='size=42');await page.evaluate(()=>window.__style.pending.shift()());
 await page.waitForFunction(()=>!document.querySelector('#preview-subtitle-style').disabled);assert.equal(await page.locator('#subtitle-font-events pre').textContent(),'size=42');
 assert.equal(await page.evaluate(()=>window.__test.calls.filter(r=>r.cmd==='list_subtitle_fonts').length),1);
 await page.keyboard.press('Escape');assert.equal(await dialog.evaluate(e=>e.open),false);await page.locator('#back-to-jobs').click();assert.equal(await page.locator('#karaoke-current-source').isVisible(),true);assert.deepEqual(errors,[]);
 console.log('PASS: initial load, debounce, save, saved status, superseded render suppression, close/reopen isolation, font cache and return');
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exitCode=1;});
