import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {DraftPresentation, OnlineCaptions, activePhase, phaseLabel, type CaptionEvent, type OnlineSnapshot, type LiveEvent} from "./online-state";
import "./online.css";

type Choice={id:string;name:string;ready:boolean};
type Choices={languages:Choice[];finals:Choice[];runtimeReady:boolean;translationReady:boolean};
const control=document.body.dataset.view==="control";
const root=document.querySelector<HTMLElement>("#online-root")!;
root.innerHTML=`<section class="online-card">
<header data-tauri-drag-region><strong data-tauri-drag-region>Atogaki · ${control?"在线字幕控制面板":"在线字幕"}</strong><button id="online-hide" class="quiet" type="button">${control?"字幕窗":"隐藏"}</button></header>
${control?`<div class="configuration"><label>原文语言<select id="online-language"></select></label><label>Qwen3-ASR 定稿<select id="online-model"></select></label><label class="check"><input type="checkbox" id="online-translate" checked>本地 Hy-MT2 翻译为中文</label><label class="check"><input type="checkbox" id="online-drafts" checked>显示可回改的译文草稿</label><p class="help">从系统输出读取音频，不使用麦克风。译文跨语音段重译，停顿约 1.2 秒或达到组上限后收口。白色区域保留最近三组双语定稿，蓝色区域显示可回改草稿。本次测试不保存录音。</p><div class="actions"><button id="online-start">开始</button><button id="online-stop" class="secondary">停止并完成末段</button></div></div>`:""}
<p id="online-status" role="status">正在读取会话状态…</p>
<section class="settled"><span class="badge">最近定稿 · 保留三组</span><p id="settled-empty" class="help">等待定稿…</p><div id="online-history"></div></section>
<div id="online-activity"><section id="source-track" class="track"><span id="source-state" class="badge">原文</span><p id="source-text">等待语音…</p></section>
<section id="translation-track" class="track"><span id="translation-state" class="badge">译文</span><p id="translation-text">等待翻译…</p></section>
</div>
${control?`<p id="online-timing" class="help"></p>`:""}
</section>`;
const model=new OnlineCaptions();
const find=<T extends HTMLElement>(id:string)=>document.getElementById(id) as T;
const set=(id:string,text:string)=>{const node=find(id);if(node.textContent!==text)node.textContent=text;};
let choices:Choices|null=null, busy=false, message="", disposed=false;
let pendingCaptionSession:string|null=null;
let displayedSession:string|null=null;
let sourceDraft=new DraftPresentation(), translationDraft=new DraftPresentation();
const settledNodes=new Map<string,HTMLElement>();
function renderHistory(history:LiveEvent[]):void {
 const container=find("online-history"), keys=new Set(history.map(e=>e.group_id??e.segment_id!));
 const follow=container.scrollHeight-container.scrollTop-container.clientHeight<24;
 let changed=false;
 for(const [key,node] of settledNodes)if(!keys.has(key)){node.remove();settledNodes.delete(key);changed=true;}
 for(const e of history){
  const key=e.group_id??e.segment_id!;
  if(settledNodes.has(key))continue;
  const article=document.createElement("article");article.dataset.captionId=key;
  const source=document.createElement("p");source.className="settled-source";source.textContent=e.source_text??e.text??"";article.append(source);
  if(e.group_id){const translation=document.createElement("p");translation.textContent=e.text??"";article.append(translation);}
  settledNodes.set(key,article);changed=true;
 }
 // Insert only newly arrived or out-of-order rows; unchanged cards keep their DOM.
 let previous:Element|null=null;
 for(const e of history){const node=settledNodes.get(e.group_id??e.segment_id!)!;
  if((previous?previous.nextElementSibling:container.firstElementChild)!==node)container.insertBefore(node,previous?previous.nextElementSibling:container.firstElementChild);
  previous=node;
 }
 find("settled-empty").hidden=history.length>0;
 if(changed&&follow)container.scrollTop=container.scrollHeight;
}
function render():void {
 const state=model.status, t=model.tracks();
 set("online-status",message||state.error||phaseLabel[state.phase]||state.phase);
 if(displayedSession!==state.sessionId){
  displayedSession=state.sessionId;sourceDraft=new DraftPresentation();translationDraft=new DraftPresentation();
  settledNodes.clear();find("online-history").replaceChildren();
 }
 renderHistory(t.history);
 const now=performance.now();
 const source=sourceDraft.select(t.activeSource,now), translation=translationDraft.select(t.activeTranslation,now);
 find("online-activity").hidden=!source&&!translation;
 for(const [prefix,e,name] of [["source",source,"原文"],["translation",translation,"译文"]] as const){
  find(`${prefix}-track`).hidden=!e;
  set(`${prefix}-state`,`${name}${e?.kind==="source_final"?"定稿 · 等待译文":"草稿"}${e?.stale_source?" · 正在追上原文":""}`);
  find(`${prefix}-text`).classList.toggle("draft",e?.kind.endsWith("draft")??false);
  set(`${prefix}-text`,e?.text??"");
 }
 if(!control)return;
 if(state.phase==="running"&&pendingCaptionSession===state.sessionId){pendingCaptionSession=null;void invoke("open_online_captions").catch(e=>{message=String(e);render();});}
 const active=activePhase(state.phase);
 find<HTMLButtonElement>("online-start").disabled=busy||active||!choices?.runtimeReady||!choices.finals.find(x=>x.id===find<HTMLSelectElement>("online-model").value)?.ready||!choices.languages.find(x=>x.id===find<HTMLSelectElement>("online-language").value)?.ready;
 find<HTMLButtonElement>("online-stop").disabled=busy||!active;
 for(const id of ["online-language","online-model","online-translate","online-drafts"]) (find(id) as HTMLInputElement).disabled=active||busy;
 if(!active&&!choices?.translationReady) find<HTMLInputElement>("online-translate").disabled=true;
 const timings: string[]=[];
 if(t.source?.kind==="source_final"&&t.source.after_audio_end_s!==undefined)timings.push(`原文定稿 ${t.source.after_audio_end_s.toFixed(2)}s`);
 if(t.translation?.kind==="translation_final"&&t.translation.after_audio_end_s!==undefined)timings.push(`译文定稿 ${t.translation.after_audio_end_s.toFixed(2)}s`);
 set("online-timing",timings.length?`${timings.join(" · ")}（各自音频段／组末尾之后）`:"延迟随定稿更新；草稿出现速度与定稿等待分别观察。");

}
async function refresh():Promise<void>{try{const snapshot=await invoke<OnlineSnapshot>("online_snapshot");if(disposed)return;model.snapshot(snapshot);render();}catch(e){message=String(e);render();}}
async function mount():Promise<void>{
const unlisten=await listen<CaptionEvent>("online-session-event",({payload})=>{model.accept(payload);render();});
if(control){
 try{
  choices=await invoke<Choices>("online_choices");
  for(const [id,list] of [["online-language",choices.languages],["online-model",choices.finals]] as const){
   const select=find<HTMLSelectElement>(id);for(const c of list){const option=document.createElement("option");option.value=c.id;option.textContent=c.name+(c.ready?"":" · 未就绪");option.disabled=!c.ready;select.append(option);}
  }
  find<HTMLSelectElement>("online-language").value="ja";
  if(choices.finals.some(x=>x.id==="qwen-1.7b"&&x.ready))find<HTMLSelectElement>("online-model").value="qwen-1.7b";
  if(!choices.translationReady)find<HTMLInputElement>("online-translate").checked=false;
  if(!choices.runtimeReady)message="在线运行环境尚未准备好。";
 }catch(e){message=String(e);}
 find("online-start").onclick=async()=>{
  busy=true;message="";render();
  try{
   await invoke("hide_online_captions");
   const status=await invoke<OnlineSnapshot["status"]>("start_online_session",{options:{language:find<HTMLSelectElement>("online-language").value,finalModel:find<HTMLSelectElement>("online-model").value,translate:find<HTMLInputElement>("online-translate").checked,translationDrafts:find<HTMLInputElement>("online-drafts").checked}});
   model.snapshot({status,events:[]});pendingCaptionSession=status.sessionId;
  }catch(e){message=String(e);}finally{busy=false;await refresh();}
 };
 find("online-stop").onclick=async()=>{busy=true;message="";render();try{await invoke("stop_online_session");}catch(e){message=String(e);}finally{busy=false;await refresh();}};
 find("online-language").onchange=()=>render();
}
find("online-hide").onclick=()=>void invoke(control?"open_online_captions":"hide_online_captions");
await refresh();
const poll=setInterval(()=>void refresh(),500);
window.addEventListener("beforeunload",()=>{disposed=true;clearInterval(poll);unlisten();});

}
void mount().catch(error=>{message=String(error);render();});
