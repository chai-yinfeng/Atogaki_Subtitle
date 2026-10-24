import { modelPickerKinds, type ModelPickerKind } from "./model-picker-backend.ts";

type Options = {
  backend: { pick(kind: ModelPickerKind): Promise<string | null> };
  visit(): number; isOpen(): boolean;
  capture(kind: ModelPickerKind): { apply(path: string): boolean } | null;
  changed(): void;
  message(text: string): void;
  clearMessage(text: string): void;
};

/** One native model chooser; a late result never replaces a newer settings draft. */
export function createModelPickerSession(options: Options) {
  let disposed = false, busy = false, lastMessage: string | null = null;
  const message = (text: string) => { lastMessage = text; options.message(text); };
  return {
    get busy() { return busy; },
    async pick(kind: ModelPickerKind): Promise<void> {
      if (disposed || busy || !modelPickerKinds.includes(kind) || !options.isOpen()) return;
      const visit = options.visit(), snapshot = options.capture(kind); if (!snapshot) return;
      const current = () => !disposed && options.isOpen() && visit === options.visit();
      busy = true; options.changed();
      message("正在打开模型文件选择器…");
      try {
        const path = await options.backend.pick(kind);
        if (!current()) return;
        if (!path) message("已取消选择模型文件。");
        else message(snapshot.apply(path) ? "已选择模型文件；尚未保存配置。" : "当前路径已变化，保留现有值；未替换为选择结果。");
      } catch (error) {
        if (current()) message(`无法打开模型选择器：${String(error)}`);
      } finally {
        busy = false;
        if (!disposed) options.changed();
      }
    },
    hidden() { if (!disposed && lastMessage !== null) { options.clearMessage(lastMessage); lastMessage = null; } },
    dispose() { disposed = true; busy = false; lastMessage = null; },
  };
}
