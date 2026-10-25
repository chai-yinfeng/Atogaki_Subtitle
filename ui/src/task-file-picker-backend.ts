export const taskFileKinds = ["media", "model", "vad"] as const;
export type TaskFileKind = typeof taskFileKinds[number];
export function createTaskFilePickerBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return { async pick(kind: TaskFileKind): Promise<string[]> {
    if (kind === "media") return invoke<string[]>("pick_media_files", {});
    const path = await invoke<string | null>(kind === "model" ? "pick_model_file" : "pick_vad_model_file", {});
    return path === null ? [] : [path];
  } };
}
