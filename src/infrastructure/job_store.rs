use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::{
    application::{
        TranscriptionOptions,
        job_manifest::{JobManifest, job_id_from_dir},
    },
    domain::TranscriptSegment,
};

#[derive(Debug, Clone)]
pub struct Job {
    pub dir: PathBuf,
    pub prefix: PathBuf,
    pub audio_wav: PathBuf,
    pub segments_json: PathBuf,
    pub source_srt: PathBuf,
    pub translated_srt: PathBuf,
    pub bilingual_srt: PathBuf,
    pub bilingual_ass: PathBuf,
    pub status_json: PathBuf,
    pub recognition_options_json: PathBuf,
    pub whisper_prompt_txt: PathBuf,
}

impl Job {
    pub fn create(output_dir: Option<&Path>) -> Result<Self> {
        let dir = match output_dir {
            Some(path) => path.to_path_buf(),
            None => return Self::create_in(Path::new("atogaki_jobs")),
        };
        fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
        Ok(Self::paths(dir))
    }

    pub fn create_in(jobs_dir: &Path) -> Result<Self> {
        let dir = jobs_dir.join(format!("job-{}", Uuid::new_v4()));
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

    pub fn write_manifest(&self, manifest: &JobManifest) -> Result<()> {
        let data = serde_json::to_vec_pretty(manifest)?;
        fs::write(&self.status_json, data)
            .with_context(|| format!("failed to write {}", self.status_json.display()))
    }

    pub fn write_recognition_options(&self, options: &TranscriptionOptions) -> Result<()> {
        let data = serde_json::to_vec_pretty(options)?;
        fs::write(&self.recognition_options_json, data).with_context(|| {
            format!(
                "failed to write {}",
                self.recognition_options_json.display()
            )
        })
    }

    pub fn read_recognition_options(&self) -> Result<TranscriptionOptions> {
        let data = fs::read(&self.recognition_options_json).with_context(|| {
            format!("failed to read {}", self.recognition_options_json.display())
        })?;
        serde_json::from_slice(&data).with_context(|| {
            format!(
                "failed to parse {}",
                self.recognition_options_json.display()
            )
        })
    }

    pub fn write_whisper_prompt(&self, prompt: Option<&str>) -> Result<()> {
        fs::write(&self.whisper_prompt_txt, prompt.unwrap_or_default())
            .with_context(|| format!("failed to write {}", self.whisper_prompt_txt.display()))
    }

    pub fn read_manifest_if_exists(&self) -> Result<Option<JobManifest>> {
        if !self.status_json.exists() {
            return Ok(None);
        }

        let data = fs::read(&self.status_json)
            .with_context(|| format!("failed to read {}", self.status_json.display()))?;
        serde_json::from_slice(&data)
            .with_context(|| format!("failed to parse {}", self.status_json.display()))
            .map(Some)
    }

    pub fn id(&self) -> String {
        job_id_from_dir(&self.dir)
    }

    pub fn read_segments(&self) -> Result<Vec<TranscriptSegment>> {
        let data = fs::read(&self.segments_json)
            .with_context(|| format!("failed to read {}", self.segments_json.display()))?;
        let mut segments: Vec<TranscriptSegment> =
            serde_json::from_slice(&data).context("failed to parse segments.json")?;
        if segments.iter_mut().any(TranscriptSegment::ensure_id) {
            self.write_segments(&segments)?;
        }
        Ok(segments)
    }

    fn paths(dir: PathBuf) -> Self {
        Self {
            prefix: dir.join("whisper"),
            audio_wav: dir.join("audio.wav"),
            segments_json: dir.join("segments.json"),
            source_srt: dir.join("source.srt"),
            translated_srt: dir.join("translation.srt"),
            bilingual_srt: dir.join("bilingual.srt"),
            bilingual_ass: dir.join("bilingual.ass"),
            status_json: dir.join("status.json"),
            recognition_options_json: dir.join("recognition-options.json"),
            whisper_prompt_txt: dir.join("whisper-prompt.txt"),
            dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::Job;

    #[test]
    fn creates_uuid_named_jobs_in_the_requested_root() {
        let root =
            std::env::temp_dir().join(format!("atogaki-job-store-test-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();

        assert_eq!(job.dir.parent(), Some(root.as_path()));
        assert!(job.id().strip_prefix("job-").is_some());
        assert!(job.dir.is_dir());

        fs::remove_dir_all(root).unwrap();
    }
}
