export type AppTheme = "moonlight" | "letter";
export function normalizeTheme(value: string | null): AppTheme {
  return value === "letter" ? "letter" : "moonlight";
}
export function readTheme(): AppTheme {
  try { return normalizeTheme(localStorage.getItem("atogaki.appearance.theme")); }
  catch { return "moonlight"; }
}
export function applyTheme(theme: AppTheme): void {
  document.documentElement.dataset.theme = theme;
  try { localStorage.setItem("atogaki.appearance.theme", theme); } catch { /* Session preference still applies. */ }
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
