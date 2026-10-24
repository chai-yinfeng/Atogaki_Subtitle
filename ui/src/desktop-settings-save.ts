import type { DesktopSettings, SaveDesktopSettingsRequest, TranslationProfileSummary } from "./desktop-settings-types.ts";

/** One dirty registry, with versions so an acknowledgement cannot clear newer input. */
export function createVersionedDirtyFields() {
  const versions = new Map<string, number>();
  let revision = 0;
  return {
    add(id: string) { versions.set(id, ++revision); },
    has: (id: string) => versions.has(id),
    delete: (id: string) => versions.delete(id),
    clear: () => versions.clear(),
    get size() { return versions.size; },
    get revision() { return revision; },
    snapshot: () => new Map(versions),
    acknowledge(snapshot: ReadonlyMap<string, number>, only?: string) {
      for (const [id, version] of snapshot) {
        if ((only === undefined || id === only) && versions.get(id) === version) versions.delete(id);
      }
    },
  };
}

type Options = {
  backend: { save(request: SaveDesktopSettingsRequest): Promise<DesktopSettings>; profiles(): Promise<TranslationProfileSummary[]> };
  visit(): number;
  capture(finishOnboarding: boolean): { request: SaveDesktopSettingsRequest; acknowledge(): void; hasNewInput(): boolean } | null;
  invalidateLoads(): void;
  accepted(settings: DesktopSettings, profiles: TranslationProfileSummary[] | null): void;
  refreshTranslation(): Promise<boolean | void>;
  reload(): Promise<void>;
  isOpen(): boolean;
  close(): void;
  changed(busy: boolean): void;
  message(text: string): void;
};

/** Saves one frozen request. Credentials live only in that request, never in session state. */
export function createDesktopSettingsSave(options: Options) {
  let busy = false, disposed = false;
  return {
    async save(finishOnboarding: boolean): Promise<void> {
      if (busy || disposed) return;
      const snapshot = options.capture(finishOnboarding);
      if (!snapshot) return;
      const visit = options.visit();
      const current = () => !disposed && visit === options.visit();
      busy = true; options.invalidateLoads(); options.changed(true);
      options.message("正在保存配置…");
      let committed = false;
      try {
        const settings = await options.backend.save({ ...snapshot.request });
        committed = true;
        if (!current()) return;
        let profiles: TranslationProfileSummary[] | null = null, refreshFailed = false;
        try { profiles = await options.backend.profiles(); }
        catch { refreshFailed = true; }
        if (!current()) return;
        snapshot.acknowledge();
        options.invalidateLoads();
        options.accepted(settings, profiles);
        try { if (await options.refreshTranslation() === false) refreshFailed = true; }
        catch { refreshFailed = true; }
        if (!current()) return;
        const newerInput = snapshot.hasNewInput();
        options.message(refreshFailed
          ? "配置已保存，但状态刷新未完成；可重新打开设置读取。"
          : newerInput ? "配置已保存；保存期间的新输入尚未保存。" : "配置已保存；如提供密钥，只会写入系统凭据库。");
        if (finishOnboarding && !settings.needsOnboarding && !newerInput && !refreshFailed) options.close();
      } catch (error) {
        if (current()) options.message(committed ? `配置已保存，但界面刷新失败：${String(error)}` : `保存失败：${String(error)}`);
      } finally {
        busy = false;
        if (!disposed) {
          options.changed(false);
          // Reopening during an in-flight save may have read the pre-save settings.
          if (visit !== options.visit() && options.isOpen()) {
            try { await options.reload(); }
            catch { /* The load boundary owns its own failure presentation. */ }
          }
        }
      }
    },
    dispose() { disposed = true; },
  };
}
