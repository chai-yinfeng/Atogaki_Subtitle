import { dictionaryCredentialProviders, type DictionaryCredentialProvider } from "./dictionary-credential-backend.ts";
import type { DictionaryCredentialStatus } from "./desktop-settings-types.ts";
import type { SettingsMutationGate } from "./settings-mutation-gate.ts";
export type DictionaryCredentialAction = "save" | "delete" | "check";
type Options = {
  backend: { save(provider: DictionaryCredentialProvider, apiKey: string | null, clear: boolean): Promise<DictionaryCredentialStatus>; check(provider: DictionaryCredentialProvider): Promise<DictionaryCredentialStatus> };
  mutations: SettingsMutationGate;
  visit(): number; isOpen(): boolean;
  capture(provider: DictionaryCredentialProvider, clear: boolean): { apiKey: string | null; acknowledge(): void } | null;
  accepted(status: DictionaryCredentialStatus): void;
  invalidate(): void;
  reload(): Promise<void>;
  changed(): void;
  message(text: string): void;
  clearMessage(text: string): void;
  checkHint: string;
};
/** Uses existing dictionary state; secrets are captured only by a single explicit write. */
export function createDictionaryCredentialSession(options: Options) {
  let disposed = false, busy = false, lastMessage: string | null = null;
  const message = (text: string) => { lastMessage = text; options.message(text); };
  return {
    get busy() { return busy; },
    async run(action: DictionaryCredentialAction, provider: DictionaryCredentialProvider): Promise<void> {
      if (disposed || busy || !options.isOpen() || !dictionaryCredentialProviders.includes(provider) || !["save", "delete", "check"].includes(action)) return;
      const write = action !== "check", release = write ? options.mutations.acquire() : null;
      if ((write && !release) || (!write && options.mutations.busy)) { message("另一个配置或凭据操作正在进行，请完成后重试。"); return; }
      const visit = options.visit(), revision = options.mutations.revision;
      const current = () => !disposed && visit === options.visit() && options.isOpen() && revision === options.mutations.revision;
      busy = true; options.changed(); let committed = false;
      try {
        const snapshot = write ? options.capture(provider, action === "delete") : null;
        if (write && !snapshot) return;
        if (action === "save" && !snapshot!.apiKey?.trim()) { message("请先输入要保存的 API Key。"); return; }
        message(write ? action === "delete" ? "正在删除词典凭据…" : "正在保存词典凭据…" : `正在检查系统凭据；${options.checkHint}…`);
        options.invalidate();
        const status = write ? await options.backend.save(provider, action === "delete" ? null : snapshot!.apiKey!.trim(), action === "delete") : await options.backend.check(provider);
        if (write) committed = true;
        if (!disposed && (write || current())) options.invalidate();
        if (!current()) return;
        if (status.providerId !== provider) throw Error("词典凭据状态的来源与请求不一致");
        if (write) snapshot!.acknowledge();
        options.accepted(status);
        message(write ? `${status.providerName} Key 已${action === "delete" ? `从 ${status.credentialStore} 删除` : `保存至 ${status.credentialStore}`}。` : status.configured ? `${status.providerName} Key 存在于 ${status.credentialStore}；内容不会回显。` : `${status.credentialStore} 中没有 ${status.providerName} Key。`);
      } catch (error) {
        if (current()) message(committed ? `词典凭据已${action === "delete" ? "删除" : "保存"}，但状态更新失败：${String(error)}` : `${write ? "词典凭据操作" : "检查"}失败：${String(error)}`);
      } finally {
        release?.(); busy = false;
        if (!disposed) {
          if (!write && visit === options.visit() && revision !== options.mutations.revision && lastMessage !== null) { options.clearMessage(lastMessage); lastMessage = null; }
          options.changed();
          if (write && committed && visit !== options.visit() && options.isOpen()) {
            try { await options.reload(); } catch { /* The dictionary read boundary presents current failures. */ }
          }
        }
      }
    },
    hidden() { if (!disposed && lastMessage !== null) { options.clearMessage(lastMessage); lastMessage = null; } },
    dispose() { disposed = true; lastMessage = null; },
  };
}
