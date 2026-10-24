type UsageRun = { input_tokens: number | null; output_tokens: number | null; plan_json?: string | null };
const validCount = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;

/** Format only the latest record; never present it as a whole-task API bill. */
export function translationUsageLabel(run: UsageRun): string {
  let usage: Record<string, unknown> | null = null;
  try {
    const value = run.plan_json ? JSON.parse(run.plan_json)?.execution_usage : null;
    if (value && typeof value === "object" && !Array.isArray(value)) usage = value;
  } catch { /* Legacy or unreadable optional metadata must not hide the record. */ }
  if (usage?.local_cache_hit === true) return "本地缓存复用（本次无新增 API 消耗）";
  const labels: string[] = [];
  if (validCount(run.input_tokens) || validCount(run.output_tokens)) {
    labels.push(`${usage ? "本次 Token" : "Token"} ${validCount(run.input_tokens) ? run.input_tokens : "?"} 入 / ${validCount(run.output_tokens) ? run.output_tokens : "?"} 出`);
  }
  if (usage && validCount(usage.cached_input_tokens) && validCount(usage.uncached_input_tokens)) {
    labels.push(`云端输入缓存 ${usage.cached_input_tokens} 命中 / ${usage.uncached_input_tokens} 未命中`);
  }
  return labels.join(" · ");
}
