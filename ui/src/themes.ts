export type AppTheme = "light" | "dark" | "system";
export function normalizeTheme(value: string | null): AppTheme {
  return value === "dark" || value === "system" ? value : "light";
}
export function readTheme(): AppTheme {
  try {
    if (!localStorage.getItem("atogaki.appearance.shellVersion")) {
      const theme=localStorage.getItem("atogaki.appearance.theme"),image=localStorage.getItem("atogaki.appearance.imageTheme");
      localStorage.setItem("atogaki.appearance.beforeShellV2",JSON.stringify({theme,image}));
      if(image){try{const saved=JSON.parse(image);if(saved&&typeof saved==="object")localStorage.setItem("atogaki.appearance.imageTheme",JSON.stringify({...saved,enabled:false}));}catch{/* Preserve malformed legacy data. */}}
      localStorage.setItem("atogaki.appearance.theme","light");
      localStorage.setItem("atogaki.appearance.shellVersion","2");
    }
    const saved=localStorage.getItem("atogaki.appearance.theme");
    if(saved && !["light","dark","system"].includes(saved)) {
      if(!localStorage.getItem("atogaki.appearance.beforeThemeV3"))localStorage.setItem("atogaki.appearance.beforeThemeV3",JSON.stringify({theme:saved,image:localStorage.getItem("atogaki.appearance.imageTheme")}));
      localStorage.setItem("atogaki.appearance.theme","light");
    }
    return normalizeTheme(saved);
  } catch { return "light"; }
}
let system: MediaQueryList | null = null;
export function resolvedTheme(theme:AppTheme):"light"|"dark" {
  return theme==="system" ? (window.matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light") : theme;
}
export function applyTheme(theme: AppTheme): void {
  document.documentElement.dataset.themePreference = theme;
  document.documentElement.dataset.theme = resolvedTheme(theme);
  try { localStorage.setItem("atogaki.appearance.theme", theme); } catch { /* Session preference still applies. */ }
  if(!system && typeof window.matchMedia==="function") {
    system=window.matchMedia("(prefers-color-scheme: dark)");
    system.addEventListener("change",()=>{if(document.documentElement.dataset.themePreference==="system")applyTheme("system");});
  }
  window.dispatchEvent(new Event("atogaki-theme-change"));
}
export type CandidateLabelInput = { id:string; provider_name:string; model_identity:string; scope_start_ms:number; scope_end_ms:number|null; created_at_unix:number; is_candidate?:boolean };
export function candidateLabel(run: CandidateLabelInput): string {
  const model = run.model_identity.split(/[\\/]/).pop()?.replace(/\.(bin|gguf)$/i, "") || "未知模型";
  const time = (ms:number) => `${Math.floor(ms/60000).toString().padStart(2,"0")}:${Math.floor(ms/1000%60).toString().padStart(2,"0")}`;
  const range = run.scope_end_ms === null ? "全片" : `${time(run.scope_start_ms)}–${time(run.scope_end_ms)}`;
  const date = new Date(run.created_at_unix * 1000).toLocaleString("zh-CN", {month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit"});
  return `${run.is_candidate === false ? "原始识别" : range+"候选"} · ${run.provider_name} · ${model} · ${date} · ${run.id.slice(0,8)}`;
}
