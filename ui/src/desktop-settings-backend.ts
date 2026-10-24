import type { DesktopSettings, SaveDesktopSettingsRequest, TranslationProfileSummary } from "./desktop-settings-types.ts";

export function createDesktopSettingsBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    save: (request: SaveDesktopSettingsRequest) => invoke<DesktopSettings>("save_desktop_settings", { request }),
    profiles: () => invoke<TranslationProfileSummary[]>("translation_profiles", {}),
  };
}
