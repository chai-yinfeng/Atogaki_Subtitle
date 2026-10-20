import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { applyTheme, usePersonalMoonlightArtwork, type AppTheme } from './themes';
import { extractPalette, paletteVariables, hex, type ImagePalette } from './image-palette';

type ImageTheme = {version:1; name:string; displayName:string; palette:ImagePalette; template:AppTheme; fit:'contain'|'cover'; strength:number; credit:string; source:string; enabled:boolean};
const KEY='atogaki.appearance.imageTheme';
const validName=(name:unknown):name is string=>typeof name==='string'&&/^[a-f0-9]{64}\.(png|jpg|webp)$/.test(name);
function read():ImageTheme|null {
  try {
    const value=JSON.parse(localStorage.getItem(KEY)??'null');
    const color=(rgb:unknown)=>Array.isArray(rgb)&&rgb.length===3&&rgb.every(c=>typeof c==='number'&&Number.isFinite(c)&&c>=0&&c<=255);
    if(value?.version!==1||!validName(value.name)||!color(value.palette?.dominant)||!color(value.palette?.accent)||!Array.isArray(value.palette?.swatches)||!value.palette.swatches.every(color))return null;
    return {...value,enabled:value.enabled===true,displayName:String(value.displayName??''),credit:String(value.credit??''),source:String(value.source??''),template:value.template==='letter'?'letter':'moonlight',fit:value.fit==='cover'?'cover':'contain',strength:Math.max(.15,Math.min(.85,Number(value.strength)||.45))};
  }catch{return null;}
}
export async function setupImageThemes(host:HTMLElement):Promise<void> {
  let config=read(),applying=false,working=false;
  const section=document.createElement('section');section.className='image-theme-editor';
  section.innerHTML=`<h4>从图片生成主题</h4><p>图片与配色分析均留在本机。支持 PNG、JPEG、WebP，最大 25 MB。</p>
    <div class="image-theme-import"><button type="button" data-import-image>选择图片并生成</button><input data-image-path aria-label="主题图片路径" placeholder="也可粘贴本机图片完整路径"><button type="button" class="secondary" data-import-path>从路径导入</button></div>
    <p class="image-theme-status" role="status"></p>
    <div class="image-theme-controls hidden"><img class="image-theme-preview" alt="导入的主题原图预览"><div class="image-theme-fields">
      <strong data-image-name></strong><div class="image-palette" aria-label="提取的图片颜色"></div>
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
  const message=(value:string,error=false)=>{status.textContent=value;status.classList.toggle('warning',error);};
  const save=()=>{try{localStorage.setItem(KEY,JSON.stringify(config));}catch{message('主题已应用，但偏好保存失败；请检查本机存储空间。',true);}};
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
  const use=async()=>{
    if(!config)return;
    if(config.source){
      const source=new URL(config.source);
      if(!['http:','https:'].includes(source.protocol)||source.username||source.password)throw new Error('来源请填写不含凭据的 HTTP / HTTPS 网页地址');
    }
    const path=await invoke<string|null>('load_theme_image',{name:config.name});
    if(!path)throw new Error('本机图片副本已缺失，请重新导入');
    applying=true;try{applyTheme(config.template);}finally{applying=false;}
    usePersonalMoonlightArtwork(false);
    const legacy=host.querySelector<HTMLInputElement>('.personal-artwork-option input');if(legacy)legacy.checked=false;
    for(const [key,value] of Object.entries(paletteVariables(config.palette,config.strength)))document.documentElement.style.setProperty(key,value);
    document.documentElement.style.setProperty('--scene',`url(${JSON.stringify(convertFileSrc(path))})`);
    document.documentElement.dataset.imageTheme=config.fit;
    config.enabled=true;refreshCredit();save();
    window.dispatchEvent(new Event('atogaki-theme-change'));
  };
  const fields=()=>{
    if(!config)return;
    config.template=find<HTMLSelectElement>('[data-image-template]').value as AppTheme;
    config.fit=find<HTMLSelectElement>('[data-image-fit]').value as 'contain'|'cover';
    config.strength=Number(find<HTMLInputElement>('[data-image-strength]').value)/100;
    config.credit=find<HTMLInputElement>('[data-image-credit]').value.trim();config.source=find<HTMLInputElement>('[data-image-source]').value.trim();
  };
  const render=async()=>{
    if(!config)return;
    find<HTMLElement>('.image-theme-controls').classList.remove('hidden');
    find<HTMLElement>('[data-image-name]').textContent=config.displayName;
    find<HTMLSelectElement>('[data-image-template]').value=config.template;
    find<HTMLSelectElement>('[data-image-fit]').value=config.fit;
    find<HTMLInputElement>('[data-image-strength]').value=String(config.strength*100);
    find<HTMLInputElement>('[data-image-credit]').value=config.credit;find<HTMLInputElement>('[data-image-source]').value=config.source;
    const palette=find<HTMLElement>('.image-palette');palette.replaceChildren();
    for(const color of config.palette.swatches){const swatch=document.createElement('span');swatch.style.backgroundColor=hex(color);swatch.title=hex(color);palette.append(swatch);}
    const path=await invoke<string|null>('load_theme_image',{name:config.name});
    if(path)find<HTMLImageElement>('.image-theme-preview').src=convertFileSrc(path);
    else throw new Error('图片副本缺失，请重新导入；已恢复内置主题');
  };
  const importImage=async(path:string|null)=>{
    if(working)return;working=true;section.querySelectorAll<HTMLButtonElement>('button').forEach(b=>b.disabled=true);message('正在本机读取图片与提取配色…');
    try{
      const imported=await invoke<{name:string;display_name:string;path:string}|null>('import_theme_image',{path});
      if(!imported){message('已取消图片选择。');return;}
      const buffer=await invoke<ArrayBuffer>('theme_image_bytes',{name:imported.name});
      const url=URL.createObjectURL(new Blob([buffer]));
      let palette:ImagePalette;
      try{
        const img=new Image();img.src=url;await img.decode();
        const canvas=document.createElement('canvas');canvas.width=96;canvas.height=96;
        const context=canvas.getContext('2d',{willReadFrequently:true});if(!context)throw new Error('无法分析图片颜色');
        context.drawImage(img,0,0,96,96);palette=extractPalette(context.getImageData(0,0,96,96).data);
      }finally{URL.revokeObjectURL(url);}
      const old=config;
      config={version:1,name:imported.name,displayName:imported.display_name,palette,template:old?.template??'moonlight',fit:'contain',strength:.45,credit:'',source:'',enabled:true};
      await render();await use();message('已从图片生成主题，自动保存到本机；字幕导出样式保持独立。');
    }catch(error){message(`导入失败：${String(error)}`,true);}
    finally{working=false;section.querySelectorAll<HTMLButtonElement>('button').forEach(b=>b.disabled=false);}
  };
  find<HTMLButtonElement>('[data-import-image]').onclick=()=>void importImage(null);
  find<HTMLButtonElement>('[data-import-path]').onclick=()=>{const path=find<HTMLInputElement>('[data-image-path]').value.trim();if(path)void importImage(path);else message('请填写本机图片完整路径。',true);};
  find<HTMLButtonElement>('[data-apply-image]').onclick=()=>{fields();void use().then(()=>message('已应用并保存图片主题。')).catch(e=>message(String(e),true));};
  find<HTMLButtonElement>('[data-reset-image]').onclick=()=>{if(config)config.enabled=false;clear();save();message('已恢复内置主题，导入配置保留，可再次使用。');};
  find<HTMLButtonElement>('[data-reveal-image]').onclick=()=>{if(config)void invoke('reveal_theme_image',{name:config.name}).catch(e=>message(String(e),true));};
  section.querySelectorAll<HTMLInputElement|HTMLSelectElement>('.image-theme-fields input,.image-theme-fields select').forEach(control=>control.addEventListener('change',()=>{fields();if(config?.enabled)void use().catch(e=>message(String(e),true));else save();}));
  window.addEventListener('atogaki-theme-change',()=>{
    if(applying||!config?.enabled)return;
    // Palette/canvas refreshes emitted by this editor do not disable its own theme.
    if(document.documentElement.dataset.imageTheme&&document.documentElement.dataset.theme===config.template)return;
    config.enabled=false;clear();save();
  });
  host.querySelectorAll<HTMLButtonElement>('[data-app-theme]').forEach(button=>button.addEventListener('click',()=>{if(config)config.enabled=false;clear();save();}));
  host.addEventListener('change',event=>{if((event.target as HTMLElement).closest('.personal-artwork-option')){if(config)config.enabled=false;clear();save();}});
  if(config){try{await render();if(config.enabled){await use();message('已恢复本机图片主题。');}else message('已保存图片主题，可点击“使用图片主题”启用。');}catch(error){clear();message(String(error),true);}}
}
