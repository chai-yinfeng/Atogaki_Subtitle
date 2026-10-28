const {webkit}=require(process.env.ATOGAKI_PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await webkit.launch({headless:true});
 try {
  const page=await browser.newPage({viewport:{width:1180,height:760}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await require('./editor-ui-fixture.cjs').installEditorFixture(page,30);
  await page.goto(process.env.ATOGAKI_EDITOR_URL||'http://127.0.0.1:1430');await page.locator('.job-open').first().click();
  await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio')?.readyState>=1);
  await page.locator('#karaoke-playback-rate').selectOption('1.5');assert.equal(await page.locator('#karaoke-media-host audio').evaluate(a=>a.playbackRate),1.5);
  await page.locator('#karaoke-step-forward').click();await page.waitForFunction(()=>document.querySelector('#karaoke-media-host audio').currentTime>=.099);
  await page.locator('#karaoke-waveform-gain').evaluate(e=>{e.value='2.5';e.dispatchEvent(new Event('input',{bubbles:true}));});assert.equal(await page.locator('#karaoke-waveform-gain-label').textContent(),'2.50×');
  await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));
  const result=await page.evaluate(async()=>{
   const assert=(condition,message)=>{if(!condition)throw Error(message);};
   const {mountEditorControls}=await import('/src/editor-controls.ts');
   const {mountPlaybackShortcutView}=await import('/src/playback-shortcut-view.ts');
   const root=document.createElement('section');root.innerHTML='<input id="editor-search"><select id="editor-filter"><option value="all">all</option><option value="stale">stale</option></select><button data-editor-action="follow"><span>follow</span></button><select id="karaoke-playback-rate"><option value="1">1</option><option value="1.5">1.5</option></select><input id="karaoke-waveform-gain" value="1"><output id="karaoke-waveform-gain-label"></output><div id="key-target" tabindex="0"></div>';
   const bindings={'karaoke-toggle-playback':'togglePlayback','karaoke-cut-segment':'cut','karaoke-join-segment':'join','karaoke-insert-block':'insert','karaoke-delete-block':'remove','undo-subtitle-structure':'undo','karaoke-open-workspace':'tools'};
   const steps={'karaoke-step-back-small':-10,'karaoke-step-back':-100,'karaoke-step-forward':100,'karaoke-step-forward-small':10};
   for(const id of [...Object.keys(bindings),...Object.keys(steps)]){const button=document.createElement('button');button.id=id;root.append(button);}document.body.append(root);
   const events=[],record=name=>value=>events.push([name,value]);
   const options=Object.fromEntries(['query','filter','action','rate','step','gain',...Object.values(bindings)].map(name=>[name,record(name)]));
   let view=mountEditorControls(root,options);
   const input=(id,value,type)=>{const field=root.querySelector('#'+id);field.value=value;field.dispatchEvent(new Event(type,{bubbles:true}));};
   input('editor-search','needle','input');input('editor-filter','stale','change');input('karaoke-playback-rate','1.5','change');input('karaoke-waveform-gain','2.5','input');root.querySelector('[data-editor-action] span').click();
   for(const id of [...Object.keys(bindings),...Object.keys(steps)])root.querySelector('#'+id).click();
   assert(events.filter(e=>e[0]==='action'&&e[1]==='follow').length===1,'nested menu action exactly once');assert(events.some(e=>e[0]==='query'&&e[1]==='needle'),'search');assert(events.some(e=>e[0]==='filter'&&e[1]==='stale'),'filter');assert(events.some(e=>e[0]==='rate'&&e[1]===1.5),'rate');assert(events.some(e=>e[0]==='gain'&&e[1]===2.5),'gain');
   assert(JSON.stringify(events.filter(e=>e[0]==='step').map(e=>e[1]))===JSON.stringify(Object.values(steps)),'step values');for(const name of Object.values(bindings))assert(events.filter(e=>e[0]===name).length===1,'button '+name);
   view.dispose();view.dispose();const length=events.length;root.querySelector('[data-editor-action]').click();input('editor-search','disposed','input');input('karaoke-waveform-gain','3','input');for(const id of Object.keys(bindings))root.querySelector('#'+id).click();assert(events.length===length,'disposed controls silent');assert(root.querySelector('#karaoke-waveform-gain-label').value==='2.50×','disposed presentation silent');
   view=mountEditorControls(root,options);root.querySelector('[data-editor-action]').click();assert(events.length===length+1,'remount must not double dispatch');view.dispose();
   const state={enabled:true,editor:true,blocked:false},keys=[];
   const shortcuts=mountPlaybackShortcutView(document,{enabled:()=>state.enabled,editor:()=>state.editor,blocked:()=>state.blocked,cut:()=>keys.push(['cut']),step:ms=>keys.push(['step',ms]),playback:action=>keys.push(['playback',action])});
   const send=(key,extra={},target=root.querySelector('#key-target'))=>{const event=new KeyboardEvent('keydown',{key,bubbles:true,cancelable:true,...extra});target.dispatchEvent(event);return event.defaultPrevented;};
   assert(send('ArrowLeft'),'left');assert(send('ArrowRight',{shiftKey:true}),'fine right');assert(send('b',{metaKey:true}),'mac cut');assert(send('b',{ctrlKey:true}),'ctrl cut');assert(send(' ',{code:'Space'}),'toggle');assert(!send(' ',{code:'Space',repeat:true}),'no toggle repeat');assert(!send('o',{repeat:true}),'no overlay repeat');
   for(const tag of ['input','textarea','select','button']){const element=document.createElement(tag);root.append(element);const before=keys.length;assert(!send('ArrowLeft',{},element)&&keys.length===before,'text target '+tag);}
   const editable=document.createElement('div');editable.contentEditable='true';editable.innerHTML='<span>edit</span>';root.append(editable);const separator=document.createElement('div');separator.setAttribute('role','separator');root.append(separator);assert(!send(' ',{code:'Space'},editable.firstChild),'contenteditable');assert(!send('ArrowLeft',{},separator),'separator');
   const dialog=document.createElement('dialog');dialog.open=true;document.body.append(dialog);assert(!send(' ',{code:'Space'}),'dialog');dialog.remove();state.blocked=true;assert(!send(' ',{code:'Space'}),'task tools');state.blocked=false;assert(!send('ArrowLeft',{altKey:true}),'alt');assert(!send('ArrowLeft',{ctrlKey:true}),'modifier');state.enabled=false;assert(!send('ArrowLeft'),'disabled');state.enabled=true;state.editor=false;assert(send('ArrowLeft'),'listening mapping');
   assert(JSON.stringify(keys)===JSON.stringify([['step',-100],['step',10],['cut'],['cut'],['playback','toggle-playback'],['playback','rewind']]),'shortcut routing');shortcuts.dispose();const count=keys.length;assert(!send(' ',{code:'Space'})&&keys.length===count,'disposed keyboard silent');root.remove();return {controls:true,shortcuts:true};
  });
  assert.deepEqual(result,{controls:true,shortcuts:true});assert.deepEqual(errors,[]);
  console.log('PASS: real editor rate/step/gain, scoped control dispatch/dispose/remount, keyboard input/modal/tool guards and document listener cleanup');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
