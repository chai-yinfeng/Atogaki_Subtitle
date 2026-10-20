export type RGB = [number, number, number];
export type ImagePalette = { dominant: RGB; accent: RGB; swatches: RGB[] };
export function hex(rgb: RGB): string { return '#'+rgb.map(c => Math.round(Math.max(0,Math.min(255,c))).toString(16).padStart(2,'0')).join(''); }
export function luminance(rgb: RGB): number {
  const linear = rgb.map(c => { const s=c/255;return s<=.04045?s/12.92:((s+.055)/1.055)**2.4; });
  return linear[0]*.2126+linear[1]*.7152+linear[2]*.0722;
}
export function contrast(a: RGB,b: RGB): number { const x=luminance(a),y=luminance(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05); }
function hsl([r,g,b]: RGB): RGB {
  r/=255;g/=255;b/=255;
  const max=Math.max(r,g,b),min=Math.min(r,g,b),delta=max-min,l=(max+min)/2;
  if (!delta) return [0,0,l];
  const s=delta/(1-Math.abs(2*l-1));
  const h=max===r?((g-b)/delta+6)%6:max===g?(b-r)/delta+2:(r-g)/delta+4;
  return [h*60,s,l];
}
function rgb([h,s,l]: RGB): RGB {
  const c=(1-Math.abs(2*l-1))*s,x=c*(1-Math.abs((h/60)%2-1)),m=l-c/2;
  const parts=h<60?[c,x,0]:h<120?[x,c,0]:h<180?[0,c,x]:h<240?[0,x,c]:h<300?[x,0,c]:[c,0,x];
  return parts.map(v=>Math.round((v+m)*255)) as RGB;
}
export function extractPalette(pixels: ArrayLike<number>): ImagePalette {
  const bins=new Map<number,{sum:RGB;count:number}>();
  for(let i=0;i+3<pixels.length;i+=4) {
    if(pixels[i+3]<128)continue;
    const r=pixels[i],g=pixels[i+1],b=pixels[i+2],key=(r>>4)<<8|(g>>4)<<4|(b>>4);
    const bin=bins.get(key)??{sum:[0,0,0] as RGB,count:0};
    bin.sum[0]+=r;bin.sum[1]+=g;bin.sum[2]+=b;bin.count++;bins.set(key,bin);
  }
  const ranked=[...bins.values()].sort((a,b)=>b.count-a.count);
  if(!ranked.length)throw new Error('图片没有可提取的可见颜色');
  const avg=(bin: typeof ranked[number])=>bin.sum.map(c=>c/bin.count) as RGB;
  const dominant=avg(ranked[0]);
  const total=ranked.reduce((n,b)=>n+b.count,0);
  const accents=ranked.filter(b=>b.count>=Math.max(1,total*.01)).sort((a,b)=>{
    const score=(bin:typeof a)=>{const [,s,l]=hsl(avg(bin));return Math.sqrt(bin.count)*(.15+s)*(l>.08&&l<.92?1:.15);};
    return score(b)-score(a);
  });
  return {dominant,accent:avg(accents[0]??ranked[0]),swatches:ranked.slice(0,6).map(avg)};
}
export function paletteVariables(palette: ImagePalette, strength=.45): Record<string,string> {
  const [h,s]=hsl(palette.accent),[groundH,groundS]=hsl(palette.dominant);
  const color=(light:number,sat=Math.min(s,.18),hue=h)=>rgb([hue,sat,light]);
  const surface=color(.93),soft=color(.86),ink=color(.14,Math.min(s,.25)),muted=color(.32,Math.min(s,.2));
  let accent=color(.34,Math.min(s,.52));
  for(let l=.33;contrast(accent,[255,250,241])<4.5&&l>=.08;l-=.01)accent=color(l,Math.min(s,.52));
  const ground=rgb([groundH,Math.min(groundS,.35),.17]);
  const alpha=Math.max(.15,Math.min(.85,strength));
  return {
    '--page-ground':hex(ground),'--surface':hex(surface),'--surface-soft':hex(soft),'--ink':hex(ink),'--muted':hex(muted),
    '--line':hex(color(.67)),'--accent':hex(accent),'--accent-hover':hex(color(.23,Math.min(s,.52))),
    '--accent-soft':hex(color(.81)),'--accent-soft-hover':hex(color(.74)),
    '--page-paper':`rgba(${ground.join(',')},${alpha})`,'--chrome-ink':'#f0f3e9','--chrome-muted':'#dce8df',
    '--chrome-line':'#dbe8e080','--chrome-fill':`rgba(${ground.join(',')},.9)`,
    '--panel-tint':hex(surface)+'f5','--reading-wash':hex(surface)+'e8','--reading-fill':hex(color(.97)),
    '--material-mark':hex(color(.53,Math.min(s,.35))),'--panel-shadow':'0 7px 24px #081b2529',
  };
}
