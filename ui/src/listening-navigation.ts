type DetailIdentity = { job: { job_id: string } };
type Options<D extends DetailIdentity> = {
  begin(): number;
  current(navigation: number): boolean;
  stop(): Promise<void>;
  prepare(): void;
  detail(jobId: string): Promise<D>;
  position(jobId: string): Promise<number>;
  learning(): Promise<void>;
  apply(detail: D, resumeMs: number): void;
  play(): Promise<void>;
  failed(error: unknown): void;
};

/** Coordinates listening visits without owning a second player or media state. */
export function createListeningNavigation<D extends DetailIdentity>(options: Options<D>) {
  let disposed = false;
  return {
    async open(jobId: string, resumeOverrideMs: number | null = null, autoplay = false): Promise<void> {
      if (disposed || !jobId) return;
      const navigation = options.begin();
      const current = () => !disposed && options.current(navigation);
      await options.stop();
      if (!current()) return;
      options.prepare();
      try {
        const [detail, savedResumeMs] = await Promise.all([options.detail(jobId), options.position(jobId)]);
        if (!current()) return;
        if (detail.job.job_id !== jobId) throw new Error("节目任务身份不匹配，请重新打开节目。");
        await options.learning();
        if (!current()) return;
        options.apply(detail, resumeOverrideMs ?? savedResumeMs);
        if (autoplay) await options.play();
      } catch (error) {
        if (current()) options.failed(error);
      }
    },
    dispose() { disposed = true; },
  };
}
