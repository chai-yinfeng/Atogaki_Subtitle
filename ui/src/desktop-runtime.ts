type Release = () => void | Promise<void>;
type CloseRequest = { preventDefault(): void };
type Handler<T> = (value: T) => void | Promise<void>;
type Options<A> = {
  refreshTasks(): Promise<void>;
  updateTimings(): void;
  subscribeVisibility(handler: Handler<boolean>): Promise<Release>;
  subscribePlayback(handler: Handler<A>): Promise<Release>;
  subscribeClose(handler: Handler<CloseRequest>): Promise<Release>;
  visibility: Handler<boolean>;
  playback: Handler<A>;
  close: Handler<CloseRequest>;
  failed(error: unknown): void;
};
type Timers = { repeat(callback: () => void, delayMs: number): number; cancel(id: number): void };

/** Owns desktop-wide polling and async event registrations for one page lifetime. */
export function createDesktopRuntime<A>(options: Options<A>, timers: Timers = {
  repeat: (callback, delayMs) => window.setInterval(callback, delayMs), cancel: id => window.clearInterval(id),
}) {
  let mounted = false, disposed = false;
  const timerIds: number[] = [], releases: Release[] = [];
  const active = () => mounted && !disposed;
  const failed = (error: unknown) => { if (active()) options.failed(error); };
  const run = <T>(handler: Handler<T>, value: T) => {
    if (!active()) return;
    try { return Promise.resolve(handler(value)).catch(failed); }
    catch (error) { failed(error); }
  };
  const releaseOne = (release: Release) => {
    try { void Promise.resolve(release()).catch(failed); }
    catch (error) { failed(error); }
  };
  const register = <T>(subscribe: (handler: Handler<T>) => Promise<Release>, handler: Handler<T>) => {
    try {
      void subscribe(value => run(handler, value)).then(release => {
        if (!active()) releaseOne(release); else releases.push(release);
      }).catch(failed);
    } catch (error) { failed(error); }
  };
  return {
    mount() {
      if (mounted || disposed) return;
      mounted = true;
      timerIds.push(timers.repeat(() => { void run(options.refreshTasks, undefined); }, 2_000));
      timerIds.push(timers.repeat(() => { void run(options.updateTimings, undefined); }, 1_000));
      register(options.subscribeVisibility, options.visibility);
      register(options.subscribePlayback, options.playback);
      register(options.subscribeClose, options.close);
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const id of timerIds.splice(0)) timers.cancel(id);
      for (const release of releases.splice(0)) releaseOne(release);
    },
  };
}
