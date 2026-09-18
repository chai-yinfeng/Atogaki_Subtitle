use std::{fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::{
    application::{AsrRun, CandidateCueSet, QualitySignal, TimedUnit},
    infrastructure::job_store::Job,
};

#[derive(Debug, Clone)]
pub struct AsrRunArtifacts {
    pub dir: PathBuf,
    pub run_json: PathBuf,
    pub provider_output_prefix: PathBuf,
    pub timed_units_json: PathBuf,
    pub candidate_cues_json: PathBuf,
    pub quality_signals_json: PathBuf,
}

impl AsrRunArtifacts {
    pub fn create(job: &Job, run: &AsrRun) -> Result<Self> {
        if run.job_id != job.id() {
            return Err(anyhow!("ASR run belongs to a different task"));
        }
        let artifacts = Self::paths(job, &run.id)?;
        fs::create_dir_all(&artifacts.dir).with_context(|| {
            format!(
                "failed to create ASR run directory {}",
                artifacts.dir.display()
            )
        })?;
        artifacts.write_run(run)?;
        Ok(artifacts)
    }

    pub fn open(job: &Job, run_id: &str) -> Result<Self> {
        let artifacts = Self::paths(job, run_id)?;
        if !artifacts.dir.is_dir() {
            return Err(anyhow!(
                "ASR run directory does not exist: {}",
                artifacts.dir.display()
            ));
        }
        Ok(artifacts)
    }

    pub fn write_run(&self, run: &AsrRun) -> Result<()> {
        write_json(&self.run_json, run)
    }

    pub fn read_run(&self) -> Result<AsrRun> {
        let data = fs::read(&self.run_json)
            .with_context(|| format!("failed to read {}", self.run_json.display()))?;
        serde_json::from_slice(&data)
            .with_context(|| format!("failed to parse {}", self.run_json.display()))
    }

    pub fn write_timed_units(&self, units: &[TimedUnit]) -> Result<()> {
        write_json(&self.timed_units_json, units)
    }

    pub fn write_candidate_cues(&self, cues: &CandidateCueSet) -> Result<()> {
        write_json(&self.candidate_cues_json, cues)
    }

    pub fn read_candidate_cues(&self) -> Result<CandidateCueSet> {
        let data = fs::read(&self.candidate_cues_json)
            .with_context(|| format!("failed to read {}", self.candidate_cues_json.display()))?;
        serde_json::from_slice(&data)
            .with_context(|| format!("failed to parse {}", self.candidate_cues_json.display()))
    }

    pub fn write_quality_signals(&self, signals: &[QualitySignal]) -> Result<()> {
        write_json(&self.quality_signals_json, signals)
    }

    fn paths(job: &Job, run_id: &str) -> Result<Self> {
        if run_id.is_empty()
            || run_id == "."
            || run_id == ".."
            || run_id.contains('/')
            || run_id.contains('\\')
        {
            return Err(anyhow!("invalid ASR run id"));
        }
        let dir = job.asr_runs_dir.join(run_id);
        Ok(Self {
            run_json: dir.join("run.json"),
            provider_output_prefix: dir.join("provider-output"),
            timed_units_json: dir.join("timed-units.json"),
            candidate_cues_json: dir.join("candidate-cues.json"),
            quality_signals_json: dir.join("quality-signals.json"),
            dir,
        })
    }
}

fn write_json(path: &std::path::Path, value: &(impl serde::Serialize + ?Sized)) -> Result<()> {
    let data = serde_json::to_vec_pretty(value)?;
    fs::write(path, data).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use crate::{
        application::{AsrInputScope, AsrRun, TimedUnit, TimedUnitKind, TimingSource},
        infrastructure::{asr_run_store::AsrRunArtifacts, job_store::Job},
    };

    #[test]
    fn creates_the_versioned_run_artifact_layout() {
        let root = std::env::temp_dir().join(format!("atogaki-asr-run-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let mut run = AsrRun::new(
            job.id(),
            None,
            "whisper.cpp",
            "Whisper.cpp",
            "model.bin",
            "video.mp4".into(),
            AsrInputScope::full(),
            json!({}),
        );
        let artifacts = AsrRunArtifacts::create(&job, &run).unwrap();
        run.start();
        artifacts.write_run(&run).unwrap();
        artifacts
            .write_timed_units(&[TimedUnit {
                id: "unit-1".into(),
                text: "test".into(),
                kind: TimedUnitKind::ProviderSegment,
                start_ms: Some(0),
                end_ms: Some(1000),
                timing_source: TimingSource::Model,
                provider_confidence: None,
                speaker: None,
            }])
            .unwrap();
        artifacts
            .write_candidate_cues(&crate::application::CandidateCueSet {
                schema_version: 1,
                segmentation_policy: "legacy-v1".into(),
                cues: Vec::new(),
            })
            .unwrap();

        assert!(artifacts.run_json.is_file());
        assert!(artifacts.timed_units_json.is_file());
        assert!(artifacts.candidate_cues_json.is_file());
        assert_eq!(
            AsrRunArtifacts::open(&job, &run.id).unwrap().dir,
            artifacts.dir
        );
        fs::remove_dir_all(root).unwrap();
    }
}
