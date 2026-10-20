export type ProcessingTime = { elapsed_seconds: number; active_since_unix: number | null };

export function executionSeconds(status: string, clock: ProcessingTime | null | undefined, now: number): number | null {
  if (!clock) return null;
  const active = !["queued", "created", "done", "failed"].includes(status);
  return Math.max(0, clock.elapsed_seconds) + (active && clock.active_since_unix !== null ? Math.max(0, now - clock.active_since_unix) : 0);
}

export function earlyTranslationLabel(status: string, error: string | null): string {
  if (status === "translating") return " · 识别完成，正在完成翻译";
  if (!error) return "";
  if (status === "failed") return " · 翻译待继续";
  if (status === "queued") return " · 等待继续";
  return " · 提前翻译已停止，识别继续";
}
