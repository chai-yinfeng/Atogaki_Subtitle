#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub video_crf: u8,
    pub video_preset: String,
    pub target_video_bitrate_bps: Option<u64>,
    pub soft_subtitles: bool,
}
