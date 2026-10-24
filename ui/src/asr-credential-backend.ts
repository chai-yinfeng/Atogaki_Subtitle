import type { AsrCredentialStatus } from "./translation-provider-types.ts";
export function createAsrCredentialBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    save: (apiKey: string | null, clear: boolean) => invoke<AsrCredentialStatus>("save_asr_credential", { request: { apiKey, clear } }),
    check: () => invoke<AsrCredentialStatus>("check_asr_credential", {}),
  };
}
