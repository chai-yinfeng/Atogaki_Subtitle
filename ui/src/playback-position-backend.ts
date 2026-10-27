type Invoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
/** Persisted listening position only; the shared media player owns playback. */
export function createPlaybackPositionBackend(invoke: Invoke) {
  return {
    read(jobId: string) { return invoke<number>("get_playback_position", { jobId }); },
    save(jobId: string, positionMs: number) {
      return invoke<void>("save_playback_position", { request: { jobId, positionMs } });
    },
  };
}
