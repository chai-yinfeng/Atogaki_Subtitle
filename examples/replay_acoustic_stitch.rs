//! Reconcile committed ASR checkpoints without decoding or changing the job.
//! cargo run --example replay_acoustic_stitch -- job-dir NEW-private-cues.json
use anyhow::{Context, Result, ensure};
use atogaki_subtitle::{application::acoustic_plan, infrastructure::job_store::Job};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(
        args.len() == 2,
        "expected job directory and NEW output path"
    );
    let job = Job::open(PathBuf::from(&args[0]))?;
    let cues = acoustic_plan::candidate_cues(&acoustic_plan::partial_units(&job)?);
    let bytes = serde_json::to_vec_pretty(&cues)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(PathBuf::from(&args[1]))
        .context("create a new private cue file (existing files are preserved)")?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "{} cues; no decoder called or job modified",
        cues.cues.len()
    );
    Ok(())
}
