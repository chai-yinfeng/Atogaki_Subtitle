type Context = { jobId: string; navigation: number };
type DetailIdentity = { job: { job_id: string } };
type Options<D extends DetailIdentity> = {
  context(): Context | null;
  read(jobId: string): Promise<D>;
  apply(detail: D): void;
  failed(error: unknown): void;
};
type Timers = { repeat(callback: () => void): number; cancel(id: number): void };

/** A mounted progress page has one reader; departed visits never receive its result. */
export function createTaskProgress<D extends DetailIdentity>(options: Options<D>, timers: Timers = {
  repeat: callback => window.setInterval(callback, 2_000), cancel: id => window.clearInterval(id),
}) {
  let timer: number | null = null, generation = 0, running = false, disposed = false;
  const refresh = async () => {
    if (disposed || timer === null || running) return;
    const context = options.context();
    if (!context) return;
    const request = generation;
    const current = () => {
      if (disposed || timer === null || request !== generation) return false;
      const now = options.context();
      return now?.jobId === context.jobId && now.navigation === context.navigation;
    };
    running = true;
    try {
      const detail = await options.read(context.jobId);
      if (!current()) return;
      if (detail.job.job_id !== context.jobId) throw new Error("任务身份不匹配，请重新打开任务。");
      options.apply(detail);
    } catch (error) {
      if (current()) options.failed(error);
    } finally { running = false; }
  };
  const unmount = () => {
    generation++;
    if (timer !== null) timers.cancel(timer);
    timer = null;
  };
  return {
    mount() {
      if (disposed || timer !== null) return;
      generation++;
      timer = timers.repeat(() => { void refresh(); });
    },
    unmount,
    dispose() { disposed = true; unmount(); },
  };
}
