export const modelPickerKinds = ["whisper", "vad", "hy-mt2"] as const;
export type ModelPickerKind = typeof modelPickerKinds[number];
const commands: Record<ModelPickerKind, string> = { whisper: "pick_model_file", vad: "pick_vad_model_file", "hy-mt2": "pick_hy_mt2_model_file" };

export function createModelPickerBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return { pick: (kind: ModelPickerKind) => invoke<string | null>(commands[kind], {}) };
}
