import type { DictionaryCredentialStatus } from "./desktop-settings-types.ts";
export const dictionaryCredentialProviders = ["merriam-webster"] as const;
export type DictionaryCredentialProvider = typeof dictionaryCredentialProviders[number];
export function createDictionaryCredentialBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    save: (providerId: DictionaryCredentialProvider, apiKey: string | null, clear: boolean) => invoke<DictionaryCredentialStatus>("save_dictionary_credential", { request: { providerId, apiKey, clear } }),
    check: (providerId: DictionaryCredentialProvider) => invoke<DictionaryCredentialStatus>("check_dictionary_credential", { providerId }),
  };
}
