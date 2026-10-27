export type TaskSection = "review" | "translation" | "export";
type DetailIdentity = { job: { job_id: string } };
type Options<D extends DetailIdentity> = {
  begin(): number;
  current(navigation: number): boolean;
  visible(): boolean;
  stop(): Promise<void>;
  knownEditable(jobId: string): boolean;
  read(jobId: string): Promise<D>;
  editable(detail: D): boolean;
  editor(jobId: string, detail?: D): Promise<void>;
  prepare(section: TaskSection): void;
  accept(detail: D): void;
  render(detail: D): void;
  translation(): Promise<void>;
  failed(error: unknown): void;
};

/** Routes task opens from summaries, then trusts the loaded detail over the card. */
export function createTaskNavigation<D extends DetailIdentity>(options: Options<D>) {
  let disposed = false;
  return {
    async open(jobId: string, section?: TaskSection, loadedDetail?: D): Promise<void> {
      if (disposed || !jobId) return;
      const defaultEditor = section === undefined || section === "review";
      if (defaultEditor && (loadedDetail ? loadedDetail.job.job_id === jobId && options.editable(loadedDetail) : options.knownEditable(jobId))) {
        await options.editor(jobId, loadedDetail);
        return;
      }
      const navigation = options.begin();
      const current = () => !disposed && options.current(navigation);
      // Preserve the existing view when drafts, IME or pending time block leaving.
      await options.stop();
      if (!current()) return;
      options.prepare(section ?? "review");
      const active = () => current() && options.visible();
      try {
        const detail = loadedDetail ?? await options.read(jobId);
        if (!active()) return;
        if (detail.job.job_id !== jobId) throw new Error("任务身份不匹配，请重新打开任务。");
        options.accept(detail);
        if (defaultEditor && options.editable(detail)) { await options.editor(jobId, detail); return; }
        options.render(detail);
        await options.translation();
      } catch (error) {
        if (active()) options.failed(error);
      }
    },
    dispose() { disposed = true; },
  };
}
