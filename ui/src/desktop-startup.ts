type StartupJob = { name: string; run(current: () => boolean): Promise<void> };
type Options = {
  jobs: readonly StartupJob[];
  directory(): Promise<string>;
  directoryAccepted(path: string): void;
  directoryFailed(): void;
  failed(name: string, error: unknown): void;
};

/** Starts independent reads once; service-owned loaders retain their own generations. */
export function createDesktopStartup(options: Options) {
  let mounted = false, disposed = false;
  const current = () => mounted && !disposed;
  return {
    get active() { return current(); },
    mount() {
      if (mounted || disposed) return;
      mounted = true;
      for (const job of options.jobs) {
        try {
          void job.run(current).catch(error => { if (current()) options.failed(job.name, error); });
        } catch (error) { if (current()) options.failed(job.name, error); }
      }
      try {
        void options.directory().then(path => {
          if (current()) options.directoryAccepted(path);
        }).catch(() => { if (current()) options.directoryFailed(); });
      } catch { if (current()) options.directoryFailed(); }
    },
    dispose() { disposed = true; },
  };
}
