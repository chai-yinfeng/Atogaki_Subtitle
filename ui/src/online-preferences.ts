export type OnlinePreferences = {
  recognitionMode: "one-pass" | "two-pass";
  draftBackend: "sensevoice" | "zipformer";
  language: string;
  finalModel: string;
  translationBackend: string;
  translate: boolean;
  translationDrafts: boolean;
};
const key = "atogaki.onlinePreferences";
export function readOnlinePreferences(): OnlinePreferences {
  const result: OnlinePreferences = { recognitionMode: "one-pass", draftBackend: "sensevoice", language: "ja",
    finalModel: "qwen-1.7b-8bit", translationBackend: "hy-mt2", translate: true, translationDrafts: true };
  try {
    const saved = JSON.parse(localStorage.getItem(key) ?? "{}");
    if (saved?.recognitionMode === "one-pass" || saved?.recognitionMode === "two-pass") result.recognitionMode = saved.recognitionMode;
    if (saved?.draftBackend === "sensevoice" || saved?.draftBackend === "zipformer") result.draftBackend = saved.draftBackend;
    if (["ja", "en", "zh", "ko"].includes(saved?.language)) result.language = saved.language;
    if (typeof saved?.finalModel === "string" && saved.finalModel.startsWith("qwen-")) result.finalModel = saved.finalModel;
    if (["hy-mt2", "index-translate"].includes(saved?.translationBackend)) result.translationBackend = saved.translationBackend;
    for (const field of ["translate", "translationDrafts"] as const) if (typeof saved?.[field] === "boolean") result[field] = saved[field];
  } catch { /* Defaults remain usable when local preferences are unavailable. */ }
  return result;
}
export function saveOnlinePreferences(patch: Partial<OnlinePreferences>): void {
  try { localStorage.setItem(key, JSON.stringify({...readOnlinePreferences(), ...patch})); }
  catch { /* A storage failure must not interrupt the live session. */ }
}
