import type { DesktopSettings, SaveDesktopSettingsRequest, TranslationProfileSummary, ModelCatalogItem, ModelDownloadState, DictionaryCatalogItem, DictionaryDownloadState, DictionaryCredentialStatus, DictionarySettingsSnapshot } from "./desktop-settings-types.ts";

export function createDesktopSettingsBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    settings: () => invoke<DesktopSettings>("desktop_settings", {}),
    models: () => invoke<ModelCatalogItem[]>("model_catalog", {}),
    downloads: () => invoke<ModelDownloadState[]>("model_download_states", {}),
    dictionaries: async (): Promise<DictionarySettingsSnapshot> => {
      const [catalog, downloads, credentials, directory] = await Promise.all([
        invoke<DictionaryCatalogItem[]>("dictionary_catalog", {}),
        invoke<DictionaryDownloadState[]>("dictionary_download_states", {}),
        invoke<DictionaryCredentialStatus[]>("dictionary_credential_statuses", {}),
        invoke<string>("dictionary_directory", {}),
      ]);
      return { catalog, downloads, credentials, directory };
    },
    save: (request: SaveDesktopSettingsRequest) => invoke<DesktopSettings>("save_desktop_settings", { request }),
    profiles: () => invoke<TranslationProfileSummary[]>("translation_profiles", {}),
  };
}
