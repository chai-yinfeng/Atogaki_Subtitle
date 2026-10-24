import type { DictionaryDownloadState } from "./desktop-settings-types.ts";
export type DictionaryDownloadNetwork = { proxyMode: string; proxyUrl: string | null; modelMirrorUrl: string | null };
export function createDictionaryDownloadBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    network: (request: DictionaryDownloadNetwork) => invoke<void>("save_download_network_settings", { request }),
    start: (dictionaryId: string) => invoke<DictionaryDownloadState>("start_dictionary_download", { dictionaryId }),
    states: () => invoke<DictionaryDownloadState[]>("dictionary_download_states", {}),
  };
}
