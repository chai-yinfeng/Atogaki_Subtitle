//! Reconcile overlapping decoder observations before assigning subtitle output.
//! Model timestamps and unit IDs are retained; uncertain seams retain evidence.
use super::{TimedUnit, acoustic_plan::AcousticChunk};
use serde::{Deserialize, Serialize};

pub const STITCHING_STRATEGY: &str = "overlap-anchor-v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeamDecision {
    pub boundary_ms: u64,
    pub left_chunk: usize,
    pub right_chunk: usize,
    pub mode: String,
    pub left_anchor_id: Option<String>,
    pub right_anchor_id: Option<String>,
    pub preserved_unit_ids: Vec<String>,
    pub review_start_ms: u64,
    pub review_end_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StitchReport {
    pub strategy: String,
    pub seams: Vec<SeamDecision>,
}

fn normalized(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn span(unit: &TimedUnit) -> (u64, u64) {
    (unit.start_ms.unwrap(), unit.end_ms.unwrap())
}

// Match at existing unit starts, accumulating short word/token units. A long
// segment may contain several shorter units; no artificial timing is introduced.
fn anchor_text(units: &[TimedUnit], start: usize) -> String {
    let mut text = String::new();
    let mut previous_end = span(&units[start]).0;
    for unit in units.iter().skip(start).take(12) {
        if span(unit).0.saturating_sub(previous_end) > 1500 {
            break;
        }
        text.push_str(&normalized(&unit.text));
        previous_end = span(unit).1;
        if text.chars().count() >= 12 {
            break;
        }
    }
    text
}

fn matching_anchor(
    left: &[TimedUnit],
    right: &[TimedUnit],
    chunk: &AcousticChunk,
    left_read_end: u64,
) -> Option<(usize, usize)> {
    let candidates = |units: &[TimedUnit]| {
        units
            .iter()
            .enumerate()
            .filter(|(_, unit)| {
                let (start, end) = span(unit);
                end > chunk.read_start_ms && start <= chunk.start_ms
            })
            .map(|(i, _)| (i, anchor_text(units, i)))
            .filter(|(_, text)| text.chars().count() >= 12)
            .collect::<Vec<_>>()
    };
    let left_candidates = candidates(left);
    let right_candidates = candidates(right);
    let mut matches = Vec::new();
    for (i, a) in &left_candidates {
        for (j, b) in &right_candidates {
            let (ls, le) = span(&left[*i]);
            let (rs, re) = span(&right[*j]);
            if ls < re && rs < le && (a.starts_with(b) || b.starts_with(a)) {
                matches.push((*i, *j));
            }
        }
    }
    matches
        .iter()
        .copied()
        // Repeated phrases at a seam are not a reliable alignment anchor.
        .filter(|(i, j)| {
            matches.iter().filter(|(a, _)| a == i).count() == 1
                && matches.iter().filter(|(_, b)| b == j).count() == 1
        })
        .filter(|(i, j)| {
            // A shared prefix must not hide additional owned speech on either
            // side. Context-only decoder suffixes may differ at the read edge.
            let left_owned_tail = left[*i..]
                .iter()
                .enumerate()
                .filter(|(index, u)| *index == 0 || span(u).0 < chunk.start_ms)
                .map(|(_, u)| normalized(&u.text))
                .collect::<String>();
            let right_replacement = right[*j..]
                .iter()
                .take_while(|u| span(u).0 < left_read_end)
                .map(|u| normalized(&u.text))
                .collect::<String>();
            let discarded_right_owned = right[..*j]
                .iter()
                .filter(|u| span(u).1 > chunk.start_ms)
                .map(|u| normalized(&u.text))
                .collect::<String>();
            let retained_left = left[..*i]
                .iter()
                .map(|u| normalized(&u.text))
                .collect::<String>();
            right_replacement.starts_with(&left_owned_tail)
                && retained_left.ends_with(&discarded_right_owned)
        })
        .min_by_key(|(i, j)| {
            chunk.start_ms.abs_diff(span(&left[*i]).0) + chunk.start_ms.abs_diff(span(&right[*j]).0)
        })
}

pub fn stitch(chunks: &[(AcousticChunk, Vec<TimedUnit>)]) -> (Vec<TimedUnit>, StitchReport) {
    let mut output = Vec::new();
    let mut seams = Vec::new();
    for (index, (chunk, units)) in chunks.iter().enumerate() {
        let mut sorted = units.clone();
        sorted.sort_by_key(|unit| unit.start_ms);
        let units = &sorted;
        if index == 0 {
            output.extend(units.clone());
            continue;
        }
        let previous = &chunks[index - 1].0;
        let anchor = if previous.end_ms == chunk.start_ms {
            matching_anchor(&output, units, chunk, previous.read_end_ms)
        } else {
            None
        };
        let has_context = output.iter().any(|u| span(u).1 > chunk.start_ms)
            || units.iter().any(|u| span(u).0 < chunk.start_ms);
        if let Some((left, right)) = anchor {
            seams.push(SeamDecision {
                boundary_ms: chunk.start_ms,
                left_chunk: previous.index,
                right_chunk: chunk.index,
                mode: "aligned_unit_start".into(),
                left_anchor_id: Some(output[left].id.clone()),
                right_anchor_id: Some(units[right].id.clone()),
                preserved_unit_ids: vec![],
                review_start_ms: span(&output[left]).0.min(span(&units[right]).0),
                review_end_ms: span(&output[left]).1.max(span(&units[right]).1),
            });
            output.truncate(left);
            output.extend_from_slice(&units[right..]);
        } else {
            if has_context {
                seams.push(SeamDecision {
                    boundary_ms: chunk.start_ms,
                    left_chunk: previous.index,
                    right_chunk: chunk.index,
                    mode: "unresolved_preserved".into(),
                    left_anchor_id: None,
                    right_anchor_id: None,
                    preserved_unit_ids: output
                        .iter()
                        .filter(|u| span(u).1 > chunk.read_start_ms)
                        .chain(units.iter().filter(|u| span(u).0 < previous.read_end_ms))
                        .map(|u| u.id.clone())
                        .collect(),
                    review_start_ms: chunk.read_start_ms,
                    review_end_ms: previous.read_end_ms,
                });
            }
            // Keep both observations when alignment is uncertain, even if that
            // leaves duplicate/overlapping captions for explicit human review.
            output.extend_from_slice(units);
        }
        output.sort_by_key(|unit| unit.start_ms);
    }
    output.sort_by_key(|unit| unit.start_ms);
    (
        output,
        StitchReport {
            strategy: STITCHING_STRATEGY.into(),
            seams,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{TimedUnitKind, TimingSource};

    fn unit(id: &str, start: u64, end: u64, text: &str) -> TimedUnit {
        TimedUnit {
            id: id.into(),
            text: text.into(),
            start_ms: Some(start),
            end_ms: Some(end),
            kind: TimedUnitKind::ProviderSegment,
            timing_source: TimingSource::Model,
            provider_confidence: None,
            speaker: None,
        }
    }
    fn chunks(left: Vec<TimedUnit>, right: Vec<TimedUnit>) -> Vec<(AcousticChunk, Vec<TimedUnit>)> {
        let plan = super::super::acoustic_plan::AcousticPlan::new(300000, "a".into(), "b".into());
        vec![
            (plan.chunks[1].clone(), left),
            (plan.chunks[2].clone(), right),
        ]
    }

    #[test]
    fn drifting_midpoints_cannot_drop_both_observations() {
        let left = unit(
            "left",
            237500,
            243750,
            "You found paradise in America, had a good trade,",
        );
        let right = unit(
            "right",
            235000,
            244530,
            "You found paradise in America, had a good trade, made a good living.",
        );
        let (output, report) = stitch(&chunks(vec![left], vec![right.clone()]));
        assert_eq!(output, vec![right]);
        assert_eq!(report.seams[0].mode, "aligned_unit_start");
    }

    #[test]
    fn segmentation_changes_align_at_a_real_unit_start() {
        let left = unit("a", 237500, 242790, "You found paradise in America,");
        let right = vec![
            unit("b", 235000, 238970, "You found paradise in America,"),
            unit("c", 238970, 244150, "had a good trade, made a good"),
            unit("d", 244150, 244530, "living."),
        ];
        let (output, _) = stitch(&chunks(vec![left], right.clone()));
        assert_eq!(output, right);
    }

    #[test]
    fn conflicting_owned_speech_and_short_repetitions_are_kept_for_review() {
        for (a, b) in [
            (
                "This is the same introduction, unique left ending",
                "This is the same introduction, unique right ending",
            ),
            ("はい", "はい"),
        ] {
            let left = unit("left", 237000, 241000, a);
            let right = unit("right", 238000, 245000, b);
            let (output, report) = stitch(&chunks(vec![left.clone()], vec![right.clone()]));
            assert_eq!(output, vec![left, right]);
            assert_eq!(report.seams[0].mode, "unresolved_preserved");
        }
    }

    #[test]
    fn short_word_units_can_align_as_a_phrase_without_changing_times() {
        let words = |prefix: &str, offset| {
            ["You", "found", "paradise", "in", "America"]
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    unit(
                        &format!("{prefix}{i}"),
                        offset + i as u64 * 500,
                        offset + (i as u64 + 1) * 500,
                        text,
                    )
                })
                .collect::<Vec<_>>()
        };
        let right = words("r", 238250);
        let (output, report) = stitch(&chunks(words("l", 238000), right.clone()));
        assert_eq!(report.seams[0].mode, "aligned_unit_start");
        assert_eq!(
            output
                .iter()
                .map(|u| normalized(&u.text))
                .collect::<String>(),
            "youfoundparadiseinamerica"
        );
        let all = [words("l", 238000), right].concat();
        assert!(output.iter().all(|u| all.contains(u)));
    }

    #[test]
    fn one_chunk_and_silent_seams_preserve_evidence_without_review() {
        let one = chunks(vec![unit("a", 130000, 131000, "same chunk")], vec![]);
        let (output, report) = stitch(&one[..1]);
        assert_eq!(output, one[0].1);
        assert!(report.seams.is_empty());
        let (_, report) = stitch(&one);
        assert!(report.seams.is_empty());
        let separated = chunks(
            vec![unit("left", 235000, 237000, "Before the silence.")],
            vec![unit("right", 241000, 242000, "After the silence.")],
        );
        let (output, report) = stitch(&separated);
        assert_eq!(output.len(), 2);
        assert!(
            report.seams.is_empty(),
            "ordinary owned speech after silence is not ambiguous context"
        );
    }

    #[test]
    fn repeated_long_anchors_are_ambiguous_and_not_collapsed() {
        let text = "Thank you very much for everything.";
        let input = chunks(
            vec![
                unit("l1", 237000, 240000, text),
                unit("l2", 239000, 242000, text),
            ],
            vec![
                unit("r1", 236000, 240500, text),
                unit("r2", 238000, 242500, text),
            ],
        );
        let (output, report) = stitch(&input);
        assert_eq!(output.len(), 4);
        assert_eq!(report.seams[0].mode, "unresolved_preserved");
        assert_eq!(report.seams[0].preserved_unit_ids.len(), 4);
    }

    #[test]
    fn common_anchor_does_not_erase_additional_owned_speech() {
        let input = chunks(
            vec![
                unit("l1", 236000, 237000, "This phrase is shared."),
                unit("l2", 237000, 239000, "Important unique words."),
            ],
            vec![
                unit("r1", 236500, 238000, "This phrase is shared."),
                unit("r2", 240000, 242000, "Different continuation."),
            ],
        );
        let (output, report) = stitch(&input);
        assert!(output.iter().any(|u| u.id == "l2"));
        assert_eq!(output.len(), 4);
        assert_eq!(report.seams[0].mode, "unresolved_preserved");
    }

    #[test]
    fn overlapping_repetition_inside_one_chunk_remains_evidence() {
        let input = chunks(
            vec![
                unit("a", 237000, 241000, "はい"),
                unit("b", 239000, 243000, "はい"),
            ],
            vec![],
        );
        let (output, _) = stitch(&input[..1]);
        assert_eq!(output, input[0].1);
    }
}
