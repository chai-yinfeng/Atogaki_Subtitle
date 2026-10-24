import type { DesktopSettings, DesktopSettingsSnapshot, DictionarySettingsSnapshot, ModelCatalogItem, ModelDownloadState, TranslationProfileSummary } from "./desktop-settings-types.ts";

type Options = {
  backend: {
    settings(): Promise<DesktopSettings>; models(): Promise<ModelCatalogItem[]>;
    downloads(): Promise<ModelDownloadState[]>; profiles(): Promise<TranslationProfileSummary[]>;
    dictionaries(): Promise<DictionarySettingsSnapshot>;
  };
  visit(): number;
  catalog(): ModelCatalogItem[];
  accepted(snapshot: DesktopSettingsSnapshot): void;
  dictionariesAccepted(snapshot: DictionarySettingsSnapshot): void;
  onboard(): void;
  message(text: string): void;
  dictionaryMessage(text: string): void;
};

/** Read-only settings requests; no second settings store or download polling loop. */
export function createDesktopSettingsLoad(options: Options) {
  let disposed = false, generation = 0, dictionaryGeneration = 0;
  const dictionaries = async (parentCurrent: () => boolean = () => true): Promise<boolean> => {
    if (disposed) return false;
    const request = ++dictionaryGeneration, visit = options.visit();
    const current = () => !disposed && request === dictionaryGeneration && visit === options.visit() && parentCurrent();
    try {
      const snapshot = await options.backend.dictionaries();
      if (!current()) return false;
      options.dictionariesAccepted(snapshot);
      return true;
    } catch (error) {
      if (current()) options.dictionaryMessage(`无法读取词典配置：${String(error)}`);
      return false;
    }
  };
  return {
    async load(openWhenNeeded = false): Promise<boolean> {
      if (disposed) return false;
      const request = ++generation, visit = options.visit();
      const current = () => !disposed && request === generation && visit === options.visit();
      try {
        const cached = options.catalog();
        const [settings, catalog, downloads, profiles] = await Promise.all([
          options.backend.settings(), cached.length ? Promise.resolve(cached) : options.backend.models(),
          options.backend.downloads(), options.backend.profiles(),
        ]);
        if (!current()) return false;
        options.accepted({ settings, catalog, downloads, profiles });
        await dictionaries(current);
        if (!current()) return false;
        if (openWhenNeeded && settings.needsOnboarding) options.onboard();
        return true;
      } catch (error) {
        if (current()) options.message(`无法读取启动配置：${String(error)}`);
        return false;
      }
    },
    loadDictionaries: () => dictionaries(),
    invalidate() { ++generation; },
    invalidateDictionaries() { ++dictionaryGeneration; },
    dispose() { disposed = true; ++generation; ++dictionaryGeneration; },
  };
}
