import type { AsrCredentialStatus } from "./translation-provider-types.ts";
import type { SettingsMutationGate } from "./settings-mutation-gate.ts";
export type AsrCredentialAction = "save" | "delete" | "check";
type Options = {
  backend: { save(apiKey: string | null, clear: boolean): Promise<AsrCredentialStatus>; check(): Promise<AsrCredentialStatus> };
  mutations: SettingsMutationGate;
  visit(): number; isOpen(): boolean;
  capture(clear: boolean): { apiKey: string | null; acknowledge(): void } | null;
  accepted(status: AsrCredentialStatus): void;
  invalidate(): void;
  refresh(): Promise<void>;
  changed(): void;
  message(text: string, warning: boolean): void;
  checkHint: string;
  clearMessage(text: string): void;
};
/** The check path never captures or clears input; secrets exist only in a write invocation. */
export function createAsrCredentialSession(options: Options) {
  let disposed = false, busy = false, lastMessage: string | null = null;
  const message = (text: string, warning: boolean) => { lastMessage = text; options.message(text, warning); };
  return {
    get busy() { return busy; },
    async run(action: AsrCredentialAction): Promise<void> {
      if (disposed || busy || !options.isOpen() || !["save", "delete", "check"].includes(action)) return;
      const write = action !== "check";
      const release = write ? options.mutations.acquire() : null;
      if ((write && !release) || (!write && options.mutations.busy)) {
        message("另一个配置或凭据操作正在进行，请完成后重试。", true); return;
      }
      const visit = options.visit(), revision = options.mutations.revision;
      const current = () => !disposed && visit === options.visit() && options.isOpen() && revision === options.mutations.revision;
      busy = true; options.changed(); let committed = false;
      try {
        const snapshot = write ? options.capture(action === "delete") : null;
        if (write && !snapshot) return;
        if (write && action === "save" && !snapshot!.apiKey?.trim()) { message("请输入 Gemini API Key。", true); return; }
        message(write ? action === "delete" ? "正在删除 Gemini Key…" : "正在保存 Gemini Key…" : `正在检查系统凭据；${options.checkHint}…`, false);
        if (write) options.invalidate();
        const status = write ? await options.backend.save(action === "delete" ? null : snapshot!.apiKey!.trim(), action === "delete") : await options.backend.check();
        if (write) { committed = true; if (!disposed) options.invalidate(); }
        if (!current()) return;
        if (write) snapshot!.acknowledge();
        options.accepted(status);
        if (write) await options.refresh();
      } catch (error) {
        if (current()) message(committed ? `Gemini Key 已${action === "delete" ? "删除" : "保存"}，但状态刷新失败：${String(error)}` : `${write ? "操作" : "检查"}失败：${String(error)}`, true);
      } finally {
        release?.(); busy = false;
        if (!disposed && !write && visit === options.visit() && revision !== options.mutations.revision && lastMessage !== null) {
          options.clearMessage(lastMessage); lastMessage = null;
        }
        if (!disposed) {
          options.changed();
          if (write && committed && visit !== options.visit() && options.isOpen()) {
            try { await options.refresh(); } catch { /* New visit can explicitly retry status reads. */ }
          }
        }
      }
    },
    hidden() { if (!disposed && lastMessage !== null) { options.clearMessage(lastMessage); lastMessage = null; } },
    dispose() { disposed = true; lastMessage = null; },
  };
}
