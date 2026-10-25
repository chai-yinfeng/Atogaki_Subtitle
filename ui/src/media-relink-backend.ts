/** Fields used from the existing LocalJobRecord response; no new persistence schema. */
export type RelinkedMediaJob = { job_id: string; input_path: string | null };
export function createMediaRelinkBackend(invoke: <T>(command: string, args: Record<string, unknown>) => Promise<T>) {
  return { relink: (jobId: string) => invoke<RelinkedMediaJob | null>("relink_job_media", { jobId }) };
}
