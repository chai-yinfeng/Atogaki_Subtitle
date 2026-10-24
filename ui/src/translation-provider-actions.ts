import type { SettingsMutationGate } from "./settings-mutation-gate.ts";
import type { TranslationProfileSummary } from "./desktop-settings-types.ts";
import type { AsrCredentialStatus, ProviderAction, TranslationCredentialCheck } from "./translation-provider-types.ts";

type Options = {
  mutations?: SettingsMutationGate;
  backend: {
    test(provider: string): Promise<unknown>;
    check(provider: string): Promise<TranslationCredentialCheck>;
    credential(provider: string, apiKey: string | null, clear: boolean): Promise<unknown>;
    profiles(): Promise<TranslationProfileSummary[]>;
    asrStatus(): Promise<AsrCredentialStatus>;
  };
  visit(): number; isOpen(): boolean;
  captureCredential(provider: string, clear: boolean): { apiKey: string | null; applied(): void; asrStatus(status: AsrCredentialStatus): void };
  profilesAccepted(profiles: TranslationProfileSummary[]): void;
  invalidateSettingsLoads(): void;
  credentialMessage(provider: string, text: string): void;
  message(text: string): void;
};

/** Coordinates provider actions; secret input is captured per invocation, never cached in session state. */
export function createTranslationProviderActions(options: Options) {
  let disposed = false, profileGeneration = 0, noticeGeneration = 0;
  const pending = new Set<string>();
  const currentVisit = (visit: number) => !disposed && visit === options.visit();
  const profiles = async (current: () => boolean): Promise<void> => {
    const request = ++profileGeneration;
    let value: TranslationProfileSummary[];
    try { value = await options.backend.profiles(); }
    catch (error) { if (current() && request === profileGeneration) throw error; else return; }
    if (current() && request === profileGeneration) options.profilesAccepted(value);
  };
  return {
    invalidateProfiles() { ++profileGeneration; },
    async refreshProfiles(): Promise<void> {
      if (disposed) return;
      const visit = options.visit();
      await profiles(() => currentVisit(visit));
    },
    async run(action: ProviderAction, id: string, viewCurrent: () => boolean = () => true): Promise<void> {
      if (disposed || !["deepseek", "gemini", "deepl", "openai-compatible"].includes(id)) return;
      const key = action === "save" || action === "delete" ? "credential" : `${action}:${id}`;
      if (pending.has(key)) return;
      const write = action === "save" || action === "delete";
      const release = write ? options.mutations?.acquire() : null;
      if (options.mutations && (write ? !release : options.mutations.busy)) {
        options.message("另一个配置或凭据操作正在进行，请完成后重试。"); return;
      }
      const mutationRevision = options.mutations?.revision;
      const visit = options.visit(), notice = ++noticeGeneration;
      const current = () => currentVisit(visit) && viewCurrent() && (write || mutationRevision === options.mutations?.revision);
      const message = (text: string) => { if (current() && notice === noticeGeneration) options.message(text); };
      pending.add(key);
      let committed = false;
      try {
        const capture = action === "save" || action === "delete" ? options.captureCredential(id, action === "delete") : null;
        if (action === "test") await options.backend.test(id);
        else if (action === "check") {
          const result = await options.backend.check(id);
          message(`${result.providerName}：${result.storedInSystem || result.availableFromEnvironment ? "凭据存在，尚不代表连接已通过" : "未找到凭据"}；本次未调用翻译 API。`);
        } else {
          ++profileGeneration; options.invalidateSettingsLoads();
          await options.backend.credential(id, capture!.apiKey, action === "delete");
          committed = true;
          if (!disposed) { ++profileGeneration; options.invalidateSettingsLoads(); }
          if (!current()) return;
          capture!.applied();
        }
        if (!current()) return;
        await profiles(current);
        if (!current()) return;
        if (committed) {
          if (id === "gemini") {
            const status = await options.backend.asrStatus();
            if (!current()) return;
            capture!.asrStatus(status);
          }
          const verb = action === "delete" ? "删除" : "保存";
          options.credentialMessage(id, `${id} 凭据已${verb}；连接状态请查看卡片。`);
          message(`${id} 凭据已${verb}；${id === "gemini" ? "语音识别与翻译共用此 Key，音频上传仍需单独授权。" : "其他后端配置未改变。"}`);
        }
      } catch (error) {
        message(committed ? `凭据已${action === "delete" ? "删除" : "保存"}，但状态刷新失败：${String(error)}` : `${action === "test" ? "连接测试失败：" : ""}${String(error)}`);
        if (action === "test" && current()) await profiles(current).catch(() => {});
      } finally {
        release?.(); pending.delete(key);
        if (!disposed && visit !== options.visit() && options.isOpen()) {
          const freshVisit = options.visit();
          try { await profiles(() => currentVisit(freshVisit)); }
          catch { /* A later explicit read can retry; do not announce an old action in the new visit. */ }
        }
      }
    },
    dispose() { disposed = true; ++profileGeneration; ++noticeGeneration; pending.clear(); },
  };
}
