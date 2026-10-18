//! Durable, independently verifiable acoustic chunks. Files are authoritative;
//! the SQLite index is repaired from committed evidence after an interruption.
use crate::{
    application::{
        AsrInputScope, AsrProvider, AsrRequest, AsrResponse, TimedUnit, TranscriptionOptions,
    },
    infrastructure::{job_store::Job, local_db::LocalDatabase, media},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

pub const ACOUSTIC_STRATEGY: &str = "owned-120s-context-5s-v1";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcousticChunk {
    pub index: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub read_start_ms: u64,
    pub read_end_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticPlan {
    pub version: u32,
    pub strategy: String,
    pub audio_sha256: String,
    pub configuration_sha256: String,
    pub duration_ms: u64,
    pub chunks: Vec<AcousticChunk>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkEvidence {
    chunk: AcousticChunk,
    units: Vec<TimedUnit>,
    device: String,
    raw_sha256: String,
    #[serde(default)]
    recognition_elapsed_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AudioIdentity {
    sha256: String,
    duration_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecoverySummary {
    pub phase: String,
    pub completed_ms: u64,
    pub duration_ms: u64,
    pub completed_ranges: Vec<(u64, u64)>,
    pub can_resume: bool,
    pub blocked_reason: Option<String>,
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("artifact has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut f = fs::File::create(&temp)?;
        f.write_all(data)?;
        f.sync_all()?;
        fs::rename(&temp, path)?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)?.sync_all()?;
    Ok(())
}
#[cfg(not(unix))]
fn sync_directory(_: &Path) -> Result<()> {
    Ok(())
}
pub fn write_json(path: &Path, value: &(impl Serialize + ?Sized)) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}
pub fn file_digest(path: &Path) -> Result<String> {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|e| e.file_name());
        let mut digest = Sha256::new();
        for entry in entries {
            let name = entry.file_name();
            if name == ".git" || name == ".cache" {
                continue;
            }
            if entry.file_type()?.is_symlink() && entry.path().is_dir() {
                bail!("symlinked model directories are not supported")
            }
            digest.update(name.to_string_lossy().as_bytes());
            digest.update([0]);
            digest.update(file_digest(&entry.path())?.as_bytes());
            digest.update([0]);
        }
        return Ok(format!("{:x}", digest.finalize()));
    }

    let mut f = fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn json_digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

/// Accept only the complete normalized mono PCM16 WAV that our extractor emits.
pub fn wav_duration(path: &Path) -> Result<u64> {
    let mut f = fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut header = [0; 12];
    f.read_exact(&mut header)?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
        bail!("expected a complete normalized RIFF WAV")
    }
    let mut format_ok = false;
    loop {
        let mut h = [0; 8];
        f.read_exact(&mut h)
            .context("WAV has no complete data chunk")?;
        let size = u32::from_le_bytes(h[4..].try_into().unwrap()) as u64;
        let start = f.stream_position()?;
        if start + size > len {
            bail!("truncated WAV")
        }
        if &h[..4] == b"fmt " {
            let mut fmt = [0; 16];
            if size < 16 {
                bail!("invalid WAV format")
            };
            f.read_exact(&mut fmt)?;
            format_ok = u16::from_le_bytes(fmt[..2].try_into().unwrap()) == 1
                && u16::from_le_bytes(fmt[2..4].try_into().unwrap()) == 1
                && u32::from_le_bytes(fmt[4..8].try_into().unwrap()) == 16000
                && u16::from_le_bytes(fmt[14..16].try_into().unwrap()) == 16;
        }
        if &h[..4] == b"data" {
            if !format_ok || size == 0 || size % 2 != 0 {
                bail!("WAV must be nonempty mono 16kHz PCM16")
            };
            return Ok(size * 1000 / 32000);
        }
        f.seek(SeekFrom::Start(start + size + (size % 2)))?;
    }
}
pub async fn prepare_audio(ffmpeg: &Path, input: &Path, job: &Job) -> Result<PathBuf> {
    let identity_path = job.dir.join("audio-identity.json");
    if identity_path.is_file() {
        let identity: AudioIdentity = serde_json::from_slice(&fs::read(&identity_path)?)?;
        if wav_duration(&job.audio_wav)? != identity.duration_ms
            || file_digest(&job.audio_wav)? != identity.sha256
        {
            bail!("任务音频校验失败；请恢复原文件，不能混用检查点")
        };
        return Ok(job.audio_wav.clone());
    }
    let temp = job.dir.join("audio-extract.part.wav");
    media::extract_wav(ffmpeg, input, &temp).await?;
    let identity = AudioIdentity {
        duration_ms: wav_duration(&temp)?,
        sha256: file_digest(&temp)?,
    };
    fs::File::open(&temp)?.sync_all()?;
    fs::rename(&temp, &job.audio_wav)?;
    sync_directory(&job.dir)?;
    write_json(&identity_path, &identity)?;
    Ok(job.audio_wav.clone())
}
impl AcousticPlan {
    pub fn new(duration_ms: u64, audio_sha256: String, configuration_sha256: String) -> Self {
        let chunks = (0..duration_ms)
            .step_by(120000)
            .enumerate()
            .map(|(index, start_ms)| {
                let end_ms = (start_ms + 120000).min(duration_ms);
                AcousticChunk {
                    index,
                    start_ms,
                    end_ms,
                    read_start_ms: start_ms.saturating_sub(5000),
                    read_end_ms: (end_ms + 5000).min(duration_ms),
                }
            })
            .collect();
        Self {
            version: 1,
            strategy: ACOUSTIC_STRATEGY.into(),
            audio_sha256,
            configuration_sha256,
            duration_ms,
            chunks,
        }
    }
}
fn evidence_path(root: &Path, index: usize) -> PathBuf {
    root.join(format!("chunk-{index:06}"))
}
fn read_evidence(root: &Path, chunk: &AcousticChunk) -> Result<ChunkEvidence> {
    let dir = evidence_path(root, chunk.index);
    let bytes = fs::read(dir.join("evidence.json"))?;
    let expected = fs::read_to_string(dir.join("sha256"))?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected {
        bail!("checkpoint digest mismatch")
    }
    let evidence: ChunkEvidence = serde_json::from_slice(&bytes)?;
    if &evidence.chunk != chunk
        || file_digest(&dir.join("provider-output.json"))? != evidence.raw_sha256
    {
        bail!("checkpoint identity mismatch")
    }
    for unit in &evidence.units {
        if unit
            .start_ms
            .zip(unit.end_ms)
            .is_none_or(|(s, e)| e <= s || e > chunk.read_end_ms || s < chunk.read_start_ms)
        {
            bail!("invalid checkpoint timestamps")
        }
    }
    Ok(evidence)
}
pub fn recovery_summary(job: &Job) -> Result<TaskRecoverySummary> {
    let phase = job
        .read_manifest_if_exists()?
        .map(|m| m.failed_stage.unwrap_or(m.status).as_str().to_string())
        .unwrap_or_else(|| "transcribing".into());
    let path = job.dir.join("asr-checkpoint/plan.json");
    if !path.is_file() {
        return Ok(TaskRecoverySummary {
            phase: if job.dir.join("workflow-version.json").exists() {
                "extracting_audio".into()
            } else {
                "legacy".into()
            },
            completed_ms: 0,
            duration_ms: 0,
            completed_ranges: vec![],
            can_resume: job.dir.join("workflow-version.json").exists(),
            blocked_reason: if job.dir.join("workflow-version.json").exists() {
                None
            } else {
                Some("该历史任务无法断点继续；可重新运行".into())
            },
        });
    }
    let plan: AcousticPlan = serde_json::from_slice(&fs::read(path)?)?;
    let completed_ranges = plan
        .chunks
        .iter()
        .filter(|c| read_evidence(&job.dir.join("asr-checkpoint"), c).is_ok())
        .map(|c| (c.start_ms, c.end_ms))
        .collect::<Vec<_>>();
    Ok(TaskRecoverySummary {
        phase,
        completed_ms: completed_ranges.iter().map(|(s, e)| e - s).sum(),
        duration_ms: plan.duration_ms,
        completed_ranges,
        can_resume: true,
        blocked_reason: None,
    })
}
pub fn partial_units(job: &Job) -> Result<Vec<TimedUnit>> {
    let root = job.dir.join("asr-checkpoint");
    let plan: AcousticPlan = serde_json::from_slice(&fs::read(root.join("plan.json"))?)?;
    let mut units = vec![];
    for chunk in &plan.chunks {
        if let Ok(e) = read_evidence(&root, chunk) {
            units.extend(
                owned_units(chunk, e.units)
                    .into_iter()
                    .map(|unit| (chunk.index, unit)),
            );
        }
    }
    Ok(stitch_units(units))
}
fn owned_units(chunk: &AcousticChunk, units: Vec<TimedUnit>) -> Vec<TimedUnit> {
    units
        .into_iter()
        .filter(|u| {
            u.start_ms.zip(u.end_ms).is_some_and(|(s, e)| {
                let midpoint = s + (e - s) / 2;
                midpoint >= chunk.start_ms && midpoint < chunk.end_ms
            })
        })
        .collect()
}
fn stitch_units(mut units: Vec<(usize, TimedUnit)>) -> Vec<TimedUnit> {
    units.sort_by_key(|(_, unit)| unit.start_ms);
    let mut output: Vec<(usize, TimedUnit)> = vec![];
    for (chunk, unit) in units {
        if let Some((previous_chunk, previous)) = output.last() {
            // Only cross-chunk observations of the same overlapping span are
            // duplicates. Repetition inside one decoder output is review evidence.
            if *previous_chunk != chunk
                && previous.end_ms > unit.start_ms
                && previous.text.trim() == unit.text.trim()
            {
                continue;
            }
        }
        output.push((chunk, unit));
    }
    output.into_iter().map(|(_, unit)| unit).collect()
}

pub async fn execute_chunks(
    job: &Job,
    ffmpeg: &Path,
    provider: Arc<dyn AsrProvider>,
    options: &TranscriptionOptions,
    run_id: &str,
    database: Option<&LocalDatabase>,
) -> Result<AsrResponse> {
    let root = job.dir.join("asr-checkpoint");
    fs::create_dir_all(&root)?;
    let fingerprint_options = options.clone();
    let provider_id = provider.status().id.clone();
    let config = tokio::task::spawn_blocking(move || -> Result<serde_json::Value> {
        let mut config = serde_json::to_value(&fingerprint_options)?;
        for key in ["model", "vad_model", "glossary"] {
            if let Some(path) = config.get(key).and_then(|v| v.as_str()) {
                config[key] = serde_json::Value::String(file_digest(Path::new(path))?);
            }
        }
        config["provider"] = serde_json::Value::String(provider_id);
        Ok(config)
    })
    .await??;
    let identity: AudioIdentity =
        serde_json::from_slice(&fs::read(job.dir.join("audio-identity.json"))?)?;
    let desired = AcousticPlan::new(identity.duration_ms, identity.sha256, json_digest(&config)?);
    let plan_path = root.join("plan.json");
    let plan = if plan_path.is_file() {
        let p: AcousticPlan = serde_json::from_slice(&fs::read(&plan_path)?)?;
        if p.version != 1
            || p.strategy != desired.strategy
            || p.configuration_sha256 != desired.configuration_sha256
            || p.audio_sha256 != desired.audio_sha256
            || p.chunks != desired.chunks
        {
            bail!("识别资源或配置已变化，不能复用检查点；请创建独立重跑候选")
        }
        p
    } else {
        write_json(&plan_path, &desired)?;
        desired
    };
    let mut effective = options.clone();
    for chunk in &plan.chunks {
        let evidence = if let Ok(evidence) = read_evidence(&root, chunk) {
            evidence
        } else {
            let attempt = root.join(format!("attempt-{}-{}", chunk.index, uuid::Uuid::new_v4()));
            fs::create_dir(&attempt)?;
            let wav = attempt.join("input.wav");
            media::extract_wav_range(
                ffmpeg,
                &job.audio_wav,
                &wav,
                chunk.read_start_ms,
                chunk.read_end_ms,
            )
            .await?;
            let prefix = attempt.join("provider-output");
            let recognition_started = std::time::Instant::now();
            let response = provider
                .transcribe(AsrRequest {
                    audio_path: wav.clone(),
                    output_prefix: prefix.clone(),
                    scope: AsrInputScope::selected_range(chunk.read_start_ms, chunk.read_end_ms)?,
                    cloud_audio_upload_authorized: false,
                    transcription: effective.clone(),
                })
                .await?;
            let device = fs::read_to_string(prefix.with_extension("device")).unwrap_or_else(|_| {
                if effective.whisper.no_gpu {
                    "cpu".into()
                } else {
                    "provider-managed".into()
                }
            });
            let mut units = response.timed_units;
            for unit in &mut units {
                let (Some(s), Some(e)) = (unit.start_ms, unit.end_ms) else {
                    bail!("ASR returned missing timestamps")
                };
                if e <= s || s >= chunk.read_end_ms - chunk.read_start_ms {
                    bail!("ASR returned invalid chunk timestamps")
                };
                if e > chunk.read_end_ms - chunk.read_start_ms {
                    unit.timing_source = crate::application::TimingSource::Unknown;
                }
                unit.start_ms = Some(chunk.read_start_ms + s);
                unit.end_ms = Some((chunk.read_start_ms + e).min(chunk.read_end_ms));
            }
            let raw = attempt.join("provider-output.json");
            if response.raw_output_path != raw {
                fs::copy(&response.raw_output_path, &raw)?;
            }
            let evidence = ChunkEvidence {
                chunk: chunk.clone(),
                units,
                device,
                raw_sha256: file_digest(&raw)?,
                recognition_elapsed_ms: recognition_started
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            };
            write_json(&attempt.join("evidence.json"), &evidence)?;
            atomic_write(
                &attempt.join("sha256"),
                format!(
                    "{:x}",
                    Sha256::digest(fs::read(attempt.join("evidence.json"))?)
                )
                .as_bytes(),
            )?;
            fs::File::open(&raw)?.sync_all()?;
            let _ = fs::remove_file(wav);
            let destination = evidence_path(&root, chunk.index);
            if destination.exists() {
                fs::rename(
                    &destination,
                    root.join(format!("corrupt-{}-{}", chunk.index, uuid::Uuid::new_v4())),
                )?;
            }
            fs::rename(&attempt, &destination)?;
            sync_directory(&root)?;
            read_evidence(&root, chunk)?
        };
        if evidence.device == "cpu" {
            effective.whisper.no_gpu = true;
        }
        if let Ok(artifacts) =
            crate::infrastructure::asr_run_store::AsrRunArtifacts::open(job, run_id)
        {
            let confirmed = plan
                .chunks
                .iter()
                .take(chunk.index + 1)
                .map(|c| (c.start_ms, c.end_ms))
                .collect::<Vec<_>>();
            write_json(
                &artifacts.dir.join("checkpoints.json"),
                &serde_json::json!({"duration_ms":plan.duration_ms,"completed_ms":confirmed.iter().map(|(s,e)|e-s).sum::<u64>(),"completed_ranges":confirmed}),
            )?;
        }

        if let Some(db) = database {
            db.record_acoustic_checkpoint(
                &job.id(),
                run_id,
                chunk,
                &evidence_path(&root, chunk.index),
                &evidence.device,
            )
            .await?;
        }
    }
    let timed_units = partial_units(job)?;
    let raw = root.join("merged-units.json");
    write_json(&raw, &timed_units)?;
    Ok(AsrResponse {
        provider_id: provider.status().id,
        model_identity: options.whisper.model.display().to_string(),
        raw_output_path: raw,
        timed_units,
        legacy_segments: vec![],
    })
}

pub fn candidate_cues(units: &[TimedUnit]) -> crate::application::CandidateCueSet {
    // A provider segment has only segment timing. Preserve it, rather than
    // inventing word boundaries by proportional character splitting.
    crate::application::CandidateCueSet {
        schema_version: 1,
        segmentation_policy: ACOUSTIC_STRATEGY.into(),
        cues: units
            .iter()
            .map(|u| crate::application::CandidateCue {
                cue: crate::domain::TranscriptSegment::new(
                    u.start_ms.unwrap(),
                    u.end_ms.unwrap(),
                    u.text.clone(),
                ),
                timed_unit_ids: vec![u.id.clone()],
            })
            .collect(),
    }
}

pub fn boundary_signals(
    job_id: &str,
    run_id: &str,
    units: &[TimedUnit],
    created_at_unix: u64,
) -> Vec<crate::application::QualitySignal> {
    let mut signals = vec![];
    for (index, unit) in units.iter().enumerate() {
        let overlaps = index > 0 && units[index - 1].end_ms > unit.start_ms;
        if unit.timing_source == crate::application::TimingSource::Unknown || overlaps {
            signals.push(crate::application::QualitySignal {schema_version:1,id:uuid::Uuid::new_v4().to_string(),job_id:job_id.into(),run_id:run_id.into(),kind:crate::application::QualitySignalKind::ManualConcern,severity:crate::application::QualitySignalSeverity::Warning,start_ms:unit.start_ms,end_ms:unit.end_ms,cue_ids:vec![],detector_version:ACOUSTIC_STRATEGY.into(),message:if overlaps {"相邻识别证据时间重叠，未静默删词；请复核接缝".into()}else{"模型时间超过读取音频，已限制到实际边界并标记精度未知；请复核片尾".into()},evidence:serde_json::json!({"timed_unit_id":unit.id,"boundary_overlap":overlaps,"timing_source":unit.timing_source}),created_at_unix});
        }
    }
    signals
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunk_plan_has_exact_ownership_and_bounded_context() {
        let plan = AcousticPlan::new(3601000, "a".into(), "c".into());
        assert_eq!(plan.chunks.len(), 31);
        assert_eq!(plan.chunks[1].start_ms, 120000);
        assert_eq!(plan.chunks[1].read_start_ms, 115000);
        assert_eq!(plan.chunks.last().unwrap().end_ms, 3601000);
        for pair in plan.chunks.windows(2) {
            assert_eq!(pair[0].end_ms, pair[1].start_ms)
        }
    }
    #[test]
    fn atomic_artifact_has_no_partial_publication() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let p = root.join("state.json");
        atomic_write(&p, b"old").unwrap();
        atomic_write(&p, b"new").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"new");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn interrupted_hour_resumes_only_uncommitted_chunks_and_repairs_index() {
        use crate::application::{
            AsrDataLocality, AsrFuture, AsrProviderCapabilities, AsrProviderStatus,
            AsrTimingGranularity, TimedUnitKind, TimingSource,
        };
        use std::{
            os::unix::fs::PermissionsExt,
            sync::atomic::{AtomicUsize, Ordering},
        };
        #[derive(Debug)]
        struct Fake {
            calls: AtomicUsize,
            fail_at: AtomicUsize,
        }
        impl AsrProvider for Fake {
            fn status(&self) -> AsrProviderStatus {
                AsrProviderStatus {
                    id: "fake".into(),
                    name: "Fake".into(),
                    capabilities: AsrProviderCapabilities {
                        timing_granularities: vec![AsrTimingGranularity::ProviderSegment],
                        vocabulary_biasing: false,
                        diarization: false,
                        streaming: false,
                        data_locality: AsrDataLocality::Local,
                    },
                }
            }
            fn transcribe<'a>(&'a self, r: AsrRequest) -> AsrFuture<'a> {
                Box::pin(async move {
                    let n = self.calls.fetch_add(1, Ordering::SeqCst);
                    if n == self.fail_at.load(Ordering::SeqCst) {
                        bail!("injected decode failure")
                    }
                    let raw = r.output_prefix.with_extension("json");
                    fs::write(&raw, b"{}")?;
                    let local = if r.scope.start_ms == 0 { 1000 } else { 6000 };
                    Ok(AsrResponse {
                        provider_id: "fake".into(),
                        model_identity: "fake".into(),
                        raw_output_path: raw,
                        legacy_segments: vec![],
                        timed_units: vec![TimedUnit {
                            id: uuid::Uuid::new_v4().to_string(),
                            text: format!("chunk {}", r.scope.start_ms),
                            kind: TimedUnitKind::ProviderSegment,
                            start_ms: Some(local),
                            end_ms: Some(local + 1000),
                            timing_source: TimingSource::Model,
                            provider_confidence: None,
                            speaker: None,
                        }],
                    })
                })
            }
        }
        let root = std::env::temp_dir().join(format!("atogaki-resume-{}", uuid::Uuid::new_v4()));
        let job = Job::create_in(&root).unwrap();
        let ffmpeg = root.join("ffmpeg");
        fs::write(
            &ffmpeg,
            "#!/bin/sh\nfor destination do :; done\n: > \"$destination\"\n".replace("\\\"", "\""),
        )
        .unwrap();
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();
        let model = root.join("model.bin");
        fs::write(&model, b"model").unwrap();
        let options = TranscriptionOptions::japanese(model.clone());
        write_json(
            &job.dir.join("audio-identity.json"),
            &AudioIdentity {
                sha256: "audio".into(),
                duration_ms: 3600000,
            },
        )
        .unwrap();
        fs::write(&job.audio_wav, b"fixture").unwrap();
        let fake = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            fail_at: AtomicUsize::new(17),
        });
        assert!(
            execute_chunks(&job, &ffmpeg, fake.clone(), &options, "first", None)
                .await
                .is_err()
        );
        assert_eq!(recovery_summary(&job).unwrap().completed_ms, 17 * 120000);
        fake.fail_at.store(usize::MAX, Ordering::SeqCst);
        let response = execute_chunks(&job, &ffmpeg, fake.clone(), &options, "second", None)
            .await
            .unwrap();
        assert_eq!(fake.calls.load(Ordering::SeqCst), 31); // 30 successful chunks + failed call
        assert_eq!(response.timed_units.len(), 30);
        assert_eq!(recovery_summary(&job).unwrap().completed_ms, 3600000);
        let db = LocalDatabase::open(root.join("index.sqlite"))
            .await
            .unwrap();
        let mut manifest = crate::application::job_manifest::JobManifest::new(
            &job,
            None,
            None,
            crate::domain::LanguagePair::default(),
        );
        manifest.mark(crate::application::job_status::JobStatus::Done);
        job.write_manifest(&manifest).unwrap();
        db.sync_snapshot(&crate::application::JobSnapshot::load(&job.dir).unwrap())
            .await
            .unwrap();
        assert!(
            db.acoustic_checkpoint_ranges(&job.id())
                .await
                .unwrap()
                .is_empty()
        );
        execute_chunks(
            &job,
            &ffmpeg,
            fake.clone(),
            &options,
            "index-repair",
            Some(&db),
        )
        .await
        .unwrap();
        assert_eq!(
            db.acoustic_checkpoint_ranges(&job.id())
                .await
                .unwrap()
                .len(),
            30
        );
        assert_eq!(fake.calls.load(Ordering::SeqCst), 31);
        db.close().await;
        // A damaged raw artifact invalidates only that chunk.
        fs::write(
            job.dir
                .join("asr-checkpoint/chunk-000003/provider-output.json"),
            b"damaged",
        )
        .unwrap();
        execute_chunks(&job, &ffmpeg, fake.clone(), &options, "third", None)
            .await
            .unwrap();
        assert_eq!(fake.calls.load(Ordering::SeqCst), 32);
        // Moving identical weights preserves identity; changing weights cannot reuse evidence.
        let moved = root.join("moved.bin");
        fs::rename(&model, &moved).unwrap();
        let moved_options = TranscriptionOptions::japanese(moved.clone());
        execute_chunks(&job, &ffmpeg, fake.clone(), &moved_options, "fourth", None)
            .await
            .unwrap();
        assert_eq!(fake.calls.load(Ordering::SeqCst), 32);
        fs::write(&moved, b"different weights").unwrap();
        assert!(
            execute_chunks(&job, &ffmpeg, fake.clone(), &moved_options, "bad", None)
                .await
                .is_err()
        );
        assert_eq!(fake.calls.load(Ordering::SeqCst), 32);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn overlapping_repetition_inside_one_chunk_is_not_silently_deleted() {
        let unit = |id: &str, start, end| TimedUnit {
            id: id.into(),
            text: "はい".into(),
            kind: crate::application::TimedUnitKind::ProviderSegment,
            start_ms: Some(start),
            end_ms: Some(end),
            timing_source: crate::application::TimingSource::Model,
            provider_confidence: None,
            speaker: None,
        };
        assert_eq!(
            stitch_units(vec![(0, unit("a", 0, 1000)), (0, unit("b", 500, 1500))]).len(),
            2
        );
        assert_eq!(
            stitch_units(vec![
                (0, unit("a", 119000, 120500)),
                (1, unit("b", 119500, 121000))
            ])
            .len(),
            1
        );
    }
}
