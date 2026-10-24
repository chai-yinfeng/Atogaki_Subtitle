import type { ModelDownloadState } from "./desktop-settings-types.ts";
export type DownloadNetwork = { proxyMode: string; proxyUrl: string | null; modelMirrorUrl: string | null };

export function createModelManagementBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return {
    network: (request: DownloadNetwork) => invoke<void>("save_download_network_settings", { request }),
    start: (modelId: string) => invoke<ModelDownloadState>("start_model_download", { modelId }),
    states: () => invoke<ModelDownloadState[]>("model_download_states", {}),
    cancel: (modelId: string) => invoke<ModelDownloadState>("cancel_model_download", { modelId }),
    select: (modelId: string) => invoke<ModelDownloadState>("select_model", { modelId }),
    uninstall: (modelId: string) => invoke<void>("uninstall_model", { modelId }),
  };
}
