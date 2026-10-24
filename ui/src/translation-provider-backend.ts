import type { TranslationProfileSummary } from "./desktop-settings-types.ts";
import type { AsrCredentialStatus, TranslationCredentialCheck } from "./translation-provider-types.ts";

export function createTranslationProviderBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    test: (providerId: string) => invoke<string>("test_translation_connection", { providerId }),
    check: (providerId: string) => invoke<TranslationCredentialCheck>("check_translation_api_key", { providerId }),
    credential: (providerId: string, apiKey: string | null, clear: boolean) => invoke<void>("save_translation_credential", { providerId, apiKey, clear }),
    profiles: () => invoke<TranslationProfileSummary[]>("translation_profiles", {}),
    asrStatus: () => invoke<AsrCredentialStatus>("asr_credential_status", {}),
  };
}
