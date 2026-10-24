import type { NetworkSourceCheck, NetworkTestRequest } from "./network-test-backend.ts";
export type NetworkTestState = { phase: "idle" | "running" | "stale" | "result" | "error"; checks: NetworkSourceCheck[]; error: string };
type Options = {
  backend: { test(request: NetworkTestRequest): Promise<NetworkSourceCheck[]> };
  visit(): number; isOpen(): boolean;
  capture(): { request: NetworkTestRequest; current(): boolean } | null;
  changed(): void;
};

/** Diagnostic results belong to one settings visit and the exact tested parameters. */
export function createNetworkTestSession(options: Options) {
  let disposed = false, busy = false, generation = 0;
  let state: NetworkTestState = { phase: "idle", checks: [], error: "" };
  const change = (phase: NetworkTestState["phase"], checks: NetworkSourceCheck[] = [], error = "") => {
    state = { phase, checks, error }; options.changed();
  };
  return {
    get busy() { return busy; },
    get state(): NetworkTestState { return { ...state, checks: state.checks.map(check => ({ ...check })) }; },
    async run(): Promise<void> {
      if (disposed || busy || !options.isOpen()) return;
      const capture = options.capture(); if (!capture) return;
      const request = { ...capture.request }, visit = options.visit(), token = ++generation;
      const current = () => !disposed && token === generation && options.isOpen() && visit === options.visit();
      busy = true; change("running");
      try {
        const checks = await options.backend.test(request);
        if (!current()) return;
        if (!capture.current()) change("stale");
        else change("result", checks.map(check => ({ ...check })));
      } catch (error) {
        if (current()) {
          if (!capture.current()) change("stale");
          else change("error", [], String(error));
        }
      } finally {
        busy = false;
        if (!disposed) options.changed();
      }
    },
    parametersChanged() {
      if (disposed || state.phase === "idle") return;
      ++generation; change("stale");
    },
    hidden() { if (!disposed) { ++generation; change("idle"); } },
    dispose() { disposed = true; ++generation; busy = false; state = { phase: "idle", checks: [], error: "" }; },
  };
}
