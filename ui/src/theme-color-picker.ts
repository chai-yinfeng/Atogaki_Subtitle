import { hex, luminance, paletteVariables, type RGB, type ImagePalette, type ColorRoles } from './image-palette';

type Role = keyof ColorRoles;
const roles: [Role,string][] = [['ground','场景底色'],['reading','阅读面色'],['accent','按钮强调色']];
const marks: Record<Role,string> = {ground:'底',reading:'面',accent:'钮'};
function colorName(color:RGB):string {
  const [r,g,b]=color.map(v=>v/255),max=Math.max(r,g,b),min=Math.min(r,g,b),d=max-min,l=(max+min)/2;
  const s=d===0?0:d/(1-Math.abs(2*l-1));
  if(s<.1)return l>.85?'近白色':l<.15?'近黑色':l>.65?'浅灰色':l<.35?'深灰色':'灰色';
  const h=((max===r?(g-b)/d:max===g?(b-r)/d+2:(r-g)/d+4)*60+360)%360;
  const hue=h<20?'红':h<45?'橙':h<65?'金':h<90?'黄绿':h<155?'绿':h<190?'青':h<255?'蓝':h<290?'紫':h<335?'紫红':'红';
  return `${l>.72?'浅':l<.32?'深':''}${hue}${s<.3?'灰':''}色`;
}

/** One palette serves all three roles. Hover/focus compares without saving. */
export function createThemeColorPicker(host:HTMLElement,onChange:(role:Role,color:RGB|undefined)=>void) {
  let palette:ImagePalette,chosen:ColorRoles={},active:Role='ground';
  const preview=document.createElement('div');preview.className='theme-color-preview';
  const roleHost=document.createElement('div');roleHost.className='theme-color-roles';roleHost.setAttribute('aria-label','配色用途');
  const choices=document.createElement('div');choices.className='theme-color-choices';choices.setAttribute('role','group');
  const auto=document.createElement('button');auto.type='button';auto.className='secondary theme-color-auto';auto.textContent='此用途恢复自动';
  host.replaceChildren(roleHost,preview,choices,auto);
  const base=(role:Role)=>chosen[role]??(role==='ground'?palette.dominant:role==='reading'?chosen.accent??palette.accent:palette.accent);
  const compare=(candidate:RGB|undefined)=>{
    const current=base(active),next=candidate??current;
    preview.replaceChildren();
    const brightness=luminance(next)-luminance(current);
    const difference=hex(next)===hex(current)?'与当前相同':brightness>.012?'比当前更亮':brightness<-.012?'比当前更暗':'与当前明暗接近';
    const label=document.createElement('p');label.textContent=roles.find(([role])=>role===active)![1]+' · '+(candidate?`${colorName(next)} · ${difference} · ${hex(next)}`:'指向颜色或用 Tab 聚焦，可与当前颜色比较；点击后应用。');
    const pair=document.createElement('div');pair.className='theme-color-pair';
    for(const [title,color] of [['当前',current],['待选',next]] as const){
      const cell=document.createElement('div'),swatch=document.createElement('span'),caption=document.createElement('small');
      swatch.style.background=hex(color);caption.textContent=title;cell.append(swatch,caption);pair.append(cell);
    }
    const sample=document.createElement('div');sample.className='theme-color-sample';
    const vars=paletteVariables(palette,.45,candidate?{...chosen,[active]:candidate}:chosen);
    sample.style.background=vars[active==='ground'?'--page-ground':'--reading-fill'];
    sample.style.color=vars[active==='ground'?'--chrome-ink':'--ink'];
    const text=document.createElement('span');text.textContent='阅读文字 Aa';
    const button=document.createElement('span');button.textContent='按钮';button.style.background=vars['--accent'];button.style.color='#fffaf1';sample.append(text,button);
    preview.append(label,pair,sample);
  };
  const render=()=>{
    if(!palette)return;
    const focused=host.contains(document.activeElement)?(document.activeElement as HTMLElement).dataset.colorFocus:undefined;
    roleHost.replaceChildren();choices.replaceChildren();
    for(const [role,label] of roles){
      const button=document.createElement('button');button.type='button';button.className='theme-color-role';button.dataset.colorFocus=role;
      button.setAttribute('aria-pressed',String(role===active));
      const dot=document.createElement('span');dot.style.background=hex(base(role));
      const title=document.createElement('strong');title.textContent=label;
      const value=document.createElement('small');value.textContent=chosen[role]?colorName(base(role)):'自动';button.append(dot,title,value);
      button.onclick=()=>{active=role;render();};roleHost.append(button);
    }
    const label=roles.find(([role])=>role===active)![1];choices.setAttribute('aria-label',`为${label}选择颜色`);
    const colors=[...palette.swatches];
    for(const color of Object.values(chosen))if(color&&!colors.some(c=>hex(c)===hex(color)))colors.push(color);
    colors.forEach((color,i)=>{
      const button=document.createElement('button');button.type='button';button.className='theme-color-choice';button.dataset.colorFocus=hex(color);
      button.setAttribute('aria-label',`${String(i+1).padStart(2,'0')} ${colorName(color)}，用于${label}`);
      button.setAttribute('aria-pressed',String(!!chosen[active]&&hex(chosen[active]!)===hex(color)));
      const swatch=document.createElement('span');swatch.className='theme-color-chip';swatch.style.background=hex(color);
      const caption=document.createElement('small');caption.textContent=String(i+1).padStart(2,'0');
      const uses=roles.filter(([role])=>chosen[role]&&hex(chosen[role]!)===hex(color)).map(([role])=>marks[role]);
      if(uses.length)caption.textContent+=` · ${uses.join('/')}`;
      button.append(swatch,caption);
      button.onpointerenter=()=>compare(color);button.onfocus=()=>compare(color);
      button.onpointerleave=()=>compare(undefined);button.onblur=()=>compare(undefined);
      button.onclick=()=>{chosen={...chosen,[active]:color};render();onChange(active,color);};choices.append(button);
    });
    auto.dataset.colorFocus='auto';auto.disabled=!chosen[active];auto.setAttribute('aria-label',`${label}恢复自动`);
    compare(undefined);
    if(focused)Array.from(host.querySelectorAll<HTMLElement>('[data-color-focus]')).find(el=>el.dataset.colorFocus===focused)?.focus({preventScroll:true});
  };
  auto.onclick=()=>{chosen={...chosen,[active]:undefined};render();onChange(active,undefined);};
  return {update:(next:ImagePalette,nextRoles:ColorRoles={})=>{palette=next;chosen={...nextRoles};render();}};
}
