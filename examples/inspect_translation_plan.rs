//! Save the actual planner's input groups without calling a translation provider.
//! cargo run --example inspect_translation_plan -- segments.json private-plan.json
use anyhow::{Context, Result, ensure};
use atogaki_subtitle::{
    application::{TranslationPlanner, TranslationPlannerCue},
    domain::TranscriptSegment,
};
use std::{collections::HashSet, fs::OpenOptions, io::Write, path::PathBuf};

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(
        args.len() == 2,
        "expected segments.json and NEW output path"
    );
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let segments: Vec<TranscriptSegment> = serde_json::from_slice(&std::fs::read(input)?)?;
    let timeline = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            Ok(TranslationPlannerCue {
                segment_id: segment.id.clone(),
                segment_index: i64::try_from(index)?,
                start_ms: i64::try_from(segment.start_ms)?,
                end_ms: i64::try_from(segment.end_ms)?,
                source_text: segment.source_text.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let ids = timeline
        .iter()
        .map(|cue| cue.segment_id.clone())
        .collect::<HashSet<_>>();
    ensure!(ids.len() == timeline.len(), "duplicate cue IDs");
    let plan = TranslationPlanner::default().plan(&timeline, &ids)?;
    let bytes = serde_json::to_vec_pretty(&plan)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .context("create a new private plan file (existing files are preserved)")?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "{} cues, {} semantic groups; no provider called",
        segments.len(),
        plan.groups.len()
    );
    Ok(())
}
