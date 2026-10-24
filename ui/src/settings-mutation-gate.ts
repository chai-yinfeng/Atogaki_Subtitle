/** Shared settings/credential write ownership; stores no configuration or secrets. */
export function createSettingsMutationGate() {
  let busy = false, revision = 0;
  return {
    get busy() { return busy; },
    get revision() { return revision; },
    acquire(): (() => void) | null {
      if (busy) return null;
      busy = true; ++revision;
      let released = false;
      return () => { if (!released) { released = true; busy = false; ++revision; } };
    },
  };
}
export type SettingsMutationGate = ReturnType<typeof createSettingsMutationGate>;
