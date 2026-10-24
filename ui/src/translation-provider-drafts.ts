import type { TranslationProfileSummary } from "./desktop-settings-types.ts";

export type ProviderFields = { model: string; endpoint: string; style: string };
const defaultStyle = "准确、自然的简体中文口语字幕；保留说话语气，不补充原文没有的信息。";
const copy = (value: ProviderFields): ProviderFields => ({ model: value.model, endpoint: value.endpoint, style: value.style });
export function defaultProviderFields(id: string): ProviderFields {
  return {
    model: id === "deepseek" ? "deepseek-v4-flash" : id === "gemini" ? "gemini-3.8-flash" : "",
    endpoint: id === "deepseek" ? "https://api.deepseek.com" : id === "gemini" ? "https://generativelanguage.googleapis.com/v1beta/openai" : id === "openai-compatible" ? "https://api.openai.com/v1" : "",
    style: defaultStyle,
  };
}

/** The single non-secret draft registry; provider-specific dirty versions never contain keys. */
export function createTranslationProviderDrafts() {
  let selected = "none", revision = 0;
  const values = new Map<string, ProviderFields>(), dirty = new Map<string, number>(), knownProfiles = new Set<string>(), readiness = new Map<string, string>();
  const get = (id: string) => copy(values.get(id) ?? defaultProviderFields(id));
  return {
    get selected() { return selected; },
    get revision() { return revision; },
    get readiness(): ReadonlyMap<string, string> { return readiness; },
    dirty: { has: (id: string) => dirty.has(id) },
    current: () => get(selected),
    select(id: string) { selected = id; if (!values.has(id)) values.set(id, defaultProviderFields(id)); return get(id); },
    edit(value: ProviderFields) { values.set(selected, copy(value)); dirty.set(selected, ++revision); },
    acceptProfiles(profiles: TranslationProfileSummary[]) {
      for (const profile of profiles) {
        const id = profile.provider_id; knownProfiles.add(id); readiness.set(id, profile.readiness);
        if (!dirty.has(id)) values.set(id, { model: profile.model ?? "", endpoint: profile.base_url ?? defaultProviderFields(id).endpoint, style: profile.style });
      }
    },
    fallback(id: string, value: ProviderFields) { if (!knownProfiles.has(id) && !dirty.has(id)) values.set(id, copy(value)); },
    snapshot: () => new Map(dirty),
    acknowledge(snapshot: ReadonlyMap<string, number>, id: string) { if (snapshot.has(id) && dirty.get(id) === snapshot.get(id)) dirty.delete(id); },
  };
}
