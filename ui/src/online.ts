import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {OnlineCaptions, activePhase, phaseLabel, type CaptionEvent, type OnlineSnapshot} from "./online-state";
import "./online.css";

type Choice={id:string;name:string;ready:boolean};
type Choices={languages:Choice[];finals:Choice[];runtimeReady:boolean;translationReady:boolean};
const control=document.body.dataset.view==="control";
const root=document.querySelector<HTMLElement>("#online-root")!;
root.innerHTML=`<section class="online-card">
<header data-tauri-drag-region><strong data-tauri-drag-region>Atogaki · ${control?"在线字幕控制面板":"在线字幕"}</strong><button id="online-hide" class="quiet" type="button">${control?"字幕窗":"隐藏"}</button></header>
${control?`<div class="configuration"><label>原文语言<select id="online-language"></select></label><label>Qwen3-ASR 定稿<select id="online-model"></select></label><label class="check"><input type="checkbox" id="online-translate" checked>本地 Hy-MT2 翻译为中文</label><label class="check"><input type="checkbox" id="online-drafts" checked>显示可回改的译文草稿</label><p class="help">从系统输出读取音频，不使用麦克风。译文按较大上下文组回改，停顿约 3 秒后收口。原文与译文分别定稿。本次测试不保存录音。</p><div class="actions"><button id="online-start">开始</button><button id="online-stop" class="secondary">停止并完成末段</button></div></div>`:""}
<p id="online-status" role="status">正在读取会话状态…</p>
<section class="track"><span id="source-state" class="badge">原文</span><p id="source-text">等待语音…</p></section>
<section class="track"><span id="translation-state" class="badge">译文</span><p id="translation-text">等待翻译…</p></section>
${control?`<p id="online-timing" class="help"></p><details><summary>最近定稿</summary><div id="online-history"></div></details>`:""}
</section>`;
const model=new OnlineCaptions();
const find=<T extends HTMLElement>(id:string)=>document.getElementById(id) as T;
const set=(id:string,text:string)=>{find(id).textContent=text;};
let choices:Choices|null=null, busy=false, message="", disposed=false;
let pendingCaptionSession:string|null=null;
function render():void {
 const state=model.status, t=model.tracks();
 set("online-status",message||state.error||phaseLabel[state.phase]||state.phase);
 for(const [prefix,e,name] of [["source",t.source,"原文"],["translation",t.translation,"译文"]] as const){
  const draft=e?.kind.endsWith("draft")??false;
  set(`${prefix}-state`,`${name}${e?(draft?"草稿":"定稿"):""}${e?.stale_source?" · 正在追上原文":""}`);
  find(`${prefix}-text`).classList.toggle("draft",draft);
  set(`${prefix}-text`,e?.text||(prefix==="source"?"等待语音…":"暂无译文"));
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
 const history=find("online-history");history.replaceChildren();
 for(const e of t.history){const article=document.createElement("article");const src=document.createElement("p"),dst=document.createElement("p");src.textContent=e.source_text??"";dst.textContent=e.text??"";article.append(src,dst);history.append(article);}
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
