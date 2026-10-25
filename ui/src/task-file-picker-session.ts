import { taskFileKinds, type TaskFileKind } from "./task-file-picker-backend.ts";
type Options = {
  backend: { pick(kind: TaskFileKind): Promise<string[]> };
  visit(): number; isOpen(): boolean;
  capture(kind: TaskFileKind): { apply(files: string[]): boolean } | null;
  changed(): void;
  message(text: string): void; clearMessage(text: string): void;
};
const labels: Record<TaskFileKind, string> = { media: "媒体", model: "Whisper 模型", vad: "VAD 模型" };
/** Native file selection owns one task-form visit, never a task submission or relink. */
export function createTaskFilePickerSession(options: Options) {
  let disposed = false, busy = false, lastMessage: string | null = null;
  const message = (text: string) => { lastMessage = text; options.message(text); };
  const hidden = () => { if (!disposed && lastMessage !== null) { options.clearMessage(lastMessage); lastMessage = null; } };
  return {
    get busy() { return busy; },
    async pick(kind: TaskFileKind): Promise<void> {
      if (disposed || busy || !taskFileKinds.includes(kind) || !options.isOpen()) return;
      const visit = options.visit(), snapshot = options.capture(kind); if (!snapshot) return;
      const current = () => !disposed && visit === options.visit() && options.isOpen();
      busy = true; options.changed(); message(`正在打开${labels[kind]}选择器…`);
      try {
        const files = await options.backend.pick(kind);
        if (!current()) return;
        if (!files.length) message("已取消选择。");
        else message(snapshot.apply([...files]) ? `已选择${labels[kind]}${kind === "media" && files.length > 1 ? ` ${files.length} 个文件` : ""}。` : "表单已变化，保留现有输入；未替换为选择结果。");
      } catch (error) {
        if (current()) message(`无法打开文件选择器：${String(error)}`);
      } finally {
        busy = false;
        if (!disposed) { if (!current()) hidden(); options.changed(); }
      }
    },
    hidden,
    dispose() { disposed = true; busy = false; lastMessage = null; },
  };
}
