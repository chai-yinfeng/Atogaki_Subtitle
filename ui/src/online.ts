import "./online.css";
import {applyTheme, readTheme, resolvedTheme} from "./themes";
import {readImageTheme} from "./appearance";
import {materialVariables} from "./material-palette";
let paletteKeys:string[]=[];
function restoreAppearance():void {
 for(const key of paletteKeys)document.documentElement.style.removeProperty(key);
 applyTheme(readTheme());
 const theme=readImageTheme();
 if(theme?.enabled){
  const palette=materialVariables(theme.seed??theme.roles?.accent??theme.palette.accent,resolvedTheme(readTheme())==="dark");
  paletteKeys=Object.keys(palette);
  for(const [key,value] of Object.entries(palette))document.documentElement.style.setProperty(key,value);
 }else paletteKeys=[];
}
restoreAppearance();
window.addEventListener("storage",event=>{if(event.key?.startsWith("atogaki.appearance."))restoreAppearance();});
window.addEventListener("atogaki-theme-change",()=>{
 const theme=readImageTheme();if(!theme?.enabled)return;
 for(const [key,value] of Object.entries(materialVariables(theme.seed??theme.roles?.accent??theme.palette.accent,resolvedTheme(readTheme())==="dark")))document.documentElement.style.setProperty(key,value);
});


import {mountOnlineView} from "./online-view";
const panel=mountOnlineView(document.querySelector<HTMLElement>("#online-root")!,{control:document.body.dataset.view==="control"});
window.addEventListener("beforeunload",()=>panel.dispose(),{once:true});
