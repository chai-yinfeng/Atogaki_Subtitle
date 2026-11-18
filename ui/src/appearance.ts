import {convertFileSrc,invoke} from '@tauri-apps/api/core';
import {applyTheme,normalizeTheme,resolvedTheme,type AppTheme} from './themes';
import {paletteVariables,type ImagePalette,type ColorRoles,type RGB,hex} from './image-palette';
import {extractMaterialPalette,materialVariables} from './material-palette';
import {mountSheet} from './app-panels';

type ImageTheme={version:1;name:string;displayName:string;palette:ImagePalette;seed?:RGB;roles?:ColorRoles;template:AppTheme;fit:'contain'|'cover';strength:number;imageOpacity?:number;credit:string;source:string;enabled:boolean};
const KEY='atogaki.appearance.imageTheme';
const color=(value:unknown):value is RGB=>Array.isArray(value)&&value.length===3&&value.every(c=>typeof c==='number'&&Number.isFinite(c)&&c>=0&&c<=255);
export function readImageTheme():ImageTheme|null {
 try {
  const value=JSON.parse(localStorage.getItem(KEY)??'null');
  if(value?.version!==1||typeof value.name!=='string'||!/^[a-f0-9]{64}\.(png|jpg|webp)$/.test(value.name)||!color(value.palette?.dominant)||!color(value.palette?.accent)||!Array.isArray(value.palette.swatches)||!value.palette.swatches.every(color))return null;
  const roles:ColorRoles={};
  for(const role of ['ground','reading','accent'] as const)if(color(value.roles?.[role]))roles[role]=value.roles[role];
  return {...value,roles,seed:color(value.seed)?value.seed:roles.accent??value.palette.accent,template:normalizeTheme(value.template),enabled:value.enabled===true,displayName:String(value.displayName??''),credit:String(value.credit??''),source:String(value.source??''),fit:value.fit==='cover'?'cover':'contain',strength:Math.max(.15,Math.min(.85,Number(value.strength)||.45))};
 }catch{return null;}
}
const clone=(theme:ImageTheme|null):ImageTheme|null=>theme?structuredClone(theme):null;
const seed=(theme:ImageTheme)=>theme.seed??theme.roles?.accent??theme.palette.accent;
const imageOpacity=(theme:ImageTheme|null)=>Math.max(0,Math.min(100,Number.isFinite(theme?.imageOpacity)?theme!.imageOpacity!:35));
export function mountImageThemes(host:HTMLElement) {
 const shell=document.querySelector<HTMLElement>('.shell');
 let config=readImageTheme(),draft=clone(config),disposed=false,working=false,applying=false;
 let operation=0,application=0,renderGeneration=0;
 const events=new AbortController(),urls=new Set<string>(),images=new Set<HTMLImageElement>();
 const section=document.createElement('section');section.className='image-theme-editor';
 section.innerHTML='<div class="settings-section-heading"><div><strong>图片主题</strong><span>从本机图片生成整套配色。</span></div><button type="button" class="secondary" data-open-image-studio aria-haspopup="dialog" aria-controls="image-theme-studio">编辑图片主题</button></div><p class="image-theme-status" role="status"></p>';
 host.append(section);
 const studio=document.createElement('dialog');studio.id='image-theme-studio';studio.className='image-theme-studio';studio.setAttribute('aria-labelledby','image-studio-title');
 studio.innerHTML=`<div class="dialog-heading"><h2 id="image-studio-title">图片主题</h2><button type="button" class="secondary" data-cancel-image>取消</button></div>
 <div class="image-studio-body"><aside class="image-studio-controls">
 <button type="button" data-import-image>导入图片</button><img class="image-theme-preview hidden" alt="主题原图">
 <label>名称<input data-image-name maxlength="100" placeholder="图片主题名称"></label>
 <div><span class="settings-subheading">配色方案</span><div class="image-seeds"></div></div>
 <label>应用时的明暗<select data-image-template><option value="light">亮色</option><option value="dark">暗色</option><option value="system">跟随系统</option></select></label>
 <label>背景图片不透明度 <output data-image-opacity-value>35%</output><input data-image-opacity type="range" min="0" max="100" value="35"></label>
 <details><summary>图片与来源</summary><label>本机图片路径<input data-image-path placeholder="可选：粘贴完整路径"></label><button type="button" class="secondary" data-import-path>从路径导入</button><label>署名<input data-image-credit maxlength="300"></label><label>来源网页<input data-image-source maxlength="2000" placeholder="https://…"></label><button type="button" class="secondary" data-reveal-image>定位图片</button></details>
 <p class="image-studio-status" role="status"></p></aside>
 <section class="image-studio-previews" aria-label="亮暗主题预览">${['light','dark'].map(mode=>`<div><h3>${mode==='light'?'亮色':'暗色'}</h3><div class="theme-mini" data-preview-mode="${mode}"><aside>Atogaki<span>工作台</span><span class="selected">收听</span><span>在线字幕</span></aside><main><strong>正在收听</strong><section><span class="mini-muted">原文</span><p>言葉をたしかめながら、続きを聴く。</p><span class="mini-muted">译文</span><p>一边确认文字，一边继续收听。</p></section><div class="mini-toolbar"><span class="mini-primary">播放</span><span>节目列表</span></div><div class="mini-input">搜索字幕</div></main></div></div>`).join('')}<p class="preview-note">背景、文字、按钮与选中状态自动协调；仅预览，应用后生效。</p></section></div>
 <div class="image-studio-footer"><button type="button" class="secondary" data-reset-image>恢复基础主题</button><button type="button" data-apply-image>应用图片主题</button></div>`;
 document.body.append(studio);
 const find=<T extends HTMLElement>(query:string)=>studio.querySelector<T>(query)!;
 const status=section.querySelector<HTMLElement>('.image-theme-status')!;
 const message=(text:string,error=false)=>{if(disposed)return;status.textContent=text;find('.image-studio-status').textContent=text;status.classList.toggle('warning',error);};
 const sheet=mountSheet(studio,[section.querySelector<HTMLElement>('[data-open-image-studio]')!],find('[data-cancel-image]'));
 const footer=document.createElement('footer');footer.className='image-theme-credit hidden';document.body.append(footer);
 const invalidate=()=>{operation++;application++;renderGeneration++;};
 const save=()=>{try{localStorage.setItem(KEY,JSON.stringify(config));}catch{message('主题已应用，但偏好保存失败。',true);}};
 const clear=()=>{
  const keys=new Set([...Object.keys(paletteVariables({dominant:[0,0,0],accent:[0,0,0],swatches:[]})),...Object.keys(materialVariables([0,0,0],false))]);
  for(const key of keys)document.documentElement.style.removeProperty(key);
  shell?.style.removeProperty('--scene');shell?.style.removeProperty('--scene-scrim');delete document.documentElement.dataset.imageTheme;footer.replaceChildren();footer.classList.add('hidden');
 };
 const paint=(theme:ImageTheme)=>{
  for(const [key,value] of Object.entries(materialVariables(seed(theme),resolvedTheme(theme.template)==='dark')))document.documentElement.style.setProperty(key,value);
  document.documentElement.dataset.imageTheme='palette';
 };
 const credit=()=>{
  footer.replaceChildren();
  if(config?.credit){const text=document.createElement('span');text.textContent=config.credit;footer.append(text);}
  if(config?.source){const url=new URL(config.source);const link=document.createElement('a');link.href=url.href;link.textContent='图片来源';footer.append(link);}
  footer.classList.toggle('hidden',!config?.enabled||!footer.childElementCount);
 };
 const use=async(theme:ImageTheme):Promise<boolean>=>{
  const generation=++application;
  if(theme.source){const url=new URL(theme.source);if(!['http:','https:'].includes(url.protocol)||url.username||url.password)throw Error('来源请填写不含凭据的 HTTP / HTTPS 网页地址');}
  let path:string|null;
  try{path=await invoke<string|null>('load_theme_image',{name:theme.name});}catch(error){if(disposed||generation!==application)return false;throw error;}
  if(disposed||generation!==application)return false;
  if(!path)throw Error('本机图片副本已缺失，请重新导入');
  clear();applying=true;try{applyTheme(theme.template);}finally{applying=false;}
  config=clone(theme);config!.enabled=true;paint(config!);
  shell?.style.setProperty('--scene',`url(${JSON.stringify(convertFileSrc(path))})`);
  shell?.style.setProperty('--scene-scrim',`color-mix(in srgb,var(--background) ${100-imageOpacity(config)}%,transparent)`);
  credit();save();
  window.dispatchEvent(new Event('atogaki-image-theme-change'));return true;
 };
 const fields=()=>{
  if(!draft)return;
  draft.displayName=find<HTMLInputElement>('[data-image-name]').value.trim()||'图片主题';
  draft.template=normalizeTheme(find<HTMLSelectElement>('[data-image-template]').value);
  draft.credit=find<HTMLInputElement>('[data-image-credit]').value.trim();draft.source=find<HTMLInputElement>('[data-image-source]').value.trim();
 };
 const preview=()=>{
  find<HTMLButtonElement>('[data-apply-image]').disabled=!draft||working;
  const opacity=find<HTMLInputElement>('[data-image-opacity]');
  opacity.disabled=!draft||working;opacity.value=String(imageOpacity(draft));
  find('[data-image-opacity-value]').textContent=opacity.value+'%';
  for(const element of studio.querySelectorAll<HTMLElement>('[data-preview-mode]'))element.style.setProperty('--scene-scrim',`color-mix(in srgb,var(--background) ${100-imageOpacity(draft)}%,transparent)`);
  const seeds=find('.image-seeds');seeds.replaceChildren();
  if(!draft)return;
  for(const rgb of draft.palette.swatches){
   const button=document.createElement('button');button.type='button';button.className='image-seed';button.style.background=hex(rgb);button.setAttribute('aria-label','配色 '+hex(rgb));button.setAttribute('aria-pressed',String(hex(rgb)===hex(seed(draft))));
   button.onclick=()=>{if(draft){draft.seed=rgb;preview();}};seeds.append(button);
  }
  for(const element of studio.querySelectorAll<HTMLElement>('[data-preview-mode]'))for(const [key,value] of Object.entries(materialVariables(seed(draft),element.dataset.previewMode==='dark')))element.style.setProperty(key,value);
 };
 const render=async()=>{
  const generation=++renderGeneration,theme=draft;
  find<HTMLInputElement>('[data-image-name]').value=theme?.displayName??'';
  find<HTMLSelectElement>('[data-image-template]').value=theme?.template??'system';
  find<HTMLInputElement>('[data-image-credit]').value=theme?.credit??'';find<HTMLInputElement>('[data-image-source]').value=theme?.source??'';
  preview();
  if(!theme){find('.image-theme-preview').classList.add('hidden');for(const element of studio.querySelectorAll<HTMLElement>('[data-preview-mode]'))element.removeAttribute('style');return;}
  let path:string|null;
  try{path=await invoke<string|null>('load_theme_image',{name:theme.name});}catch(error){if(disposed||generation!==renderGeneration)return;throw error;}
  if(disposed||generation!==renderGeneration||draft!==theme)return;
  const image=find<HTMLImageElement>('.image-theme-preview');image.classList.toggle('hidden',!path);
  for(const element of studio.querySelectorAll<HTMLElement>('[data-preview-mode]')){
   if(path)element.style.setProperty('--scene',`url(${JSON.stringify(convertFileSrc(path))})`);
   else element.style.removeProperty('--scene');
  }
  if(path)image.src=convertFileSrc(path);
 };
 section.querySelector('[data-open-image-studio]')!.addEventListener('click',()=>{
  draft=clone(config);void render().catch(error=>message(String(error),true));
 },{signal:events.signal});
 studio.addEventListener('close',()=>{invalidate();draft=clone(config);},{signal:events.signal});
 const release=(url:string)=>{if(urls.delete(url))URL.revokeObjectURL(url);};
 const analyze=async(name:string,current:()=>boolean)=>{
  const bytes=await invoke<ArrayBuffer>('theme_image_bytes',{name});if(!current())return null;
  const url=URL.createObjectURL(new Blob([bytes]));urls.add(url);const image=new Image();images.add(image);
  try{
   image.src=url;await image.decode();if(!current())return null;
   const canvas=document.createElement('canvas');canvas.width=96;canvas.height=96;const context=canvas.getContext('2d',{willReadFrequently:true});if(!context)throw Error('无法分析图片颜色');
   context.drawImage(image,0,0,96,96);return extractMaterialPalette(context.getImageData(0,0,96,96).data);
  }finally{images.delete(image);image.src='';release(url);}
 };
 const importImage=async(path:string|null)=>{
  if(disposed||working)return;working=true;const generation=++operation;const current=()=>!disposed&&generation===operation;
  preview();message('正在本机提取配色…');
  try{
   const imported=await invoke<{name:string;display_name:string}|null>('import_theme_image',{path});if(!current())return;if(!imported){message('已取消导入。');return;}
   const palette=await analyze(imported.name,current);if(!palette||!current())return;
   draft={version:1,name:imported.name,displayName:imported.display_name,palette,seed:palette.accent,template:draft?.template??'system',imageOpacity:imageOpacity(draft),fit:'contain',strength:.45,credit:'',source:'',enabled:false};
   await render();if(current())message('配色已生成；预览后点击应用。');
  }catch(error){if(current())message('导入失败：'+String(error),true);}finally{if(!disposed){working=false;preview();}}
 };
 find('[data-import-image]').onclick=()=>void importImage(null);
 find('[data-import-path]').onclick=()=>{const path=find<HTMLInputElement>('[data-image-path]').value.trim();if(path)void importImage(path);};
 find('[data-apply-image]').onclick=()=>{if(!draft||working)return;fields();void use(clone(draft)!).then(applied=>{if(applied){message('已应用图片主题。');sheet.close();}}).catch(error=>message(String(error),true));};
 find('[data-reset-image]').onclick=()=>{invalidate();if(config)config.enabled=false;clear();save();message('已恢复基础主题，原图与配置保留。');sheet.close();};
 find('[data-reveal-image]').onclick=()=>{if(draft)void invoke('reveal_theme_image',{name:draft.name}).catch(error=>message(String(error),true));};
 studio.addEventListener('change',()=>{fields();preview();},{signal:events.signal});
 find<HTMLInputElement>('[data-image-opacity]').addEventListener('input',event=>{
  if(draft){draft.imageOpacity=(event.currentTarget as HTMLInputElement).valueAsNumber;preview();}
 },{signal:events.signal});
 host.querySelectorAll('[data-app-theme]').forEach(button=>button.addEventListener('click',()=>{invalidate();if(config)config.enabled=false;clear();save();},{signal:events.signal}));
 window.addEventListener('atogaki-theme-change',()=>{
  if(applying||!config?.enabled)return;
  if(document.documentElement.dataset.themePreference===config.template&&document.documentElement.dataset.imageTheme){paint(config);return;}
  invalidate();config.enabled=false;clear();save();
 },{signal:events.signal});
 const ready=(async()=>{if(config?.enabled)try{if(await use(config))message('已恢复图片配色。');}catch(error){if(!disposed)message(String(error),true);}})();
 return {ready,dispose(){
  if(disposed)return;disposed=true;invalidate();events.abort();sheet.dispose();
  for(const image of images)image.src='';images.clear();for(const url of [...urls])release(url);
  studio.querySelectorAll<HTMLElement>('*').forEach(node=>node.onclick=null);studio.remove();section.remove();footer.remove();
 }};
}
