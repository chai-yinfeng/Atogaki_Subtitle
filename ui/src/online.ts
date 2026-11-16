import "./online.css";
import {applyTheme, readTheme} from "./themes";
import {readImageTheme} from "./appearance";
import {paletteVariables} from "./image-palette";
let paletteKeys:string[]=[];
function restoreAppearance():void {
 for(const key of paletteKeys)document.documentElement.style.removeProperty(key);
 applyTheme(readTheme());
 const theme=readImageTheme();
 if(theme?.enabled){
  const palette=paletteVariables(theme.palette,theme.strength,theme.roles);
  paletteKeys=Object.keys(palette);
  for(const [key,value] of Object.entries(palette))document.documentElement.style.setProperty(key,value);
 }else paletteKeys=[];
}
restoreAppearance();
window.addEventListener("storage",event=>{if(event.key?.startsWith("atogaki.appearance."))restoreAppearance();});

import {mountOnlineView} from "./online-view";
const panel=mountOnlineView(document.querySelector<HTMLElement>("#online-root")!,{control:document.body.dataset.view==="control"});
window.addEventListener("beforeunload",()=>panel.dispose(),{once:true});
