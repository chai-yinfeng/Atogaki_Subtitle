use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::application::CandidateCueSet;

pub const ASR_QUALITY_DETECTOR_VERSION: &str = "quality-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualitySignalKind {
    RepeatedLoop,
    BoundaryRepetition,
    AbnormalSpeechRate,
    SpeechDuringSilence,
    ManualConcern,
}

impl QualitySignalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RepeatedLoop => "repeated_loop",
            Self::BoundaryRepetition => "boundary_repetition",
            Self::AbnormalSpeechRate => "abnormal_speech_rate",
            Self::SpeechDuringSilence => "speech_during_silence",
            Self::ManualConcern => "manual_concern",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualitySignalSeverity {
    Info,
    Warning,
    Critical,
}

impl QualitySignalSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualitySignal {
    pub schema_version: u32,
    pub id: String,
    pub job_id: String,
    pub run_id: String,
    pub kind: QualitySignalKind,
    pub severity: QualitySignalSeverity,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub cue_ids: Vec<String>,
    pub detector_version: String,
    pub message: String,
    pub evidence: Value,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecisionKind {
    ConfirmedProblem,
    FalsePositive,
    Resolved,
}

impl ReviewDecisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConfirmedProblem => "confirmed_problem",
            Self::FalsePositive => "false_positive",
            Self::Resolved => "resolved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDecision {
    pub id: String,
    pub signal_id: String,
    pub decision: ReviewDecisionKind,
    pub note: Option<String>,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairAttemptStatus {
    Candidate,
    Adopted,
    Rejected,
}

impl RepairAttemptStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Adopted => "adopted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairAttempt {
    pub id: String,
    pub signal_id: String,
    pub candidate_run_id: String,
    pub status: RepairAttemptStatus,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

pub fn detect_quality_signals(
    job_id: &str,
    run_id: &str,
    cues: &CandidateCueSet,
    created_at_unix: u64,
) -> Vec<QualitySignal> {
    let mut signals = Vec::new();
    for window in cues.cues.windows(2) {
        let left = &window[0].cue;
        let right = &window[1].cue;
        let left_text = normalize(&left.source_text);
        let right_text = normalize(&right.source_text);
        if left_text.len() >= 4 && left_text == right_text {
            signals.push(signal(
                job_id,
                run_id,
                QualitySignalKind::RepeatedLoop,
                QualitySignalSeverity::Critical,
                Some(left.start_ms),
                Some(right.end_ms),
                vec![left.id.clone(), right.id.clone()],
                "相邻字幕包含完全相同的识别文本",
                json!({"normalized_text": left_text, "occurrences": 2}),
                created_at_unix,
            ));
        } else if let Some(overlap) = boundary_overlap(&left_text, &right_text) {
            signals.push(signal(
                job_id,
                run_id,
                QualitySignalKind::BoundaryRepetition,
                QualitySignalSeverity::Warning,
                Some(left.start_ms),
                Some(right.end_ms),
                vec![left.id.clone(), right.id.clone()],
                "相邻字幕边界包含较长的重复文本",
                json!({"overlap": overlap}),
                created_at_unix,
            ));
        }
    }
    for candidate in &cues.cues {
        let cue = &candidate.cue;
        let duration_seconds = cue.end_ms.saturating_sub(cue.start_ms) as f64 / 1_000.0;
        if duration_seconds <= 0.0 {
            continue;
        }
        let character_count = cue
            .source_text
            .chars()
            .filter(|character| !character.is_whitespace())
            .count();
        let characters_per_second = character_count as f64 / duration_seconds;
        if character_count >= 12 && characters_per_second > 20.0 {
            signals.push(signal(
                job_id,
                run_id,
                QualitySignalKind::AbnormalSpeechRate,
                QualitySignalSeverity::Warning,
                Some(cue.start_ms),
                Some(cue.end_ms),
                vec![cue.id.clone()],
                "字幕文字密度异常，可能包含时间戳塌缩或循环输出",
                json!({
                    "character_count": character_count,
                    "duration_ms": cue.end_ms.saturating_sub(cue.start_ms),
                    "characters_per_second": characters_per_second,
                }),
                created_at_unix,
            ));
        }
    }
    signals
}

#[allow(clippy::too_many_arguments)]
fn signal(
    job_id: &str,
    run_id: &str,
    kind: QualitySignalKind,
    severity: QualitySignalSeverity,
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    cue_ids: Vec<String>,
    message: &str,
    evidence: Value,
    created_at_unix: u64,
) -> QualitySignal {
    QualitySignal {
        schema_version: 1,
        id: Uuid::new_v4().to_string(),
        job_id: job_id.to_string(),
        run_id: run_id.to_string(),
        kind,
        severity,
        start_ms,
        end_ms,
        cue_ids,
        detector_version: ASR_QUALITY_DETECTOR_VERSION.to_string(),
        message: message.to_string(),
        evidence,
        created_at_unix,
    }
}

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace() && !character.is_ascii_punctuation())
        .collect::<String>()
        .to_lowercase()
}

fn boundary_overlap(left: &str, right: &str) -> Option<String> {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let maximum = left.len().min(right.len()).min(40);
    (4..=maximum).rev().find_map(|length| {
        (left[left.len() - length..] == right[..length])
            .then(|| left[left.len() - length..].iter().collect())
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{CandidateCue, CandidateCueSet, QualitySignalKind},
        domain::TranscriptSegment,
    };

    use super::detect_quality_signals;

    fn cue(id: &str, start_ms: u64, end_ms: u64, text: &str) -> CandidateCue {
        let mut cue = TranscriptSegment::new(start_ms, end_ms, text.into());
        cue.id = id.into();
        CandidateCue {
            cue,
            timed_unit_ids: Vec::new(),
        }
    }

    #[test]
    fn detects_loop_boundary_overlap_and_implausible_text_density() {
        let cues = CandidateCueSet {
            schema_version: 1,
            segmentation_policy: "test".into(),
            cues: vec![
                cue("a", 0, 1_000, "これは繰り返しです"),
                cue("b", 1_000, 2_000, "これは繰り返しです"),
                cue("c", 2_000, 3_000, "繰り返しです次の話"),
                cue("d", 3_000, 3_100, "一二三四五六七八九十十一十二"),
            ],
        };
        let signals = detect_quality_signals("job", "run", &cues, 1);
        assert!(
            signals
                .iter()
                .any(|signal| signal.kind == QualitySignalKind::RepeatedLoop)
        );
        assert!(
            signals
                .iter()
                .any(|signal| signal.kind == QualitySignalKind::BoundaryRepetition)
        );
        assert!(
            signals
                .iter()
                .any(|signal| signal.kind == QualitySignalKind::AbnormalSpeechRate)
        );
    }
}
