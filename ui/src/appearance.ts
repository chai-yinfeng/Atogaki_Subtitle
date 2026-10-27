import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { applyTheme, type AppTheme } from './themes';
import { createThemeColorPicker } from './theme-color-picker';
import { extractPalette, paletteVariables, type ImagePalette, type ColorRoles } from './image-palette';

type ImageTheme = {version:1; name:string; displayName:string; palette:ImagePalette; roles?:ColorRoles; template:AppTheme; fit:'contain'|'cover'; strength:number; credit:string; source:string; enabled:boolean};
const KEY='atogaki.appearance.imageTheme';
const validName=(name:unknown):name is string=>typeof name==='string'&&/^[a-f0-9]{64}\.(png|jpg|webp)$/.test(name);
function read():ImageTheme|null {
  try {
    const value=JSON.parse(localStorage.getItem(KEY)??'null');
    const color=(rgb:unknown)=>Array.isArray(rgb)&&rgb.length===3&&rgb.every(c=>typeof c==='number'&&Number.isFinite(c)&&c>=0&&c<=255);
    if(value?.version!==1||!validName(value.name)||!color(value.palette?.dominant)||!color(value.palette?.accent)||!Array.isArray(value.palette?.swatches)||!value.palette.swatches.every(color))return null;
    const roles:ColorRoles={};
    for(const role of ['ground','reading','accent'] as const) { if(color(value.roles?.[role]))roles[role]=value.roles[role]; }
    return {...value,roles,enabled:value.enabled===true,displayName:String(value.displayName??''),credit:String(value.credit??''),source:String(value.source??''),template:value.template==='letter'?'letter':'moonlight',fit:value.fit==='cover'?'cover':'contain',strength:Math.max(.15,Math.min(.85,Number(value.strength)||.45))};
  }catch{return null;}
}
export function mountImageThemes(host:HTMLElement) {
  let config=read(),applying=false,working=false,disposed=false;
  let applyGeneration=0,renderGeneration=0,operationGeneration=0;
  const events=new AbortController(),urls=new Set<string>(),images=new Set<HTMLImageElement>();
  const invalidate=()=>{applyGeneration++;operationGeneration++;};
  const section=document.createElement('section');section.className='image-theme-editor';
  section.innerHTML=`<h4>从图片生成主题</h4><p>图片与配色分析均留在本机。支持 PNG、JPEG、WebP，最大 25 MB。</p>
    <div class="image-theme-import"><button type="button" data-import-image>选择图片并生成</button><input data-image-path aria-label="主题图片路径" placeholder="也可粘贴本机图片完整路径"><button type="button" class="secondary" data-import-path>从路径导入</button></div>
    <p class="image-theme-status" role="status"></p>
    <div class="image-theme-controls hidden"><img class="image-theme-preview" alt="导入的主题原图预览"><div class="image-theme-fields">
      <div class="image-theme-palette-heading"><strong data-image-name></strong><button type="button" class="secondary" data-refresh-palette>重新提取配色</button></div><div class="image-role-colors"></div><p class="image-role-help">先选择用途，再从共享色盘选色；“底／面／钮”显示颜色已用于哪些位置，可重复使用。文字、边框、悬停与选中颜色自动派生，并调整明暗保证可读。警告与错误颜色保持独立。</p>
      <label>阅读面模板<select data-image-template><option value="moonlight">月夜 · 冷纸阅读面</option><option value="letter">来信 · 信笺阅读面</option></select></label>
      <label>背景显示<select data-image-fit><option value="contain">完整显示</option><option value="cover">铺满窗口（可能裁切）</option></select></label>
      <label>界面遮罩强度<input data-image-strength type="range" min="15" max="85" step="5"></label>
      <label>署名<input data-image-credit maxlength="300" placeholder="作者或版权说明（可选）"></label>
      <label>来源网页<input data-image-source maxlength="2000" placeholder="https://…（可选）"></label>
      <div class="image-theme-buttons"><button type="button" data-apply-image>使用图片主题</button><button type="button" class="secondary" data-reset-image>恢复内置主题</button><button type="button" class="secondary" data-reveal-image>定位图片</button></div>
    </div></div>`;
  host.querySelector('.theme-options')?.before(section);
  const find=<T extends HTMLElement>(selector:string)=>section.querySelector<T>(selector)!;
  const status=find<HTMLElement>('.image-theme-status');
  const creditFooter=document.createElement('footer');creditFooter.className='image-theme-credit hidden';document.body.append(creditFooter);
  const message=(value:string,error=false)=>{if(disposed)return;status.textContent=value;status.classList.toggle('warning',error);};
  const save=()=>{if(disposed)return;try{localStorage.setItem(KEY,JSON.stringify(config));}catch{message('主题已应用，但偏好保存失败；请检查本机存储空间。',true);}};
  const clear=()=>{
    for(const key of Object.keys(paletteVariables({dominant:[0,0,0],accent:[0,0,0],swatches:[]})))document.documentElement.style.removeProperty(key);
    document.documentElement.style.removeProperty('--scene');delete document.documentElement.dataset.imageTheme;
    creditFooter.replaceChildren();creditFooter.classList.add('hidden');
  };
  const refreshCredit=()=>{
    creditFooter.replaceChildren();
    if(config?.credit){const text=document.createElement('span');text.textContent=config.credit;creditFooter.append(text);}
    if(config?.source){try{const url=new URL(config.source);if(['http:','https:'].includes(url.protocol)&&!url.username&&!url.password){const link=document.createElement('a');link.href=url.href;link.textContent='图片来源';creditFooter.append(link);}}catch{/* Invalid draft URL is not made clickable. */}}
    creditFooter.classList.toggle('hidden',!config?.enabled||!creditFooter.childElementCount);
  };
  const use=async():Promise<boolean>=>{
    if(disposed||!config)return false;
    const theme=config,request=++applyGeneration;
    if(theme.source){
      const source=new URL(theme.source);
      if(!['http:','https:'].includes(source.protocol)||source.username||source.password)throw new Error('来源请填写不含凭据的 HTTP / HTTPS 网页地址');
    }
    let path:string|null;
    try{path=await invoke<string|null>('load_theme_image',{name:theme.name});}
    catch(error){if(disposed||request!==applyGeneration||config!==theme)return false;throw error;}
    if(disposed||request!==applyGeneration||config!==theme)return false;
    if(!path)throw new Error('本机图片副本已缺失，请重新导入');
    applying=true;try{applyTheme(theme.template);}finally{applying=false;}
    for(const [key,value] of Object.entries(paletteVariables(theme.palette,theme.strength,theme.roles)))document.documentElement.style.setProperty(key,value);
    document.documentElement.style.setProperty('--scene',`url(${JSON.stringify(convertFileSrc(path))})`);
    document.documentElement.dataset.imageTheme=theme.fit;
    theme.enabled=true;refreshCredit();save();
    window.dispatchEvent(new Event('atogaki-theme-change'));
    return true;
  };
  const fields=()=>{
    if(disposed||!config)return;
    config.template=find<HTMLSelectElement>('[data-image-template]').value as AppTheme;
    config.fit=find<HTMLSelectElement>('[data-image-fit]').value as 'contain'|'cover';
    config.strength=Number(find<HTMLInputElement>('[data-image-strength]').value)/100;
    config.credit=find<HTMLInputElement>('[data-image-credit]').value.trim();config.source=find<HTMLInputElement>('[data-image-source]').value.trim();
  };
  const colorPicker=createThemeColorPicker(find<HTMLElement>('.image-role-colors'),(role,color)=>{
    if(disposed||!config)return;
    config.roles={...config.roles,[role]:color};
    if(config.enabled)void use().then(applied=>{if(applied)message('配色已应用并保存。');}).catch(e=>message(String(e),true));else save();
  });
  const render=async():Promise<boolean>=>{
    if(disposed||!config)return false;
    const theme=config,request=++renderGeneration;
    find<HTMLElement>('.image-theme-controls').classList.remove('hidden');
    find<HTMLElement>('[data-image-name]').textContent=theme.displayName;
    find<HTMLSelectElement>('[data-image-template]').value=theme.template;
    find<HTMLSelectElement>('[data-image-fit]').value=theme.fit;
    find<HTMLInputElement>('[data-image-strength]').value=String(theme.strength*100);
    find<HTMLInputElement>('[data-image-credit]').value=theme.credit;find<HTMLInputElement>('[data-image-source]').value=theme.source;
    colorPicker.update(theme.palette,theme.roles);
    let path:string|null;
    try{path=await invoke<string|null>('load_theme_image',{name:theme.name});}
    catch(error){if(disposed||request!==renderGeneration||config!==theme)return false;throw error;}
    if(disposed||request!==renderGeneration||config!==theme)return false;
    if(path)find<HTMLImageElement>('.image-theme-preview').src=convertFileSrc(path);
    else throw new Error('图片副本缺失，请重新导入；已恢复内置主题');
    return true;
  };
  const releaseUrl=(url:string)=>{if(urls.delete(url))URL.revokeObjectURL(url);};
  const analyze=async(name:string,current:()=>boolean):Promise<ImagePalette|null>=>{
    const buffer=await invoke<ArrayBuffer>('theme_image_bytes',{name});
    if(!current())return null;
    const url=URL.createObjectURL(new Blob([buffer]));urls.add(url);
    const img=new Image();images.add(img);
    try{
      img.src=url;await img.decode();
      if(!current())return null;
      const canvas=document.createElement('canvas');canvas.width=96;canvas.height=96;
      const context=canvas.getContext('2d',{willReadFrequently:true});if(!context)throw new Error('无法分析图片颜色');
      context.drawImage(img,0,0,96,96);return extractPalette(context.getImageData(0,0,96,96).data);
    }finally{images.delete(img);img.src='';releaseUrl(url);}
  };
  find<HTMLButtonElement>('[data-refresh-palette]').onclick=()=>void(async()=>{
    if(disposed||working||!config)return;working=true;
    const theme=config,request=++operationGeneration;
    const current=()=>!disposed&&request===operationGeneration&&config===theme;
    try{
      const palette=await analyze(theme.name,current);if(!palette||!current())return;theme.palette=palette;
      // Keep explicitly chosen role colors, credits and template when updating choices.
      if(!await render()||!current())return;
      if(theme.enabled){if(!await use()||!current())return;}else save();
      message(`已提取 ${palette.swatches.length} 个代表色，保留现有选择、署名与模板。`);
    }catch(error){if(current())message(`配色提取失败：${String(error)}`,true);}finally{if(!disposed)working=false;}
  })();
  const importImage=async(path:string|null)=>{
    if(disposed||working)return;working=true;
    const request=++operationGeneration,current=()=>!disposed&&request===operationGeneration;
    section.querySelectorAll<HTMLButtonElement>('button').forEach(b=>b.disabled=true);message('正在本机读取图片与提取配色…');
    try{
      const imported=await invoke<{name:string;display_name:string;path:string}|null>('import_theme_image',{path});
      if(!current())return;
      if(!imported){message('已取消图片选择。');return;}
      const palette=await analyze(imported.name,current);if(!palette||!current())return;
      const old=config;
      config={version:1,name:imported.name,displayName:imported.display_name,palette,template:old?.template??'moonlight',fit:'contain',strength:.45,credit:'',source:'',enabled:true};
      if(!await render()||!current())return;
      if(await use()&&current())message('已从图片生成主题，自动保存到本机；字幕导出样式保持独立。');
    }catch(error){if(current())message(`导入失败：${String(error)}`,true);}
    finally{if(!disposed){working=false;section.querySelectorAll<HTMLButtonElement>('button').forEach(b=>b.disabled=false);}}
  };
  find<HTMLButtonElement>('[data-import-image]').onclick=()=>void importImage(null);
  find<HTMLButtonElement>('[data-import-path]').onclick=()=>{if(disposed)return;const path=find<HTMLInputElement>('[data-image-path]').value.trim();if(path)void importImage(path);else message('请填写本机图片完整路径。',true);};
  find<HTMLButtonElement>('[data-apply-image]').onclick=()=>{fields();void use().then(applied=>{if(applied)message('已应用并保存图片主题。');}).catch(e=>message(String(e),true));};
  find<HTMLButtonElement>('[data-reset-image]').onclick=()=>{if(disposed)return;invalidate();if(config)config.enabled=false;clear();save();message('已恢复内置主题，导入配置保留，可再次使用。');};
  find<HTMLButtonElement>('[data-reveal-image]').onclick=()=>{if(!disposed&&config)void invoke('reveal_theme_image',{name:config.name}).catch(e=>message(String(e),true));};
  section.querySelectorAll<HTMLInputElement|HTMLSelectElement>('.image-theme-fields input,.image-theme-fields select').forEach(control=>control.addEventListener('change',()=>{fields();if(config?.enabled)void use().catch(e=>message(String(e),true));else save();},{signal:events.signal}));
  window.addEventListener('atogaki-theme-change',()=>{
    if(applying||!config?.enabled)return;
    // Palette/canvas refreshes emitted by this editor do not disable its own theme.
    if(document.documentElement.dataset.imageTheme&&document.documentElement.dataset.theme===config.template)return;
    invalidate();config.enabled=false;clear();save();
  },{signal:events.signal});
  host.querySelectorAll<HTMLButtonElement>('[data-app-theme]').forEach(button=>button.addEventListener('click',()=>{invalidate();if(config)config.enabled=false;clear();save();},{signal:events.signal}));
  const ready=(async()=>{
    if(!config)return;
    const initial=config,operation=operationGeneration;
    let application=applyGeneration;
    const current=()=>!disposed&&config===initial&&application===applyGeneration&&operation===operationGeneration;
    try{
      if(!await render()||!current())return;
      if(initial.enabled){
        const pending=use();application=applyGeneration;
        if(await pending&&current())message('已恢复本机图片主题。');
      }else message('已保存图片主题，可点击“使用图片主题”启用。');
    }catch(error){if(!current())return;clear();message(String(error),true);}
  })();
  return {
    ready,
    dispose(){
      if(disposed)return;disposed=true;invalidate();renderGeneration++;events.abort();
      for(const image of images)image.src='';images.clear();
      for(const url of [...urls])releaseUrl(url);
      for(const element of section.querySelectorAll<HTMLElement>('*')){element.onclick=null;element.onpointerenter=null;element.onpointerleave=null;element.onfocus=null;element.onblur=null;}
      section.remove();creditFooter.remove();
    },
  };
}
