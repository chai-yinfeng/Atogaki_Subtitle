use anyhow::{Result, anyhow, ensure};
use atogaki_subtitle::{
    application::LocalLearningService,
    infrastructure::local_db::{
        LocalLearningItemDetail, NewLocalLearningGrammarSelection, NewLocalLearningSelection,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub(crate) struct Token<'a> {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) surface: &'a str,
}

pub(crate) fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut words = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut offset = 0;
    let mut start = None;
    while let Some((byte, ch)) = chars.next() {
        let inner_join = matches!(ch, '\'' | '’' | '-')
            && start.is_some()
            && chars.peek().is_some_and(|(_, next)| next.is_alphabetic());
        if ch.is_alphabetic() || inner_join {
            start.get_or_insert((byte, offset));
        } else if let Some((from_byte, from)) = start.take() {
            words.push(Token {
                byte_start: from_byte,
                byte_end: byte,
                start: from,
                end: offset,
                surface: &text[from_byte..byte],
            });
        }
        offset += ch.len_utf16();
    }
    if let Some((from_byte, from)) = start {
        words.push(Token {
            byte_start: from_byte,
            byte_end: text.len(),
            start: from,
            end: offset,
            surface: &text[from_byte..],
        });
    }
    words
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Selection {
    pub(crate) job_id: String,
    pub(crate) segment_id: String,
    pub(crate) source_text: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveRequest {
    pub(crate) selection: Selection,
    pub(crate) id: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct GrammarMatch {
    pub(crate) id: &'static str,
    title: &'static str,
    explanation: &'static str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

const PARTICIPLES: &[&str] = &[
    "been", "begun", "come", "done", "eaten", "felt", "found", "given", "gone", "had", "heard",
    "kept", "known", "left", "lost", "made", "met", "read", "run", "said", "seen", "shown",
    "spoken", "taken", "thought", "told", "used", "won", "written",
];

pub(crate) fn analyze(selection: &Selection) -> Result<Vec<GrammarMatch>> {
    let text = &selection.source_text;
    let length = text.encode_utf16().count();
    ensure!(
        length <= 4096 && selection.start < selection.end && selection.end <= length,
        "请选择有效的字幕范围"
    );
    let mut matches = Vec::new();
    for pair in tokens(text).windows(2) {
        let [left, right] = [pair[0], pair[1]];
        if !text[left.byte_end..right.byte_start]
            .chars()
            .all(char::is_whitespace)
            || left.start >= selection.end
            || right.end <= selection.start
        {
            continue;
        }
        let first = left.surface.to_ascii_lowercase().replace('’', "'");
        let next = right.surface.to_ascii_lowercase().replace('’', "'");
        let be = matches!(
            first.as_str(),
            "am" | "is"
                | "are"
                | "was"
                | "were"
                | "be"
                | "been"
                | "being"
                | "i'm"
                | "you're"
                | "we're"
                | "they're"
        );
        let have = matches!(
            first.as_str(),
            "have" | "has" | "had" | "i've" | "you've" | "we've" | "they've"
        );
        let participle =
            PARTICIPLES.contains(&next.as_str()) || (next.len() > 4 && next.ends_with("ed"));
        let rule = if be && next.len() > 4 && next.ends_with("ing") {
            Some((
                "en:progressive",
                "be + -ing",
                "be 后接动词的 -ing 形式，常用来描述当时正在发生的动作。也可能用于已安排的计划。",
            ))
        } else if have && participle {
            Some((
                "en:perfect",
                "have + 过去分词",
                "have、has 或 had 后接过去分词，常用来把之前发生的动作或状态与另一个时间点联系起来。",
            ))
        } else if matches!(
            (first.as_str(), next.as_str()),
            ("there", "is" | "are" | "was" | "were")
        ) {
            Some((
                "en:existential",
                "there + be",
                "用来引入存在的人或事物，例如 there are two chairs。",
            ))
        } else if matches!((first.as_str(), next.as_str()), ("used", "to")) {
            Some((
                "en:used-to",
                "used to",
                "后接动词原形时常表示过去的习惯或状态。若前面有 be，且后接名词或 -ing，通常表示习惯于某事。",
            ))
        } else if matches!((first.as_str(), next.as_str()), ("going", "to")) {
            Some((
                "en:going-to",
                "going to",
                "后接动词原形时常表示计划或预测；后接地点时也可能是正在前往某地。",
            ))
        } else if matches!(first.as_str(), "don't" | "doesn't" | "didn't") {
            Some((
                "en:do-negation",
                "do + not",
                "用 do、does 或 did 否定一般动词，后面的主要动词通常用原形。",
            ))
        } else {
            None
        };
        if let Some((id, title, explanation)) = rule {
            matches.push(GrammarMatch {
                id,
                title,
                explanation,
                start: left.start,
                end: right.end,
            });
        }
    }
    matches.truncate(8);
    Ok(matches)
}

pub(crate) async fn save(
    learning: &LocalLearningService,
    request: SaveRequest,
) -> Result<LocalLearningItemDetail> {
    let selection = request.selection;
    let matched = analyze(&selection)?
        .into_iter()
        .find(|matched| {
            matched.id == request.id && matched.start == request.start && matched.end == request.end
        })
        .ok_or_else(|| anyhow!("句型已变化，请重新分析"))?;
    let cues = learning
        .grammar_context(
            &selection.job_id,
            &selection.segment_id,
            &selection.segment_id,
            "en",
        )
        .await?;
    let cue = cues
        .into_iter()
        .find(|cue| cue.id == selection.segment_id)
        .filter(|cue| cue.source_text == selection.source_text)
        .ok_or_else(|| anyhow!("字幕已变化，请重新选句"))?;
    learning
        .save_grammar_selection(
            NewLocalLearningSelection {
                job_id: selection.job_id,
                segment_id: selection.segment_id,
                item_type: "selection".into(),
                selection_start_utf16: matched.start.try_into()?,
                selection_end_utf16: matched.end.try_into()?,
            },
            NewLocalLearningGrammarSelection {
                source_language: "en",
                grammar_id: matched.id.into(),
                title: matched.title.into(),
                meaning: matched.explanation.into(),
                cues: vec![cue],
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_utf16_ranges_and_does_not_match_across_punctuation() {
        let text = "🙂 They’ve seen it. We are, surprisingly, leaving. There are two chairs.";
        let matches = analyze(&Selection {
            job_id: String::new(),
            segment_id: String::new(),
            source_text: text.into(),
            start: 0,
            end: text.encode_utf16().count(),
        })
        .unwrap();
        assert_eq!(
            matches.iter().map(|m| m.id).collect::<Vec<_>>(),
            ["en:perfect", "en:existential"]
        );
        let chars = text.encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            String::from_utf16(&chars[matches[0].start..matches[0].end]).unwrap(),
            "They’ve seen"
        );
    }
}
