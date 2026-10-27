type DetailIdentity = { job: { job_id: string } };
type Options<D extends DetailIdentity> = {
  begin(): number;
  current(navigation: number): boolean;
  stop(): Promise<void>;
  prepare(jobId: string): void;
  read(jobId: string): Promise<D>;
  editable(detail: D): boolean;
  progress(jobId: string, detail: D): Promise<void>;
  apply(detail: D): void;
  waveform(): Promise<void>;
  processingRange(): void;
  signals(jobId: string): Promise<void>;
  renderList(): void;
  translation(): Promise<void>;
  failed(error: unknown): void;
};

/** Opens an editor visit using the shared navigation clock and existing services. */
export function createEditorNavigation<D extends DetailIdentity>(options: Options<D>) {
  let disposed = false;
  return {
    async open(jobId: string, loadedDetail?: D): Promise<number | null> {
      if (disposed || !jobId) return null;
      const navigation = options.begin();
      const current = () => !disposed && options.current(navigation);
      // A failed draft/time preflight must propagate without tearing down the view.
      await options.stop();
      if (!current()) return null;
      options.prepare(jobId);
      try {
        const detail = loadedDetail ?? await options.read(jobId);
        if (!current()) return null;
        if (detail.job.job_id !== jobId) throw new Error("字幕任务身份不匹配，请重新打开任务。");
        if (!options.editable(detail)) { await options.progress(jobId, detail); return null; }
        options.apply(detail);
        await options.waveform();
        if (!current()) return null;
        options.processingRange();
        await options.signals(jobId);
        if (!current()) return null;
        options.renderList();
        await options.translation();
        return current() ? navigation : null;
      } catch (error) {
        if (current()) options.failed(error);
        return null;
      }
    },
    dispose() { disposed = true; },
  };
}
