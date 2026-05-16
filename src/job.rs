use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

use crate::segment::TranscriptSegment;

#[derive(Debug, Clone)]
pub struct Job {
    pub dir: PathBuf,
    pub prefix: PathBuf,
    pub audio_wav: PathBuf,
    pub segments_json: PathBuf,
    pub ja_srt: PathBuf,
    pub zh_srt: PathBuf,
    pub bilingual_srt: PathBuf,
    pub bilingual_ass: PathBuf,
}

impl Job {
    pub fn create(output_dir: Option<&Path>) -> Result<Self> {
        let dir = match output_dir {
            Some(path) => path.to_path_buf(),
            None => {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .context("system clock is before unix epoch")?
                    .as_secs();
                PathBuf::from("atogaki_jobs").join(format!("job-{ts}"))
            }
        };
        fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
        Ok(Self::paths(dir))
    }

    pub fn open(dir: PathBuf) -> Result<Self> {
        if !dir.is_dir() {
            anyhow::bail!("job directory does not exist: {}", dir.display());
        }
        Ok(Self::paths(dir))
    }

    pub fn write_segments(&self, segments: &[TranscriptSegment]) -> Result<()> {
        let data = serde_json::to_vec_pretty(segments)?;
        fs::write(&self.segments_json, data)
            .with_context(|| format!("failed to write {}", self.segments_json.display()))
    }

    pub fn read_segments(&self) -> Result<Vec<TranscriptSegment>> {
        let data = fs::read(&self.segments_json)
            .with_context(|| format!("failed to read {}", self.segments_json.display()))?;
        serde_json::from_slice(&data).context("failed to parse segments.json")
    }

    fn paths(dir: PathBuf) -> Self {
        Self {
            prefix: dir.join("whisper"),
            audio_wav: dir.join("audio.wav"),
            segments_json: dir.join("segments.json"),
            ja_srt: dir.join("ja.srt"),
            zh_srt: dir.join("zh.srt"),
            bilingual_srt: dir.join("bilingual.srt"),
            bilingual_ass: dir.join("bilingual.ass"),
            dir,
        }
    }
}
