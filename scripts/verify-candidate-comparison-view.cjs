const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));
  await page.evaluate(async()=>{
   const check=(condition,message)=>{if(!condition)throw Error(message);};
   const {mountCandidateComparisonView}=await import('/src/candidate-comparison-view.ts');
   const root=document.createElement('dialog');root.innerHTML='<p id="retranscription-preview-summary"></p><div class="retranscription-comparison"><div id="retranscription-original-list"></div><div id="retranscription-candidate-list"></div></div><p id="retranscription-confirm-message">old failure</p>';document.body.append(root);
   let opens=0,refreshes=0;
   const options={time:ms=>String(ms),escape:s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;'),refresh:()=>refreshes++,open:()=>opens++};
   const preview={start_ms:100,end_ms:200,original_segments:[{start_ms:100,end_ms:200,source_text:'原文 <img src=x onerror="window.injected=1">'}],candidate_segments:[{start_ms:110,end_ms:190,source_text:'候选 <script>bad</script>'}],provider_name:'<provider>',context_mode:'isolated',timing_granularities:['<word>'],configuration:{prompt:'<img src=x>'},workspace_revision:42};
   let view=mountCandidateComparisonView(root,options);view.show(preview);
   check(opens===1,'open once');check(root.querySelector('#retranscription-preview-summary').textContent.includes('前文文字已隔离'),'context summary');check(root.querySelector('#retranscription-preview-summary').textContent.includes('<provider>'),'provider text');
   check(root.querySelector('#retranscription-original-list p').textContent===preview.original_segments[0].source_text,'source literal');check(root.querySelector('#retranscription-candidate-list p').textContent===preview.candidate_segments[0].source_text,'candidate literal');check(!root.querySelector('img,script'),'untrusted text escaped');check(root.querySelector('.candidate-diff').textContent.includes('<word>'),'timing evidence literal');check(root.querySelector('.candidate-diff pre').textContent.includes('<img src=x>'),'configuration literal');check(root.querySelector('#retranscription-confirm-message').textContent==='','old failure cleared');
   const old=root.querySelector('.refresh-comparison');old.click();check(refreshes===1,'refresh callback');old.disabled=true;old.dispatchEvent(new MouseEvent('click',{bubbles:true}));check(refreshes===1,'disabled refresh guarded');old.disabled=false;
   view.show({...preview,original_segments:[],candidate_segments:[],workspace_revision:undefined,context_mode:undefined});check(root.querySelectorAll('.refresh-comparison').length===1&&root.querySelectorAll('.candidate-diff').length===1,'replaced comparison nodes');check(root.querySelector('#retranscription-original-list').textContent==='这个范围没有字幕。','empty source');check(root.querySelector('.candidate-diff').textContent.includes('历史候选，采用前需重新对照'),'legacy warning');old.click();check(refreshes===1,'detached button silent');
   const retained=root.querySelector('.refresh-comparison');view.dispose();view.dispose();retained.click();view.show(preview);check(opens===2&&refreshes===1,'disposed show and button silent');check(!root.querySelector('.refresh-comparison,.candidate-diff,article'),'owned nodes cleared');check(root.querySelector('.retranscription-comparison'),'base structure retained');
   view=mountCandidateComparisonView(root,options);view.show(preview);root.querySelector('.refresh-comparison').click();check(opens===3&&refreshes===2,'remount no duplicate');view.dispose();root.remove();
  });
  assert.deepEqual(errors,[]);console.log('PASS: candidate text/evidence escaping, summary and empty/legacy states, replacement, disabled/detached refresh, dispose and remount');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
