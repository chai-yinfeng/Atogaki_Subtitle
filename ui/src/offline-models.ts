import type {DesktopSettings,ModelCatalogItem,ModelDownloadState} from './desktop-settings-types';
import type {RegisteredModel} from './custom-models';
export type OfflineModelChoice={id:string;name:string;path:string;provider:string;ready:boolean;reason:string};
/** Selection holds IDs; paths stay an internal compatibility input for old tasks. */
export function mountOfflineModels(root:HTMLFormElement, options:{choices():Promise<OfflineModelChoice[]>;vads?():Promise<RegisteredModel[]>;changed():void;message(text:string):void}) {
 const model=root.querySelector<HTMLSelectElement>('#recognition-model')!,vad=root.querySelector<HTMLSelectElement>('#vad-model')!;
 const modelPath=root.querySelector<HTMLInputElement>('#model-path')!,vadPath=root.querySelector<HTMLInputElement>('#vad-model-path')!,vadEnabled=root.querySelector<HTMLInputElement>('#vad-enabled')!;
 const events=new AbortController();let models:OfflineModelChoice[]=[],request=0,disposed=false;
 let preferred='',preferredVad='';try{preferred=localStorage.getItem('atogaki.offlineModelId')??'';preferredVad=localStorage.getItem('atogaki.offlineVadModelPath')??'';}catch{/* Optional preference. */}
 const apply=()=>{
  const choice=models.find(item=>item.id===model.value),qwen=choice?.provider==='mlx-qwen3-asr';
  modelPath.value=choice?.ready?choice.path:'';
  root.querySelector<HTMLElement>('.vad-setting')!.hidden=!!qwen;
  root.querySelector<HTMLElement>('.task-vad-model')!.hidden=!!qwen;
  vadEnabled.disabled=!!qwen;
  if(qwen)vadPath.required=false;
  options.changed();
 };
 const render=(next:OfflineModelChoice[])=>{
  models=next;const previous=model.value||preferred;
  model.replaceChildren();
  for(const choice of models){const node=new Option(choice.name+(choice.reason?' · '+choice.reason:''),choice.id);node.disabled=!choice.ready;model.add(node);}
  if(!models.some(item=>item.ready))model.add(new Option('请在设置中下载或配置识别模型',''));
  model.value=models.some(item=>item.id===previous&&item.ready)?previous:models.find(item=>item.ready)?.id??'';apply();
 };
 const api={
  get selection(){return models.find(item=>item.id===model.value);},
  async refresh(current:DesktopSettings,catalog:ModelCatalogItem[],downloads:ModelDownloadState[]){
   const generation=++request;
   try {
    const [choices,registered]=await Promise.all([options.choices(),options.vads?.()??Promise.resolve([])]);
    if(disposed||request!==generation)return;
    if(!Array.isArray(choices)||!Array.isArray(registered))throw Error('识别模型列表格式无效');
    const previousVad=vad.value||preferredVad||current.vadModelPath||'';vad.replaceChildren();
    const addVad=(name:string,path:string)=>{if(![...vad.options].some(option=>option.value===path))vad.add(new Option(name,path));};
    for(const item of registered.filter(item=>item.kind==='vad'&&item.ready))addVad(item.name,item.path);
    if(current.vadModelPath)addVad('已配置的 Silero VAD',current.vadModelPath);
    for(const item of catalog.filter(item=>item.kind==='vad')){const state=downloads.find(state=>state.modelId===item.id&&state.status==='done');if(state?.path)addVad(item.name,state.path);}
    if(!vad.options.length)vad.add(new Option('请在设置中准备 VAD 模型',''));
    vad.value=[...vad.options].some(item=>item.value===previousVad)?previousVad:vad.options[0].value;vadPath.value=vad.value;
    render(choices);
   }catch(error){if(!disposed&&request===generation)options.message('无法读取识别模型：'+String(error));}
  },
  dispose(){disposed=true;++request;events.abort();}
 };
 model.addEventListener('change',()=>{preferred=model.value;try{localStorage.setItem('atogaki.offlineModelId',preferred);}catch{/* Selection still works. */}apply();},{signal:events.signal});
 vad.addEventListener('change',()=>{vadPath.value=vad.value;preferredVad=vad.value;try{localStorage.setItem('atogaki.offlineVadModelPath',preferredVad);}catch{/* Selection still works. */}options.changed();},{signal:events.signal});
 return api;
}
