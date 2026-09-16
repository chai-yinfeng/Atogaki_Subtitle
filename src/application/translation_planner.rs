use std::collections::HashSet;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::application::{TranslationContextSegment, TranslationTargetSegment};

pub const TRANSLATION_GROUPING_STRATEGY: &str = "semantic-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationPlannerCue {
    pub segment_id: String,
    pub segment_index: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub source_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationPlan {
    pub schema_version: u32,
    pub strategy_version: String,
    pub groups: Vec<TranslationGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationGroup {
    pub id: String,
    pub before_context: Vec<TranslationContextSegment>,
    pub targets: Vec<TranslationTargetSegment>,
    pub after_context: Vec<TranslationContextSegment>,
}

#[derive(Debug, Clone)]
pub struct TranslationPlanner {
    max_targets: usize,
    max_target_chars: usize,
    pause_boundary_ms: i64,
    context_window_ms: i64,
    max_context_chars: usize,
}

impl Default for TranslationPlanner {
    fn default() -> Self {
        Self {
            max_targets: 12,
            max_target_chars: 1_800,
            pause_boundary_ms: 1_500,
            context_window_ms: 30_000,
            max_context_chars: 2_000,
        }
    }
}

impl TranslationPlanner {
    pub fn plan(
        &self,
        timeline: &[TranslationPlannerCue],
        target_ids: &HashSet<String>,
    ) -> Result<TranslationPlan> {
        let known_ids = timeline
            .iter()
            .map(|cue| cue.segment_id.as_str())
            .collect::<HashSet<_>>();
        if let Some(missing) = target_ids
            .iter()
            .find(|id| !known_ids.contains(id.as_str()))
        {
            return Err(anyhow!(
                "translation target is not in the timeline: {missing}"
            ));
        }

        let mut grouped: Vec<Vec<&TranslationPlannerCue>> = Vec::new();
        let mut current: Vec<&TranslationPlannerCue> = Vec::new();
        let mut current_chars = 0usize;
        for cue in timeline
            .iter()
            .filter(|cue| target_ids.contains(&cue.segment_id))
        {
            let chars = cue.source_text.chars().count();
            let boundary = current.last().is_some_and(|previous| {
                cue.segment_index != previous.segment_index + 1
                    || cue.start_ms.saturating_sub(previous.end_ms) >= self.pause_boundary_ms
                    || ends_sentence(&previous.source_text)
                    || current.len() >= self.max_targets
                    || current_chars.saturating_add(chars) > self.max_target_chars
            });
            if boundary {
                grouped.push(std::mem::take(&mut current));
                current_chars = 0;
            }
            current_chars = current_chars.saturating_add(chars);
            current.push(cue);
        }
        if !current.is_empty() {
            grouped.push(current);
        }

        let groups = grouped
            .into_iter()
            .enumerate()
            .map(|(index, targets)| {
                let (before_context, after_context) = self.context(timeline, &targets);
                TranslationGroup {
                    id: format!("translation-group-{:04}", index + 1),
                    before_context,
                    targets: targets
                        .iter()
                        .map(|cue| TranslationTargetSegment {
                            segment_id: cue.segment_id.clone(),
                            source_text: cue.source_text.clone(),
                        })
                        .collect(),
                    after_context,
                }
            })
            .collect();
        Ok(TranslationPlan {
            schema_version: 1,
            strategy_version: TRANSLATION_GROUPING_STRATEGY.to_string(),
            groups,
        })
    }

    fn context(
        &self,
        timeline: &[TranslationPlannerCue],
        targets: &[&TranslationPlannerCue],
    ) -> (
        Vec<TranslationContextSegment>,
        Vec<TranslationContextSegment>,
    ) {
        let first = targets.first().expect("translation group cannot be empty");
        let last = targets.last().expect("translation group cannot be empty");
        let window_start = first.start_ms.saturating_sub(self.context_window_ms);
        let window_end = last.end_ms.saturating_add(self.context_window_ms);
        let target_ids = targets
            .iter()
            .map(|cue| cue.segment_id.as_str())
            .collect::<HashSet<_>>();
        let candidates = timeline
            .iter()
            .filter(|cue| cue.end_ms >= window_start && cue.start_ms <= window_end)
            .filter(|cue| !target_ids.contains(cue.segment_id.as_str()))
            .collect::<Vec<_>>();

        let mut before = Vec::new();
        let mut used = 0usize;
        for cue in candidates
            .iter()
            .copied()
            .filter(|cue| cue.segment_index < first.segment_index)
            .rev()
        {
            let available =
                (self.max_context_chars / 2).saturating_sub(used + usize::from(used > 0));
            if available == 0 {
                break;
            }
            let (source_text, truncated) = suffix(&cue.source_text, available);
            used += usize::from(used > 0) + source_text.chars().count();
            before.push(TranslationContextSegment {
                segment_id: cue.segment_id.clone(),
                source_text,
            });
            if truncated {
                break;
            }
        }
        before.reverse();

        let mut after = Vec::new();
        for cue in candidates
            .into_iter()
            .filter(|cue| cue.segment_index > last.segment_index)
        {
            let available = self
                .max_context_chars
                .saturating_sub(used + usize::from(used > 0));
            if available == 0 {
                break;
            }
            let (source_text, truncated) = prefix(&cue.source_text, available);
            used += usize::from(used > 0) + source_text.chars().count();
            after.push(TranslationContextSegment {
                segment_id: cue.segment_id.clone(),
                source_text,
            });
            if truncated {
                break;
            }
        }
        (before, after)
    }
}

fn prefix(text: &str, max_chars: usize) -> (String, bool) {
    let length = text.chars().count();
    (text.chars().take(max_chars).collect(), length > max_chars)
}

fn suffix(text: &str, max_chars: usize) -> (String, bool) {
    let length = text.chars().count();
    (
        text.chars()
            .skip(length.saturating_sub(max_chars))
            .collect(),
        length > max_chars,
    )
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|ch| matches!(ch, '。' | '！' | '？' | '.' | '!' | '?'))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{TranslationPlanner, TranslationPlannerCue};

    fn cue(index: i64, text: &str) -> TranslationPlannerCue {
        TranslationPlannerCue {
            segment_id: format!("cue-{index}"),
            segment_index: index,
            start_ms: index * 1_000,
            end_ms: (index + 1) * 1_000,
            source_text: text.into(),
        }
    }

    #[test]
    fn sparse_targets_never_become_one_semantic_group() {
        let timeline = vec![
            cue(0, "前"),
            cue(1, "対象一"),
            cue(2, "中"),
            cue(3, "対象二"),
        ];
        let targets = HashSet::from(["cue-1".to_string(), "cue-3".to_string()]);
        let plan = TranslationPlanner::default()
            .plan(&timeline, &targets)
            .unwrap();
        assert_eq!(plan.groups.len(), 2);
        assert_eq!(plan.groups[0].targets[0].segment_id, "cue-1");
        assert_eq!(plan.groups[1].targets[0].segment_id, "cue-3");
    }

    #[test]
    fn sentence_boundary_starts_a_new_group_with_frozen_context() {
        let timeline = vec![
            cue(0, "前"),
            cue(1, "文末。"),
            cue(2, "次の文"),
            cue(3, "後"),
        ];
        let targets = HashSet::from(["cue-1".to_string(), "cue-2".to_string()]);
        let plan = TranslationPlanner::default()
            .plan(&timeline, &targets)
            .unwrap();
        assert_eq!(plan.groups.len(), 2);
        assert_eq!(plan.groups[0].before_context[0].source_text, "前");
        assert_eq!(plan.groups[1].after_context[0].source_text, "後");
    }
}
