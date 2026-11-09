import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {DraftPresentation, OnlineCaptions, activePhase, phaseLabel, type CaptionEvent, type OnlineSnapshot, type LiveEvent} from "./online-state";
import "./online.css";

type Choice={id:string;name:string;ready:boolean};
type Choices={languages:Choice[];finals:Choice[];runtimeReady:boolean;translationReady:boolean;translators:Choice[]};
type Glossary={id:string;name:string;source_language:string};
const control=document.body.dataset.view==="control";
const root=document.querySelector<HTMLElement>("#online-root")!;
root.innerHTML=`<section class="online-card">
<header ${control?"data-tauri-drag-region":""}><strong ${control?"data-tauri-drag-region":""}>Atogaki · ${control?"在线字幕控制面板":"在线字幕"}</strong><button id="online-hide" class="quiet" type="button">${control?"字幕窗":"隐藏"}</button></header>
${control?`<div class="configuration"><label>原文语言<select id="online-language"></select></label><label>Qwen3-ASR 定稿<select id="online-model"></select></label><label class="check"><input type="checkbox" id="online-translate" checked>启用本地中文翻译</label><label>翻译模型<select id="online-translator"></select></label><label>词表（核心人名／术语）<select id="online-glossary"><option value="">不使用词表</option></select></label><label class="help">场景提示（可选，仅供翻译参考）<textarea id="online-scene" maxlength="1000" rows="2" placeholder="例如：个人电台，讲述过去的经历。只填写确定的信息。"></textarea></label><label class="check"><input type="checkbox" id="online-drafts" checked>显示可回改的译文草稿</label><p class="help">从系统输出读取音频，不使用麦克风。原译文按对应组共同保留，原文末尾追加最近未译内容；白色已锁定，蓝色仍可回改。约 3–5 秒为软阅读范围，长组完整保留。日语先显示草稿，再结合已定稿后文重译；长停顿或等待上限仍会提交未完句。本次测试不保存录音。</p><div class="actions"><button id="online-start">开始</button><button id="online-stop" class="secondary">停止并完成末段</button><button id="online-refresh" class="secondary">刷新模型／词表</button></div></div>`:""}
${control?`<label class="check overlay-lock"><input type="checkbox" id="online-lock">锁定字幕窗（鼠标穿透；在此取消锁定）</label>`:""}
<p id="online-status" role="status">正在读取会话状态…</p>
<section id="source-track" class="track"><span class="badge">原文</span><p id="source-text">等待语音…</p></section>
<section id="translation-track" class="track"><span class="badge">译文</span><p id="translation-text">等待翻译…</p></section>
${control?`<p id="online-timing" class="help"></p>`:""}
</section>`;
const model=new OnlineCaptions();
const find=<T extends HTMLElement>(id:string)=>document.getElementById(id) as T;
const set=(id:string,text:string)=>{const node=find(id);if(node.textContent!==text)node.textContent=text;};
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
 if(changed&&follow)container.scrollTop=container.scrollHeight;
 return shown;
}
function render():void {
 const state=model.status, t=model.tracks();
 document.body.dataset.locked=String(captionLocked);
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
 find<HTMLButtonElement>("online-start").disabled=busy||active||!choices?.runtimeReady||!choices.finals.find(x=>x.id===find<HTMLSelectElement>("online-model").value)?.ready||!choices.languages.find(x=>x.id===find<HTMLSelectElement>("online-language").value)?.ready;
 find<HTMLButtonElement>("online-stop").disabled=busy||!active;
 for(const id of ["online-language","online-model","online-translate","online-drafts","online-translator","online-glossary","online-scene","online-refresh"]) (find(id) as HTMLInputElement).disabled=active||busy;
 if(!active&&!choices?.translators.some(x=>x.id===find<HTMLSelectElement>("online-translator").value&&x.ready)) find<HTMLInputElement>("online-translate").disabled=true;
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
async function loadChoices():Promise<void>{
 const [loaded,glossaries]=await Promise.all([invoke<Choices>("online_choices"),invoke<Glossary[]>("list_glossaries")]);
 if(disposed)return;
 choices=loaded;
 for(const [id,list,fallback] of [["online-language",choices.languages,"ja"],["online-model",choices.finals,"qwen-1.7b"],["online-translator",choices.translators,"hy-mt2"]] as const){
  const select=find<HTMLSelectElement>(id), previous=select.value||fallback;
  select.replaceChildren(...list.map(c=>{const option=document.createElement("option");option.value=c.id;option.textContent=c.name+(c.ready?"":" · 请在模型与存储下载");option.disabled=!c.ready;return option;}));
  select.value=list.some(c=>c.id===previous&&c.ready)?previous:list.find(c=>c.ready)?.id??"";
 }
 const select=find<HTMLSelectElement>("online-glossary"), previous=select.value;
 const empty=document.createElement("option");empty.value="";empty.textContent="不使用词表";
 select.replaceChildren(empty,...glossaries.map(g=>{const option=document.createElement("option");option.value=g.id;option.textContent=g.name;option.dataset.language=g.source_language;return option;}));
 select.value=previous;
 if(!select.value)select.value="";
 filterGlossaries();
}
async function mount():Promise<void>{
const unlisten=await listen<CaptionEvent>("online-session-event",({payload})=>{model.accept(payload);render();});
if(control){
 find("online-lock").onchange=async()=>{
  const locked=find<HTMLInputElement>("online-lock").checked;lockBusy=true;render();
  try{await invoke("set_online_caption_locked",{locked});captionLocked=locked;}catch(e){message=String(e);}finally{lockBusy=false;await refresh();}
 };
 try{
  await loadChoices();
  if(!choices?.translationReady)find<HTMLInputElement>("online-translate").checked=false;
  if(!choices?.runtimeReady)message="在线运行环境尚未准备好。";
 }catch(e){message=String(e);}
 find("online-start").onclick=async()=>{
  busy=true;message="";render();
  try{
   await invoke("hide_online_captions");
   const status=await invoke<OnlineSnapshot["status"]>("start_online_session",{options:{language:find<HTMLSelectElement>("online-language").value,finalModel:find<HTMLSelectElement>("online-model").value,translate:find<HTMLInputElement>("online-translate").checked,translationDrafts:find<HTMLInputElement>("online-drafts").checked,translationBackend:find<HTMLSelectElement>("online-translator").value,glossaryId:find<HTMLSelectElement>("online-glossary").value||null,scene:find<HTMLTextAreaElement>("online-scene").value}});
   model.snapshot({status,events:[]});pendingCaptionSession=status.sessionId;
  }catch(e){message=String(e);}finally{busy=false;await refresh();}
 };
 find("online-stop").onclick=async()=>{busy=true;message="";render();try{await invoke("stop_online_session");}catch(e){message=String(e);}finally{busy=false;await refresh();}};
 find("online-language").onchange=()=>{filterGlossaries();render();};
 find("online-translator").onchange=()=>render();
 find("online-refresh").onclick=()=>void loadChoices().then(()=>{message="";render();}).catch(e=>{message=String(e);render();});
}
find("online-hide").onclick=async()=>{
 if(captionBusy)return;if(control)pendingCaptionSession=null;captionBusy=true;render();
 try{await invoke(control?"toggle_online_captions":"hide_online_captions");}catch(e){message=String(e);}finally{captionBusy=false;await refresh();}
};
await refresh();
const poll=setInterval(()=>void refresh(),500);
window.addEventListener("beforeunload",()=>{disposed=true;clearInterval(poll);unlisten();});

}
void mount().catch(error=>{message=String(error);render();});
