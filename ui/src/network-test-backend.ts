export type NetworkTestRequest = { proxyMode: string; proxyUrl: string | null; modelMirrorUrl: string | null };
export type NetworkSourceCheck = { label: string; requestedUrl: string; ok: boolean; status: number | null; resolvedHost: string | null; error: string | null };

export function createNetworkTestBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return { test: (request: NetworkTestRequest) => invoke<NetworkSourceCheck[]>("test_network_connection", { request }) };
}
