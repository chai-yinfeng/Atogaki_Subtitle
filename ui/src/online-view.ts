import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {DraftPresentation, OnlineCaptions, activePhase, phaseLabel, type CaptionEvent, type OnlineSnapshot, type LiveEvent} from "./online-state";
import {readOnlinePreferences, saveOnlinePreferences} from "./online-preferences";
type Choice={id:string;name:string;ready:boolean};
type DraftChoice=Choice&{language:string};
type Choices={drafts:DraftChoice[];accelerationReady:boolean;languages:Choice[];finals:Choice[];runtimeReady:boolean;translationReady:boolean;translators:Choice[]};
type Glossary={id:string;name:string;source_language:string};

/** The main page and the independent subtitle overlay share one view implementation.
 * The Rust session owns capture; hiding this view never stops it. */
export function mountOnlineView(root:HTMLElement, options:{control:boolean;status?:(phase:string)=>void}) {
const control=options.control;
root.classList.add("online-panel");root.dataset.view=control?"control":"caption";
const events=new AbortController();let unlisten=()=>{};let poll:number|null=null;
root.innerHTML=`<section class="online-card">
<header ${control?"data-tauri-drag-region":""}><strong ${control?"data-tauri-drag-region":""}>${control?"在线字幕":"Atogaki · 在线字幕"}</strong><button id="online-hide" class="quiet" type="button">${control?"字幕窗":"隐藏"}</button></header>
<div class="online-layout">${control?`<aside class="session-sidebar"><section class="session-controls" aria-label="在线会话控制">
<div class="session-options"><fieldset class="recognition-mode"><legend>识别模式</legend><label><input type="radio" name="recognition-mode" value="one-pass" checked>One Pass</label><label><input type="radio" name="recognition-mode" value="two-pass">Two Pass</label></fieldset><label>原文语言<select id="online-language"></select></label><label class="check"><input type="checkbox" id="online-translate" checked>翻译为中文</label></div>
<div class="actions"><button id="online-start">开始</button><button id="online-stop" class="secondary">停止</button></div>
<p id="online-profile-status" class="help" role="status"></p>
</section>
<details id="online-settings" class="online-settings"><summary>识别与翻译设置</summary>
<div class="configuration">
<label id="online-draft-field">草稿识别<select id="online-draft-model"></select></label>
<label><span id="online-model-label">识别模型</span><select id="online-model"></select></label>
<label>翻译模型<select id="online-translator"></select></label>
<label class="wide">词表<select id="online-glossary"><option value="">不使用词表</option></select></label>
<label class="wide">场景提示（可选）<textarea id="online-scene" maxlength="1000" rows="2" placeholder="例如：个人电台，讲述过去的经历。只填写确定的信息。"></textarea></label>
<label class="check wide"><input type="checkbox" id="online-drafts" checked aria-describedby="online-drafts-help">提前翻译未完成的原文（译文可回改）</label>
<p id="online-drafts-help" class="help">关闭后等待原文和分组确认。</p>

<button id="online-refresh" class="secondary wide" type="button">刷新模型／词表</button>
</div></details>
<label class="check overlay-lock"><input type="checkbox" id="online-lock">锁定字幕窗（鼠标穿透）</label><p class="help">系统音频 · 不保存录音</p></aside>`:""}<div class="caption-workspace">
<p id="online-status" role="status">正在读取会话状态…</p>
<section id="source-track" class="track"><div class="track-heading"><span class="badge">原文</span><span id="source-state" class="track-state"></span></div><p id="source-text">等待语音…</p></section>
<section id="translation-track" class="track"><div class="track-heading"><span class="badge">译文</span><span id="translation-state" class="track-state"></span></div><p id="translation-text">等待翻译…</p></section>
${control?`<details class="timing-details"><summary>处理延迟</summary><p id="online-timing" class="help"></p></details>`:""}
</div></div></section>`;
const model=new OnlineCaptions();
const find=<T extends HTMLElement>(id:string)=>root.querySelector<T>("#"+id)!;
const set=(id:string,text:string)=>{const node=find(id);if(node.textContent!==text)node.textContent=text;};
let preferences=readOnlinePreferences();
let choices:Choices|null=null, busy=false, message="", disposed=false;
let pendingCaptionSession:string|null=null;
let captionLocked=false, lockBusy=false;
let captionVisible=false, captionBusy=false;
let displayedSession:string|null=null;
type PartNode={node:HTMLSpanElement;gate:DraftPresentation};
const trackNodes={source:new Map<string,PartNode>(),translation:new Map<string,PartNode>()};
function renderTrack(prefix:"source"|"translation",parts:LiveEvent[]):LiveEvent[] {
 const shown:LiveEvent[]=[];
 const container=find(`${prefix}-text`), nodes=trackNodes[prefix];
 const keys=new Set(parts.map(e=>e.group_id??e.segment_id!));
 const follow=container.scrollHeight-container.scrollTop-container.clientHeight<24;
 let changed=false;
 for(const [key,{node}] of nodes)if(!keys.has(key)){node.remove();nodes.delete(key);changed=true;}
 if(parts.length){
  if(!nodes.size)container.replaceChildren();
  let previous:Element|null=null;
  for(const e of parts){
   const key=e.group_id??e.segment_id!;
   let part=nodes.get(key);
   if(!part){part={node:document.createElement("span"),gate:new DraftPresentation()};part.node.dataset.captionId=key;nodes.set(key,part);}
   const displayed=part.gate.select(e,performance.now())!;
   shown.push(displayed);
   const text=(displayed.text??"")+" ";
   if(part.node.textContent!==text){part.node.textContent=text;changed=true;}
   part.node.classList.toggle("draft",displayed.kind.endsWith("draft"));
   part.node.classList.toggle("omitted",displayed.kind==="source_gap"||displayed.kind==="translation_failed");
   if(displayed.kind==="source_gap")part.node.setAttribute("aria-label","省略的未译原文");
   if((previous?previous.nextElementSibling:container.firstElementChild)!==part.node){container.insertBefore(part.node,previous?previous.nextElementSibling:container.firstElementChild);changed=true;}
   previous=part.node;
  }
 }else{set(`${prefix}-text`,prefix==="source"?"等待语音…":model.translationEnabled||!model.status.sessionId?"等待翻译…":"翻译已关闭");}
 set(`${prefix}-state`,shown.some(e=>e.kind==="translation_failed")?"部分翻译失败":shown.some(e=>e.kind.endsWith("draft"))?"含可回改草稿":shown.length?"已锁定":"");
 if(changed&&follow)container.scrollTop=container.scrollHeight;
 return shown;
}
function render():void {
 if(disposed)return;
 const state=model.status, t=model.tracks();
 options.status?.(state.phase);
 root.dataset.locked=String(captionLocked);
 if(control){const lock=find<HTMLInputElement>("online-lock");lock.disabled=lockBusy;if(!lockBusy)lock.checked=captionLocked;}
 if(control){const toggle=find<HTMLButtonElement>("online-hide");toggle.disabled=captionBusy;toggle.textContent=captionVisible?"隐藏字幕窗":"显示字幕窗";toggle.setAttribute("aria-pressed",String(captionVisible));}
 set("online-status",message||state.error||`${phaseLabel[state.phase]||state.phase}${model.translationFailures?" · 部分片段翻译失败，原文已保留":""}`);
 if(displayedSession!==state.sessionId){
  displayedSession=state.sessionId;
  for(const prefix of ["source","translation"] as const){trackNodes[prefix].clear();find(`${prefix}-text`).replaceChildren();}
 }
 const shownTranslations=renderTrack("translation",t.translationParts);
 renderTrack("source",model.tracks(shownTranslations).sourceParts);
 if(!control)return;
 if(state.phase==="running"&&pendingCaptionSession===state.sessionId){pendingCaptionSession=null;void invoke("open_online_captions").catch(e=>{message=String(e);render();});}
 const active=activePhase(state.phase);
 const mode=recognitionMode(), single=mode==="one-pass";
 const draft=choices?.drafts.find(x=>x.id===find<HTMLSelectElement>("online-draft-model").value&&x.language===find<HTMLSelectElement>("online-language").value);
 const target=choices?.finals.find(x=>x.id===find<HTMLSelectElement>("online-model").value);
 const ready=target?.ready&&(single?choices?.accelerationReady&&target.id==="qwen-1.7b-8bit":draft?.ready);
 find<HTMLButtonElement>("online-start").disabled=busy||active||!choices?.runtimeReady||!ready;
 find<HTMLButtonElement>("online-stop").disabled=busy||!active;
 for(const id of ["online-language","online-model","online-draft-model","online-translate","online-drafts","online-translator","online-glossary","online-scene","online-refresh"]) (find(id) as HTMLInputElement).disabled=active||busy;
 for(const input of root.querySelectorAll<HTMLInputElement>('input[name="recognition-mode"]'))input.disabled=active||busy;
 find("online-draft-field").hidden=single;
 set("online-model-label",single?"识别模型":"确认识别模型");
 set("online-profile-status",single?(choices?.accelerationReady?"Qwen3-ASR · DFlash":"需准备 Qwen3-ASR 1.7B 8-bit 和 DFlash"):draft?.ready?draft.name+" → Qwen3-ASR":"请准备所选草稿识别模型");
 const translationReady=!!choices?.translators.some(x=>x.id===find<HTMLSelectElement>("online-translator").value&&x.ready);
 find<HTMLInputElement>("online-translate").disabled=active||busy||!translationReady;
 const translating=find<HTMLInputElement>("online-translate").checked&&translationReady;
 find<HTMLSelectElement>("online-translator").disabled=active||busy||!find<HTMLInputElement>("online-translate").checked;
 find<HTMLInputElement>("online-drafts").disabled=active||busy||!translating;
 const timings: string[]=[];
 if(t.source?.kind==="source_final"&&t.source.after_audio_end_s!==undefined)timings.push(`原文定稿 ${t.source.after_audio_end_s.toFixed(2)}s`);
 if(t.translation?.kind==="translation_final"&&t.translation.after_audio_end_s!==undefined)timings.push(`译文距末段 ${t.translation.after_audio_end_s.toFixed(2)}s${t.translation.after_first_audio_end_s===undefined?"":`／距首段 ${t.translation.after_first_audio_end_s.toFixed(2)}s`}`);
 set("online-timing",timings.length?`${timings.join(" · ")}（从对应音频段结束计）`:"延迟随定稿更新；草稿出现速度与定稿等待分别观察。");

}
async function refresh():Promise<void>{try{const snapshot=await invoke<OnlineSnapshot>("online_snapshot");if(disposed)return;captionLocked=snapshot.captionLocked??false;captionVisible=snapshot.captionVisible??false;model.snapshot(snapshot);render();}catch(e){message=String(e);render();}}
function filterGlossaries():void {
 const select=find<HTMLSelectElement>("online-glossary"), language=find<HTMLSelectElement>("online-language").value;
 for(const option of select.options)option.disabled=!!option.value&&option.dataset.language!==language;
 if(select.selectedOptions[0]?.disabled)select.value="";
}
function recognitionMode(): "one-pass"|"two-pass" {
 return root.querySelector<HTMLInputElement>('input[name="recognition-mode"]:checked')?.value === "two-pass"?"two-pass":"one-pass";
}
function filterModels():void {
 const select=find<HTMLSelectElement>("online-model"),single=recognitionMode()==="one-pass";
 for(const option of select.options)option.disabled=!choices?.finals.find(x=>x.id===option.value)?.ready||(single&&option.value!=="qwen-1.7b-8bit");
 if(select.selectedOptions[0]?.disabled)select.value=[...select.options].find(x=>!x.disabled)?.value??"";
 const drafts=find<HTMLSelectElement>("online-draft-model"),previous=drafts.value||preferences.draftBackend;
 drafts.replaceChildren(...(choices?.drafts??[]).filter(x=>x.language===find<HTMLSelectElement>("online-language").value).map(c=>{
  const option=document.createElement("option");option.value=c.id;option.textContent=c.name+(c.ready?"":" · 未就绪");option.disabled=!c.ready;return option;
 }));
 drafts.value=[...drafts.options].some(x=>x.value===previous&&!x.disabled)?previous:[...drafts.options].find(x=>!x.disabled)?.value??"";
}
function remember():void {
 saveOnlinePreferences({recognitionMode:recognitionMode(),draftBackend:find<HTMLSelectElement>("online-draft-model").value==="zipformer"?"zipformer":"sensevoice",
  language:find<HTMLSelectElement>("online-language").value,finalModel:find<HTMLSelectElement>("online-model").value,
  translationBackend:find<HTMLSelectElement>("online-translator").value,translate:find<HTMLInputElement>("online-translate").checked,
  translationDrafts:find<HTMLInputElement>("online-drafts").checked});
}
async function loadChoices():Promise<void>{
 const [loaded,glossaries]=await Promise.all([invoke<Choices>("online_choices"),invoke<Glossary[]>("list_glossaries")]);
 if(disposed)return;
 choices=loaded;
 for(const [id,list,fallback] of [["online-language",choices.languages,preferences.language],["online-model",choices.finals,preferences.finalModel],["online-translator",choices.translators,preferences.translationBackend]] as const){
  const select=find<HTMLSelectElement>(id), previous=select.value||fallback;
  select.replaceChildren(...list.map(c=>{const option=document.createElement("option");option.value=c.id;option.textContent=c.name+(c.ready?"":" · 未就绪");option.disabled=!c.ready;return option;}));
  select.value=list.some(c=>c.id===previous&&c.ready)?previous:list.find(c=>c.ready)?.id??"";
 }
 const select=find<HTMLSelectElement>("online-glossary"), previous=select.value;
 const empty=document.createElement("option");empty.value="";empty.textContent="不使用词表";
 select.replaceChildren(empty,...glossaries.map(g=>{const option=document.createElement("option");option.value=g.id;option.textContent=g.name;option.dataset.language=g.source_language;return option;}));
 select.value=previous;
 if(!select.value)select.value="";
 filterGlossaries();filterModels();
}
async function mount():Promise<void>{
const release=await listen<CaptionEvent>("online-session-event",({payload})=>{model.accept(payload);render();});
if(disposed){release();return;}unlisten=release;
if(control){
 root.querySelector<HTMLInputElement>('input[name="recognition-mode"][value="'+preferences.recognitionMode+'"]')!.checked=true;
 find<HTMLInputElement>("online-translate").checked=preferences.translate;
 find<HTMLInputElement>("online-drafts").checked=preferences.translationDrafts;
 find("online-lock").onchange=async()=>{
  const locked=find<HTMLInputElement>("online-lock").checked;lockBusy=true;render();
  try{await invoke("set_online_caption_locked",{locked});captionLocked=locked;}catch(e){message=String(e);}finally{lockBusy=false;await refresh();}
 };
 try{
  await loadChoices();
  if(!choices?.translationReady)find<HTMLInputElement>("online-translate").checked=false;
  if(!choices?.runtimeReady)message="在线运行环境尚未准备好。";
 }catch(e){message=String(e);}
 if(disposed)return;
 find("online-start").onclick=async()=>{
  busy=true;message="";render();
  try{
   remember();
   await invoke("hide_online_captions");
   const status=await invoke<OnlineSnapshot["status"]>("start_online_session",{options:{recognitionMode:recognitionMode(),draftBackend:find<HTMLSelectElement>("online-draft-model").value,language:find<HTMLSelectElement>("online-language").value,finalModel:find<HTMLSelectElement>("online-model").value,translate:find<HTMLInputElement>("online-translate").checked,translationDrafts:find<HTMLInputElement>("online-drafts").checked,translationBackend:find<HTMLSelectElement>("online-translator").value,glossaryId:find<HTMLSelectElement>("online-glossary").value||null,scene:find<HTMLTextAreaElement>("online-scene").value}});
   model.snapshot({status,events:[]});pendingCaptionSession=status.sessionId;
  }catch(e){message=String(e);}finally{busy=false;await refresh();}
 };
 window.addEventListener("storage",event=>{
  if(event.key!=="atogaki.onlinePreferences"||busy||activePhase(model.status.phase))return;
  preferences=readOnlinePreferences();
  root.querySelector<HTMLInputElement>('input[name="recognition-mode"][value="'+preferences.recognitionMode+'"]')!.checked=true;
  find<HTMLSelectElement>("online-language").value=preferences.language;
  find<HTMLSelectElement>("online-model").value=preferences.finalModel;
  find<HTMLSelectElement>("online-draft-model").value=preferences.draftBackend;
  find<HTMLSelectElement>("online-translator").value=preferences.translationBackend;
  find<HTMLInputElement>("online-translate").checked=preferences.translate;
  find<HTMLInputElement>("online-drafts").checked=preferences.translationDrafts;
  filterModels();filterGlossaries();render();
 },{signal:events.signal});
 find("online-stop").onclick=async()=>{busy=true;message="";render();try{await invoke("stop_online_session");}catch(e){message=String(e);}finally{busy=false;await refresh();}};
 find("online-language").onchange=()=>{filterGlossaries();filterModels();remember();render();};
 for(const input of root.querySelectorAll<HTMLInputElement>('input[name="recognition-mode"]'))input.onchange=()=>{filterModels();remember();render();};
 find("online-model").onchange=()=>{remember();render();};
 find("online-draft-model").onchange=()=>{remember();render();};
 find("online-drafts").onchange=()=>remember();
 find("online-translator").onchange=()=>{remember();render();};
 find("online-translate").onchange=()=>{remember();render();};
 find("online-refresh").onclick=()=>void loadChoices().then(()=>{message="";render();}).catch(e=>{message=String(e);render();});
}
find("online-hide").onclick=async()=>{
 if(captionBusy)return;if(control)pendingCaptionSession=null;captionBusy=true;render();
 try{await invoke(control?"toggle_online_captions":"hide_online_captions");}catch(e){message=String(e);}finally{captionBusy=false;await refresh();}
};
if(disposed)return;
await refresh();
if(!disposed)poll=window.setInterval(()=>void refresh(),500);

}
const ready=mount().catch(error=>{message=String(error);render();});
return {ready, refresh, dispose(){disposed=true;events.abort();if(poll!==null)window.clearInterval(poll);unlisten();root.querySelectorAll<HTMLElement>("button,input,select,textarea").forEach(node=>{node.onclick=null;node.onchange=null;});}};
}
