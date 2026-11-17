import {argbFromRgb, redFromArgb, greenFromArgb, blueFromArgb, hexFromArgb, QuantizerCelebi, Score, themeFromSourceColor} from '@material/material-color-utilities';
import type {ImagePalette, RGB} from './image-palette.ts';
const rgb=(value:number):RGB=>[redFromArgb(value),greenFromArgb(value),blueFromArgb(value)];
export function extractMaterialPalette(pixels:ArrayLike<number>):ImagePalette {
  const colors:number[]=[];
  for(let i=0;i+3<pixels.length;i+=4)if(pixels[i+3]>=128)colors.push(argbFromRgb(pixels[i],pixels[i+1],pixels[i+2]));
  if(!colors.length)throw Error('图片没有可提取的可见颜色');
  const population=QuantizerCelebi.quantize(colors,128);
  const dominant=[...population].sort((a,b)=>b[1]-a[1])[0][0];
  const candidates=Score.score(population,{desired:6,fallbackColorARGB:dominant});
  return {dominant:rgb(dominant),accent:rgb(candidates[0]),swatches:candidates.map(rgb)};
}
/** The image chooses a seed; fixed semantic roles choose tones, never random components. */
export function materialVariables(seed:RGB,dark:boolean):Record<string,string> {
  const theme=themeFromSourceColor(argbFromRgb(...seed)),scheme=dark?theme.schemes.dark:theme.schemes.light;
  const neutral=(tone:number)=>hexFromArgb(theme.palettes.neutral.tone(tone));
  const variant=(tone:number)=>hexFromArgb(theme.palettes.neutralVariant.tone(tone));
  return {
    '--background':neutral(dark?6:99),'--foreground':hexFromArgb(scheme.onSurface),
    '--card':neutral(dark?10:100),'--card-foreground':hexFromArgb(scheme.onSurface),
    '--popover':neutral(dark?12:100),'--popover-foreground':hexFromArgb(scheme.onSurface),
    '--primary':hexFromArgb(scheme.primary),'--primary-foreground':hexFromArgb(scheme.onPrimary),
    '--primary-hover':hexFromArgb(theme.palettes.primary.tone(dark?75:35)),
    '--secondary':neutral(dark?16:95),'--secondary-foreground':hexFromArgb(scheme.onSurface),
    '--muted':neutral(dark?16:95),'--muted-foreground':variant(dark?80:30),
    '--accent':hexFromArgb(scheme.primaryContainer),'--accent-foreground':hexFromArgb(scheme.onPrimaryContainer),
    '--accent-hover':hexFromArgb(theme.palettes.primary.tone(dark?35:85)),
    '--border':variant(dark?30:80),'--input':variant(dark?40:70),'--ring':hexFromArgb(scheme.primary),
    '--sidebar':neutral(dark?10:97),'--sidebar-foreground':hexFromArgb(scheme.onSurface),
    '--sidebar-accent':hexFromArgb(scheme.primaryContainer),'--sidebar-border':variant(dark?30:80),
    '--destructive':dark?'#f87171':'#dc2626',
    '--surface':'var(--card)','--surface-soft':'var(--muted)','--ink':'var(--foreground)','--line':'var(--border)',
    '--reading-fill':'var(--card)','--reading-wash':'var(--card)','--panel-tint':'var(--card)',
    '--page-ground':'var(--background)','--page-paper':'var(--background)',
    '--chrome-ink':'var(--foreground)','--chrome-muted':'var(--muted-foreground)',
    '--chrome-line':'var(--border)','--chrome-fill':'var(--secondary)','--panel-shadow':'none',
  };
}
