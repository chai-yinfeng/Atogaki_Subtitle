import type { ProviderBackend, TaskProvider, TranslationStatus } from "./task-translation-provider.ts";

export function createTaskProviderBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>): ProviderBackend {
  return {
    status: jobId => invoke<TranslationStatus>("translation_status", { jobId }),
    select: (jobId: string, providerId: TaskProvider) => invoke("set_task_translation", { jobId, providerId }),
  };
}
