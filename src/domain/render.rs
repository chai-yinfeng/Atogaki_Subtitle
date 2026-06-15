#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub video_crf: u8,
    pub video_preset: String,
    pub soft_subtitles: bool,
}
