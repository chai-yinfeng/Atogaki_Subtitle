export type RegisteredModel={id:string;name:string;kind:string;path:string;ready:boolean};
type Options={invoke<T>(command:string,args:Record<string,unknown>):Promise<T>;visit():number;active():boolean;changed():void};
/** Named references to local assets. Removing a reference never removes the file. */
export function mountCustomModels(host:HTMLElement,options:Options) {
 const events=new AbortController();let disposed=false,busy=false,revision=0;
 host.innerHTML=`<div class="settings-section-heading"><strong>自定义模型</strong><span>保留原文件，只保存名称与位置。</span></div>
 <details class="custom-model-import"><summary>导入本机模型</summary><div class="custom-model-fields">
 <label>类型<select data-custom-kind><option value="whisper">Whisper</option><option value="qwen-asr">Qwen3-ASR</option><option value="vad">Silero VAD（GGML）</option></select></label>
 <label>名称<input data-custom-name maxlength="64" placeholder="例如：我的 Whisper Medium"></label>
 <label class="custom-model-path">模型文件或目录<input data-custom-path placeholder="选择模型，或粘贴完整路径"></label><button type="button" class="secondary" data-custom-pick>选择</button>
 <button type="button" data-custom-add>导入配置</button></div></details><p class="custom-model-message" role="status"></p><div class="custom-model-list"></div>`;
 const find=<T extends HTMLElement>(q:string)=>host.querySelector<T>(q)!;
 const message=(text:string,error=false)=>{if(disposed)return;find('.custom-model-message').textContent=text;find('.custom-model-message').classList.toggle('warning',error);};
 const lock=(value:boolean)=>{busy=value;host.querySelectorAll<HTMLButtonElement>('button').forEach(button=>button.disabled=value);};
 const changed=async(command:string,args:Record<string,unknown>)=>{
  if(disposed||busy)return;lock(true);const visit=options.visit();
  try{
   await options.invoke(command,args);if(disposed)return;options.changed();await refresh();
   if(visit===options.visit()&&options.active())message(command==='forget_registered_model'?'已移除配置，原文件保留。':'已保存模型配置。');
  }catch(error){if(!disposed&&visit===options.visit())message(String(error),true);}finally{if(!disposed)lock(false);}
 };
 const refresh=async()=>{
  const request=++revision;
  try{
   const models=await options.invoke<RegisteredModel[]>('registered_models',{});if(disposed||request!==revision)return;
   if(!Array.isArray(models))throw Error('模型列表格式无效');
   const list=find('.custom-model-list');list.replaceChildren();
   if(!models.length){const empty=document.createElement('p');empty.className='settings-message';empty.textContent='尚未导入自定义模型。';list.append(empty);}
   for(const model of models){
    const card=document.createElement('article');card.className='registered-model';card.dataset.registeredModel=model.id;
    const text=document.createElement('div'),title=document.createElement('strong'),detail=document.createElement('span');
    title.textContent=model.name;detail.textContent=(model.kind==='qwen-asr'?'Qwen3-ASR':model.kind==='vad'?'Silero VAD':'Whisper')+(model.ready?' · 已配置':' · 文件缺失或格式不完整');text.append(title,detail);
    const path=document.createElement('details'),summary=document.createElement('summary'),location=document.createElement('p');summary.textContent='文件位置';location.textContent=model.path;path.append(summary,location);text.append(path);
    const actions=document.createElement('div');actions.className='registered-model-actions';
    const rename=document.createElement('button');rename.type='button';rename.className='secondary';rename.textContent='改名';
    rename.onclick=()=>{
     if(busy)return;
     const input=document.createElement('input');input.value=model.name;input.maxLength=64;input.setAttribute('aria-label','新的模型名称');
     text.replaceChildren(input);const save=document.createElement('button'),cancel=document.createElement('button');save.type=cancel.type='button';save.textContent='保存';cancel.textContent='取消';cancel.className='secondary';
     save.onclick=()=>void changed('rename_registered_model',{id:model.id,name:input.value});cancel.onclick=()=>void refresh();
     input.onkeydown=event=>{if(event.key==='Enter'){event.preventDefault();save.click();}else if(event.key==='Escape'){event.preventDefault();event.stopPropagation();void refresh();}};
     actions.replaceChildren(save,cancel);input.focus();input.select();
    };
    const remove=document.createElement('button');remove.type='button';remove.className='secondary';remove.textContent='移除配置';remove.onclick=()=>void changed('forget_registered_model',{id:model.id});
    actions.append(rename,remove);card.append(text,actions);list.append(card);
   }
   if(busy)host.querySelectorAll<HTMLButtonElement>('button').forEach(button=>button.disabled=true);
  }catch(error){if(!disposed&&request===revision)message('无法读取自定义模型：'+String(error),true);}
 };
 find('[data-custom-pick]').onclick=async()=>{
  if(disposed||busy)return;lock(true);const visit=options.visit(),kind=find<HTMLSelectElement>('[data-custom-kind]').value,pathBefore=find<HTMLInputElement>('[data-custom-path]').value;
  try{
   const path=await options.invoke<string|null>('pick_registered_model',{kind});
   if(disposed||visit!==options.visit()||!options.active()||kind!==find<HTMLSelectElement>('[data-custom-kind]').value||pathBefore!==find<HTMLInputElement>('[data-custom-path]').value)return;
   if(path){find<HTMLInputElement>('[data-custom-path]').value=path;const name=find<HTMLInputElement>('[data-custom-name]');if(!name.value.trim())name.value=path.split(/[\\/]/).at(-1)?.replace(/\.bin$/,'')??'';message('填写名称后导入配置。');}
  }catch(error){if(!disposed&&visit===options.visit())message(String(error),true);}finally{if(!disposed)lock(false);}
 };
 find('[data-custom-add]').onclick=()=>void changed('register_model',{kind:find<HTMLSelectElement>('[data-custom-kind]').value,name:find<HTMLInputElement>('[data-custom-name]').value,path:find<HTMLInputElement>('[data-custom-path]').value.trim()});
 find('[data-custom-kind]').addEventListener('change',()=>{find<HTMLInputElement>('[data-custom-path]').value='';},{signal:events.signal});
 return {refresh,dispose(){disposed=true;revision++;events.abort();host.querySelectorAll<HTMLElement>('*').forEach(node=>{node.onclick=null;node.onkeydown=null;});host.replaceChildren();}};
}
