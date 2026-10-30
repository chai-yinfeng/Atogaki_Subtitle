import type { MediaCapabilities, VideoRender, VideoRenderEstimate, VideoOutputSelection, VideoRenderQuality } from "./video-render-types";
export function createVideoRenderBackend(invoke: <T>(command: string, args?: Record<string, unknown>) => Promise<T>) {
  return {
    capabilities: () => invoke<MediaCapabilities>("media_capabilities"),
    estimates: (sourceJobId: string) => invoke<VideoRenderEstimate[]>("video_render_estimates", { sourceJobId }),
    list: (sourceJobId: string) => invoke<VideoRender[]>("list_video_renders", { sourceJobId }),
    pick: (initialDirectory: string | null, suggestedName: string) => invoke<VideoOutputSelection | null>("pick_video_output_file", { initialDirectory, suggestedName }),
    outputExists: (path: string) => invoke<boolean>("video_output_exists", { path }),
    submit: (request: { sourceJobId: string; outputPath: string; subtitleTrack: VideoRender["subtitle_track"]; quality: VideoRenderQuality; overwriteExisting: boolean }) => invoke<VideoRender>("submit_video_render", { request }),
    cancel: (renderId: string) => invoke<VideoRender>("cancel_video_render", { renderId }),
    reveal: (path: string) => invoke<void>("reveal_rendered_video", { path }),
  };
}
