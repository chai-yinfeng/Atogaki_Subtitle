//! Create a persistent, unique temporary QA data directory. No existing data is opened.
//! Optional argument: trusted local ffmpeg executable to generate synthetic video.
use anyhow::{Result, ensure};
use atogaki_subtitle::{
    application::{
        AsrInputScope, AsrRun, CandidateCue, CandidateCueSet, TimedUnit, TimedUnitKind,
        TimingSource, job_manifest::JobManifest, job_snapshot::JobSnapshot, job_status::JobStatus,
    },
    domain::TranscriptSegment,
    infrastructure::{
        asr_run_store::AsrRunArtifacts,
        job_store::Job,
        local_db::{LocalDatabase, LocalMachineTranslation, LocalTranslationDependency},
    },
};
use std::{fs, path::Path, process::Command};

#[tokio::main]
async fn main() -> Result<()> {
    let root = std::env::temp_dir().join(format!("atogaki-token-qa-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root)?;
    eprintln!("synthetic fixture: {}", root.display());
    let job = Job::create_in(&root.join("jobs"))?;
    // Thirty seconds of normalized PCM16 mono silence; no microphone or user media.
    let samples = 16_000_u32 * 30;
    let mut wav = Vec::with_capacity(44 + samples as usize * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + samples * 2).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&16_000_u32.to_le_bytes());
    wav.extend_from_slice(&32_000_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(samples * 2).to_le_bytes());
    wav.resize(44 + samples as usize * 2, 0);
    fs::write(&job.audio_wav, wav)?;
    let mut input = job.audio_wav.clone();
    if let Some(ffmpeg) = std::env::args_os().nth(1) {
        input = root.join("synthetic.mp4");
        let status = Command::new(ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x243847:s=640x360:r=25:d=30",
                "-i",
            ])
            .arg(&job.audio_wav)
            .args([
                "-c:v",
                "h264_videotoolbox",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(&input)
            .status()?;
        ensure!(status.success(), "synthetic video generation failed");
    }
    let segments = (0..10)
        .map(|i| {
            let start = if i < 2 { i * 1000 } else { i * 3000 };
            let mut cue = TranscriptSegment::new(
                start,
                start + 1000,
                format!("原生撤销验收第 {} 句：来源与上下文。", i + 1),
            );
            cue.set_translation(Some(format!("Synthetic translation {}.", i + 1)));
            cue
        })
        .collect::<Vec<_>>();
    let mut manifest = JobManifest::new(&job, Some(input), None, Default::default());
    manifest.mark(JobStatus::Done);
    job.write_segments(&segments)?;
    job.write_manifest(&manifest)?;
    let database_path = root.join("atogaki.sqlite");
    let db = LocalDatabase::open(&database_path).await?;
    db.sync_snapshot(&JobSnapshot {
        manifest,
        segments: segments.clone(),
    })
    .await?;
    db.rename_job(
        &job.id(),
        Some("Token 原生验收 · 合成来源与三条依赖".into()),
    )
    .await?;
    let original = evidence(&db, &job, &segments, "synthetic-original", false).await?;
    let before = db.list_segments(&job.id()).await?;
    // Use the actual adoption transaction to establish provenance and protection ledger.
    let original_cues = AsrRunArtifacts::open(&job, &original.id)?.read_candidate_cues()?;
    db.replace_segment_range(
        &job.id(),
        0,
        28000,
        &before,
        &segments,
        Some((&original.id, &original_cues)),
    )
    .await?;
    // Adoption deliberately clears translations, so populate the actual current
    // rows through the machine-translation API before attaching frozen context.
    let adopted = db.list_segments(&job.id()).await?;
    db.apply_machine_translations(
        &job.id(),
        &adopted
            .iter()
            .enumerate()
            .map(|(i, cue)| LocalMachineTranslation {
                segment_id: cue.id.clone(),
                source_text: cue.source_text.clone(),
                expected_translated_text: cue.translated_text.clone(),
                expected_translation_edited: false,
                expected_source_revision: None,
                expected_translation_revision: None,
                translated_text: format!("Synthetic translation {}.", i + 1),
            })
            .collect::<Vec<_>>(),
    )
    .await?;
    let translated = db.list_segments(&job.id()).await?;
    for (target, contexts) in [(2, vec![0, 1]), (0, vec![1])] {
        let cue = &translated[target];
        db.apply_machine_translation_group(
            &job.id(),
            &[LocalMachineTranslation {
                segment_id: cue.id.clone(),
                source_text: cue.source_text.clone(),
                expected_translated_text: cue.translated_text.clone(),
                expected_translation_edited: false,
                expected_source_revision: None,
                expected_translation_revision: None,
                translated_text: cue.translated_text.clone().unwrap(),
            }],
            &contexts
                .into_iter()
                .map(|index| LocalTranslationDependency {
                    segment_id: segments[index].id.clone(),
                    source_text: segments[index].source_text.clone(),
                    expected_source_revision: 0,
                })
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    let replacement = TranscriptSegment::new(0, 1000, "合成候选：核对采用与撤销。".into());
    let candidate = evidence(&db, &job, &[replacement], "synthetic-candidate", true).await?;
    let (revision, _) = db.subtitle_workspace_snapshot(&job.id()).await?;
    let provenance = db.list_subtitle_provenance(&job.id()).await?;
    let report = serde_json::json!({"data_dir":root,"database":database_path,"job_id":job.id(),
        "original_run_id":original.id,"candidate_run_id":candidate.id,"workspace_revision":revision,
        "segments":db.list_segments(&job.id()).await?,"provenance":provenance});
    fs::write(root.join("seed.json"), serde_json::to_vec_pretty(&report)?)?;
    db.close().await;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
async fn evidence(
    db: &LocalDatabase,
    job: &Job,
    cues: &[TranscriptSegment],
    model: &str,
    candidate: bool,
) -> Result<AsrRun> {
    let mut run = AsrRun::new(
        job.id(),
        None,
        "fixture",
        "合成验收",
        model,
        job.audio_wav.clone(),
        AsrInputScope::full(),
        serde_json::json!({"synthetic_qa":true}),
    );
    run.succeed();
    let artifacts = AsrRunArtifacts::create(job, &run)?;
    let units = cues
        .iter()
        .enumerate()
        .map(|(i, cue)| TimedUnit {
            id: format!("{}-unit-{i}", run.id),
            text: cue.source_text.clone(),
            kind: TimedUnitKind::ProviderSegment,
            start_ms: Some(cue.start_ms),
            end_ms: Some(cue.end_ms),
            timing_source: TimingSource::Model,
            provider_confidence: None,
            speaker: None,
        })
        .collect::<Vec<_>>();
    artifacts.write_timed_units(&units)?;
    artifacts.write_candidate_cues(&CandidateCueSet {
        schema_version: 1,
        segmentation_policy: "synthetic-qa".into(),
        cues: cues
            .iter()
            .zip(&units)
            .map(|(cue, unit)| CandidateCue {
                cue: cue.clone(),
                timed_unit_ids: vec![unit.id.clone()],
            })
            .collect(),
    })?;
    artifacts.write_quality_signals(&[])?;
    if candidate {
        fs::write(artifacts.dir.join("candidate.json"), b"true")?;
    }
    fs::write(
        artifacts.provider_output_prefix.with_extension("json"),
        b"{\"synthetic_qa\":true}",
    )?;
    ensure!(
        Path::new(&artifacts.run_json).is_file(),
        "fixture run must be persisted"
    );
    db.record_asr_run(&run, &artifacts.dir).await?;
    Ok(run)
}
