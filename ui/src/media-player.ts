type Source = {
  jobId: string;
  inputPath: string | null;
  primaryPath: string | null;
  fallbackPath: string | null;
  host: HTMLElement;
  message: HTMLElement;
  resumeMs: number;
  playbackRate: number;
};
type Options = {
  currentJob(jobId: string): boolean;
  resolve(path: string): string;
  isAudio(path: string): boolean;
  time(ms: number): string;
  position(ms: number): void;
  progress(): void;
  changed(): void;
  persist(): void;
  frameEnabled(): boolean;
};
type Environment = {
  create(kind: "audio" | "video"): HTMLMediaElement;
  requestFrame(callback: FrameRequestCallback): number;
  cancelFrame(id: number): void;
};

/** One shared player for editor and listening; all element events are disposable. */
export function createMediaPlayer(options: Options, environment: Environment = {
  create: kind => document.createElement(kind),
  requestFrame: callback => window.requestAnimationFrame(callback),
  cancelFrame: id => window.cancelAnimationFrame(id),
}) {
  let media: HTMLMediaElement | null = null;
  let events: AbortController | null = null;
  let generation = 0;
  let frame: number | null = null;
  let disposed = false;
  let currentGuard = () => false;
  let requestSeek = (_milliseconds: number) => {};
  let position = () => media ? media.currentTime * 1_000 : 0;
  const cancelAnimation = () => { if (frame !== null) environment.cancelFrame(frame); frame = null; };
  const take = (invalidate = true) => {
    if (invalidate) generation++;
    events?.abort(); events = null; cancelAnimation();
    const previous = media; media = null; currentGuard = () => false;
    requestSeek = () => {}; position = () => 0;
    previous?.pause();
    return previous;
  };
  const release = (element: HTMLMediaElement | null) => {
    if (!element) return;
    element.removeAttribute("src"); element.load(); element.remove();
  };
  const refreshAnimation = () => {
    if (frame !== null || disposed || !media || media.paused || !options.frameEnabled() || !currentGuard()) return;
    const element = media, request = generation;
    frame = environment.requestFrame(() => {
      frame = null;
      if (disposed || media !== element || generation !== request || element.paused || !options.frameEnabled() || !currentGuard()) return;
      options.position(element.currentTime * 1_000);
      refreshAnimation();
    });
  };
  const mount = (source: Source) => {
    if (disposed) return;
    release(take());
    const request = generation;
    source.host.replaceChildren(); options.changed();
    const firstPath = source.primaryPath ?? source.fallbackPath;
    if (!firstPath) { source.message.textContent = "没有找到可播放的本地媒体；任务可能仍在处理或源文件已移动。"; return; }
    const loadPath = (path: string, fallback: boolean, positionMs = source.resumeMs, autoplay = false, volume = 1, muted = false, rate = source.playbackRate): void => {
      release(take(false));
      const element = environment.create(fallback || options.isAudio(source.inputPath ?? path) ? "audio" : "video");
      const abort = new AbortController(); events = abort;
      const current = () => !disposed && media === element && generation === request && options.currentJob(source.jobId);
      const on = (name: string, callback: EventListener) => element.addEventListener(name, callback, { signal: abort.signal });
      let shouldPlay = autoplay, metadataLoaded = false;
      // WebKit can settle overlapping native seeks out of order. Keep one in
      // flight and retain only the latest requested destination.
      let pendingSeek: number | null = null, seekingTarget: number | null = null;
      const intendedPosition = () => pendingSeek ?? seekingTarget ?? element.currentTime * 1_000;
      const flushSeek = () => {
        if (!current() || !metadataLoaded || pendingSeek === null || seekingTarget !== null || element.seeking) return;
        const duration = Number.isFinite(element.duration) ? element.duration * 1_000 : pendingSeek;
        const target = Math.max(0, Math.min(pendingSeek, duration));
        pendingSeek = null;
        if (Math.abs(element.currentTime * 1_000 - target) < 1) return;
        seekingTarget = target;
        element.currentTime = target / 1_000;
      };
      const seek = (milliseconds: number) => {
        if (!current() || !Number.isFinite(milliseconds)) return;
        const duration = metadataLoaded && Number.isFinite(element.duration) ? element.duration * 1_000 : Infinity;
        pendingSeek = Math.max(0, Math.min(milliseconds, duration)); flushSeek();
      };
      const fallbackMessage = "原素材播放失败，已自动使用任务音频。";
      element.controls = true; element.preload = "metadata";
      on("timeupdate", () => { if (current() && pendingSeek === null && seekingTarget === null) { options.position(element.currentTime * 1_000); options.progress(); } });
      on("seeked", () => {
        if (!current()) return;
        seekingTarget = null; flushSeek();
        if (pendingSeek === null && seekingTarget === null) options.position(element.currentTime * 1_000);
      });
      on("play", () => { if (current()) { shouldPlay = true; options.changed(); refreshAnimation(); } });
      on("pause", () => {
        if (!current()) return;
        if (!element.error) shouldPlay = false;
        cancelAnimation(); options.changed(); options.persist();
      });
      on("ended", () => { if (current()) { cancelAnimation(); options.changed(); options.persist(); } });
      on("ratechange", () => {
        if (!current()) return;
        if (metadataLoaded && element.defaultPlaybackRate !== element.playbackRate) element.defaultPlaybackRate = element.playbackRate;
        options.changed();
      });
      on("loadedmetadata", () => {
        if (!current()) return;
        metadataLoaded = true;
        element.playbackRate = element.defaultPlaybackRate;
        if (pendingSeek === null && positionMs > 0 && Number.isFinite(element.duration)) pendingSeek = Math.min(positionMs, Math.max(0, element.duration * 1_000 - 250));
        flushSeek();
        if (shouldPlay) void element.play().catch(() => { if (current()) source.message.textContent += " · 请点击播放"; });
        if (positionMs > 0) source.message.textContent = `${fallback ? fallbackMessage : source.inputPath ?? "原素材"} · 已恢复到 ${options.time(element.currentTime * 1_000)}`;
        options.changed(); options.position(element.currentTime * 1_000);
      });
      on("error", () => {
        if (!current()) return;
        if (!fallback && source.fallbackPath && source.fallbackPath !== path) {
          loadPath(source.fallbackPath, true, pendingSeek ?? seekingTarget ?? (metadataLoaded ? element.currentTime * 1_000 : positionMs), shouldPlay, element.volume, element.muted, element.playbackRate);
        } else {
          cancelAnimation(); element.pause();
          source.message.textContent = "媒体加载失败。可能是文件已移动，或系统 WebView 不支持这种编码。";
        }
      });
      media = element; currentGuard = current; requestSeek = seek; position = intendedPosition;
      element.defaultPlaybackRate = rate; element.playbackRate = rate; element.volume = volume; element.muted = muted;
      source.host.replaceChildren(element);
      element.src = /^http:\/\/127\.0\.0\.1:\d+\/media\/[a-f0-9-]+$/.test(path) ? path : options.resolve(path);
      options.changed();
      const display = source.inputPath ?? path;
      source.message.textContent = !fallback
        ? positionMs > 0 ? `${display} · 已恢复到 ${options.time(positionMs)}` : display
        : `${source.primaryPath ? fallbackMessage : "原素材不可用，已使用任务音频。"} · ${options.time(positionMs)}`;
    };
    loadPath(firstPath, source.primaryPath === null && firstPath === source.fallbackPath);
  };
  return {
    get media() { return media; }, get positionMs() { return position(); },
    seek(milliseconds: number) { requestSeek(milliseconds); }, mount, refreshAnimation,
    setRate(rate: number) { if (media) { media.defaultPlaybackRate = rate; media.playbackRate = rate; } },
    async stop(beforeRelease?: (element: HTMLMediaElement, positionMs: number) => Promise<void>) {
      const positionMs = position(), previous = take(); options.changed();
      try { if (previous) await beforeRelease?.(previous, positionMs); }
      finally { release(previous); }
    },
    dispose() { disposed = true; release(take()); },
  };
}
