export type TranslationContextState = { workspace_revision: number; translation_context_stale_ids: string[] };
type Context = { jobId: string; navigation: number };
type Ticket = Context & { generation: number };
type Options = {
  context(): Context | null;
  ids(): readonly string[];
  read(jobId: string): Promise<TranslationContextState>;
  apply(state: TranslationContextState, changedIds: readonly string[]): void;
  failed(message: string): void;
};
/** On-demand, coalesced context metadata. Never owns cues, drafts or a timer. */
export function createEditorContextStatus(options: Options) {
  let generation = 0, disposed = false, running = false, pending: Ticket | null = null;
  const current = (ticket: Ticket) => {
    if (disposed || ticket.generation !== generation) return false;
    const now = options.context();
    return now?.jobId === ticket.jobId && now.navigation === ticket.navigation;
  };
  const pump = async () => {
    if (disposed || running || !pending) return;
    const ticket = pending; pending = null; running = true;
    try {
      const state = await options.read(ticket.jobId);
      if (!current(ticket)) return;
      const before = new Set(options.ids()), after = new Set(state.translation_context_stale_ids);
      const changed = [...before].filter(id => !after.has(id)).concat([...after].filter(id => !before.has(id)));
      if (changed.length) options.apply(state, changed);
    } catch (error) {
      if (current(ticket)) options.failed(`文字已保存，但翻译上下文状态暂未同步；请重新打开任务核对。${String(error)}`);
    } finally {
      running = false;
      if (!disposed && pending) void pump();
    }
  };
  return {
    refresh(jobId: string) {
      if (disposed) return;
      const context = options.context();
      if (!context || context.jobId !== jobId) return;
      pending = { ...context, generation: ++generation };
      void pump();
    },
    /** New detail/translation/structure state supersedes any older status read. */
    invalidate() { generation++; pending = null; },
    dispose() { disposed = true; generation++; pending = null; },
  };
}
