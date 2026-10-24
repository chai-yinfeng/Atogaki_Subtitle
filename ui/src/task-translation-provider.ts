export type TranslationStatus = {
  provider_id: string; provider: string; configured: boolean; model: string | null;
  endpoint_kind: string; configuration_hint: string | null;
};
export const taskProviders = ["none", "deepseek", "gemini", "deepl", "hy-mt2-local", "openai-compatible"] as const;
export type TaskProvider = typeof taskProviders[number];
type Context = { jobId: string | null; navigation: number };
export type ProviderBackend = {
  status(jobId: string | null): Promise<TranslationStatus>;
  select(jobId: string, providerId: TaskProvider): Promise<unknown>;
};
type Options = {
  context(): Context; backend: ProviderBackend; changed(): void;
  loading(): void; accepted(status: TranslationStatus): void;
  message(text: string, error?: boolean): void;
};
const validProvider = (value: string): value is TaskProvider => (taskProviders as readonly string[]).includes(value);

/** Task-bound selection; an in-flight IPC may commit, but cannot retarget a new visit. */
export function createTaskTranslationProvider(options: Options) {
  let disposed = false, generation = 0, editVersion = 0, presentation = 0;
  let latest: { context: Context; status: TranslationStatus } | null = null;
  const drafts = new Map<string, { provider: TaskProvider; version: number }>();
  type Mutation = { context: Context; provider: TaskProvider; version: number; presentation: number };
  const pending = new Map<string, Mutation>();
  const current = (context: Context) => {
    if (disposed) return false;
    const now = options.context();
    return context.jobId === now.jobId && context.navigation === now.navigation;
  };
  const changed = () => { if (!disposed) options.changed(); };
  const state = () => {
    if (disposed) return { selected: "none" as TaskProvider, busy: false, canApply: false, visitKey: "disposed" };
    const context = options.context(), status = latest && current(latest.context) ? latest.status : null;
    const draft = context.jobId ? drafts.get(context.jobId) : undefined;
    const mutation = context.jobId ? pending.get(context.jobId) : undefined;
    const selected = draft?.provider ?? mutation?.provider ?? (status && validProvider(status.provider_id) ? status.provider_id : "none");
    return { selected, busy: Boolean(mutation), canApply: Boolean(context.jobId && !mutation && (draft || status)), visitKey: JSON.stringify([context.jobId, context.navigation]) };
  };
  const read = async (context: Context) => {
    const request = ++generation;
    options.loading();
    let status: TranslationStatus;
    try { status = await options.backend.status(context.jobId); }
    catch (error) {
      if (!current(context) || request !== generation) return false;
      throw error;
    }
    if (!current(context) || request !== generation) return false;
    latest = { context, status };
    options.accepted(status); changed();
    return true;
  };
  const clearAppliedDraft = (ticket: Mutation) => {
    if (ticket.context.jobId && drafts.get(ticket.context.jobId)?.version === ticket.version) drafts.delete(ticket.context.jobId);
  };
  const api = {
    state,
    choose(value: string) {
      if (disposed) return;
      const context = options.context();
      if (disposed || !context.jobId || !validProvider(value)) return;
      drafts.set(context.jobId, { provider: value, version: ++editVersion }); changed();
    },
    async refresh(): Promise<boolean> {
      if (disposed) return false;
      const context = { ...options.context() }, visit = presentation;
      if (context.jobId && pending.has(context.jobId)) { options.loading(); changed(); return false; }
      try { return await read(context); }
      catch (error) {
        // A newer read owns the message too, including errors from old visits.
        if (current(context) && visit === presentation) options.message(`无法读取翻译配置：${String(error)}`, true);
        return false;
      }
    },
    async apply(): Promise<void> {
      if (disposed) return;
      const context = { ...options.context() }, snapshot = state();
      if (!snapshot.canApply || !context.jobId) return;
      const ticket: Mutation = { context, provider: snapshot.selected, version: drafts.get(context.jobId)?.version ?? 0, presentation };
      pending.set(context.jobId, ticket); ++generation; changed();
      let committed = false;
      try {
        await options.backend.select(context.jobId, ticket.provider);
        committed = true;
        if (!current(context)) {
          if (!disposed && options.context().jobId !== context.jobId) clearAppliedDraft(ticket);
          return;
        }
        const accepted = await read(context);
        if (accepted) clearAppliedDraft(ticket);
        if (accepted && ticket.presentation === presentation) options.message("本任务后端已更新；已有字幕未改变。");
      } catch (error) {
        if (current(context) && ticket.presentation === presentation) options.message(
          committed ? `本任务后端已保存，但状态读取失败：${String(error)}` : `设置任务后端失败：${String(error)}`, true,
        );
      } finally {
        if (pending.get(context.jobId) === ticket) pending.delete(context.jobId);
        changed();
        if (!disposed) {
          const now = options.context();
          if (now.jobId === context.jobId && now.navigation !== context.navigation) {
            if (await api.refresh() && committed) clearAppliedDraft(ticket);
            changed();
          }
        }
      }
    },
    closed() { ++presentation; },
    dispose() { disposed = true; ++generation; drafts.clear(); pending.clear(); latest = null; },
  };
  return api;
}
