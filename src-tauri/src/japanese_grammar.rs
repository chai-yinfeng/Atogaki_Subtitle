pub(crate) mod catalog;
mod rules;

use anyhow::{Result, anyhow, ensure};
use atogaki_subtitle::{
    application::LocalLearningService,
    infrastructure::local_db::{
        LocalLearningItemDetail, LocalSubtitleSegmentRecord, NewLocalLearningGrammarSelection,
        NewLocalLearningSelection,
    },
};
use serde::{Deserialize, Serialize};

use crate::dictionary_lookup::DictionaryLookupService;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Request {
    job_id: String,
    segment_id: String,
    source_text: String,
    start: usize,
    end: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveRequest {
    selection: Request,
    text: String,
    id: String,
    start: usize,
    end: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Preview {
    text: String,
    focus_start: usize,
    focus_end: usize,
    matches: Vec<rules::Match>,
}

struct Sentence {
    text: String,
    start_in_cues: usize,
    focus: (usize, usize),
    cues: Vec<LocalSubtitleSegmentRecord>,
}

impl Sentence {
    fn from_cues(mut cues: Vec<LocalSubtitleSegmentRecord>, request: &Request) -> Result<Self> {
        let anchor = cues
            .iter()
            .position(|cue| cue.id == request.segment_id)
            .ok_or_else(|| anyhow!("分析所需的字幕已变化"))?;
        let cue = &cues[anchor];
        ensure!(
            cue.source_text == request.source_text,
            "字幕已变化，请重新选句"
        );
        let text = cue.source_text.encode_utf16().collect::<Vec<_>>();
        ensure!(
            request.start < request.end && request.end <= text.len(),
            "请选择有效的字幕范围"
        );
        // Check that JavaScript UTF-16 offsets do not split a surrogate pair.
        String::from_utf16(&text[request.start..request.end])?;

        // Adjacent subtitles often split one sentence. Cap the window at one
        // neighboring cue per side, and stay inside nearby sentence punctuation.
        const MAX_GAP_MS: i64 = 750;
        let before =
            anchor > 0 && cue.start_ms.saturating_sub(cues[anchor - 1].end_ms) <= MAX_GAP_MS;
        let after = cues
            .get(anchor + 1)
            .is_some_and(|next| next.start_ms.saturating_sub(cue.end_ms) <= MAX_GAP_MS);
        let first = anchor - usize::from(before);
        let last = anchor + usize::from(after);
        let anchor_start = if before {
            cues[first].source_text.encode_utf16().count()
        } else {
            0
        };
        let joined = cues[first..=last]
            .iter()
            .flat_map(|cue| cue.source_text.encode_utf16())
            .collect::<Vec<_>>();
        let start = anchor_start + request.start;
        let end = anchor_start + request.end;
        let hard_break = |ch: u16| matches!(ch, 0x3002 | 0xff01 | 0xff1f | 0x21 | 0x3f | 10 | 13);
        let left = joined[..start]
            .iter()
            .rposition(|&ch| hard_break(ch))
            .map_or(0, |position| position + 1);
        let right = if hard_break(joined[end - 1]) {
            end
        } else {
            joined[end..]
                .iter()
                .position(|&ch| hard_break(ch))
                .map_or(joined.len(), |position| end + position + 1)
        };
        ensure!(right - left <= 4096, "字幕范围过长，请缩小选区");
        Ok(Self {
            text: String::from_utf16(&joined[left..right])?,
            start_in_cues: left,
            focus: (start - left, end - left),
            cues: cues.drain(first..=last).collect(),
        })
    }

    fn example(self, found: &rules::Match) -> Result<(Vec<LocalSubtitleSegmentRecord>, i64, i64)> {
        let start = self.start_in_cues + found.start;
        let end = self.start_in_cues + found.end;
        let mut offset = 0;
        let mut first_offset = None;
        let cues = self
            .cues
            .into_iter()
            .filter(|cue| {
                let cue_start = offset;
                offset += cue.source_text.encode_utf16().count();
                let overlaps = cue_start < end && offset > start;
                if overlaps {
                    first_offset.get_or_insert(cue_start);
                }
                overlaps
            })
            .collect::<Vec<_>>();
        let first_offset = first_offset.ok_or_else(|| anyhow!("找不到句型原文"))?;
        Ok((
            cues,
            (start - first_offset).try_into()?,
            (end - first_offset).try_into()?,
        ))
    }
}

async fn analyze(
    dictionary: &DictionaryLookupService,
    learning: &LocalLearningService,
    request: &Request,
) -> Result<(Sentence, Vec<rules::Match>)> {
    let cues = learning
        .grammar_context(&request.job_id, &request.segment_id)
        .await?;
    let sentence = Sentence::from_cues(cues, request)?;
    let tokens = dictionary.grammar_tokens(sentence.text.clone()).await?;
    let matches = rules::find(&tokens, sentence.focus);
    Ok((sentence, matches))
}

pub(crate) async fn preview(
    dictionary: &DictionaryLookupService,
    learning: &LocalLearningService,
    request: &Request,
) -> Result<Preview> {
    let (sentence, matches) = analyze(dictionary, learning, request).await?;
    Ok(Preview {
        text: sentence.text,
        focus_start: sentence.focus.0,
        focus_end: sentence.focus.1,
        matches,
    })
}

pub(crate) async fn save(
    dictionary: &DictionaryLookupService,
    learning: &LocalLearningService,
    request: SaveRequest,
) -> Result<LocalLearningItemDetail> {
    let (sentence, hits) = analyze(dictionary, learning, &request.selection).await?;
    ensure!(sentence.text == request.text, "字幕已变化，请重新分析句型");
    let found = hits
        .iter()
        .find(|hit| hit.id == request.id && hit.start == request.start && hit.end == request.end)
        .ok_or_else(|| anyhow!("句型结果已变化，请重新分析"))?;
    let entry = catalog::get(found.id).ok_or_else(|| anyhow!("找不到对应语法条目"))?;
    let (cues, start, end) = sentence.example(found)?;
    let segment_id = cues
        .first()
        .ok_or_else(|| anyhow!("找不到句型原文"))?
        .id
        .clone();
    learning
        .save_grammar_selection(
            NewLocalLearningSelection {
                job_id: request.selection.job_id,
                segment_id,
                item_type: "selection".into(),
                selection_start_utf16: start,
                selection_end_utf16: end,
            },
            NewLocalLearningGrammarSelection {
                grammar_id: entry.id.clone(),
                title: entry.pattern.clone(),
                meaning: entry.meaning.clone(),
                cues,
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use atogaki_subtitle::{
        application::{
            JobSnapshot,
            job_manifest::JobManifest,
            job_status::JobStatus,
            translation_provider::{MutableTranslationProvider, UnconfiguredTranslationProvider},
        },
        domain::TranscriptSegment,
        infrastructure::{job_store::Job, local_db::LocalDatabase},
    };

    use crate::{
        desktop_settings::DesktopSettingsService, japanese_morphology::JapaneseMorphology,
    };

    use super::*;

    #[test]
    #[ignore = "requires ATOGAKI_DICTIONARY_SMOKE_DIR with a Sudachi Core package"]
    fn matches_all_supported_patterns_and_rejects_common_false_matches() {
        let root = PathBuf::from(std::env::var_os("ATOGAKI_DICTIONARY_SMOKE_DIR").unwrap());
        let morphology = JapaneseMorphology::default();
        let cases = [
            ("まだ食べていなかった。", "te-iru", "食べていなかった"),
            ("電車を待ってる。", "te-iru", "待ってる"),
            ("そう言ってた。", "te-iru", "言ってた"),
            ("食べちゃった。", "te-shimau-chau", "食べちゃった"),
            ("本を読んじゃう。", "te-shimau-chau", "読んじゃう"),
            ("先に予約しておいた。", "te-oku", "しておいた"),
            ("あとで準備しとく。", "te-oku", "しとく"),
            ("外で食べてみた。", "te-miru", "食べてみた"),
            ("ちょっと待ってください。", "te-kudasai", "待ってください"),
            ("雨が降りそうだ。", "sou-da-appearance", "降りそうだ"),
            ("重くなさそう。", "sou-da-appearance", "なさそう"),
            ("雨が降るそうだ。", "sou-da-hearsay", "降るそうだ"),
            ("元気だそうです。", "sou-da-hearsay", "元気だそうです"),
            ("雨が降ったら帰る。", "tara", "降ったら"),
            ("安ければ買う。", "ba", "安ければ"),
            ("東京なら行ける。", "nara", "東京なら"),
            ("音楽を聞きながら歩く。", "nagara", "聞きながら"),
            ("毎朝運動するようにする。", "you-ni-suru", "するようにする"),
            ("留学することにした。", "koto-ni-suru", "することにした"),
            ("転校することになった。", "koto-ni-naru", "することになった"),
            ("忙しくても行く。", "te-mo-de-mo", "忙しくても"),
            ("雨が降っても行く。", "te-mo-de-mo", "降っても"),
            (
                "疲れたわけではない。",
                "wake-de-wa-nai",
                "疲れたわけではない",
            ),
            (
                "疲れたわけじゃない。",
                "wake-de-wa-nai",
                "疲れたわけじゃない",
            ),
            (
                "行かなければならない。",
                "nakereba-naranai-nakute-wa-ikenai",
                "行かなければならない",
            ),
            (
                "行かなきゃ。",
                "nakereba-naranai-nakute-wa-ikenai",
                "行かなきゃ",
            ),
            ("行かなくてもいい。", "nakute-mo-ii", "行かなくてもいい"),
            ("言うまでもなく難しい。", "made-mo-nai", "言うまでもなく"),
            (
                "努力したにもかかわらず失敗した。",
                "ni-mo-kakawarazu",
                "したにもかかわらず",
            ),
            (
                "一人で食べざるを得ない。",
                "zaru-o-enai",
                "食べざるを得ない",
            ),
            ("これに違いない。", "ni-chigai-nai", "これに違いない"),
            ("疲れたから帰る。", "kara-reason", "疲れたから"),
            (
                "せっかく来たものの、閉まっていた。",
                "mono-no",
                "来たものの",
            ),
        ];
        let mut tested = std::collections::HashSet::new();
        for (text, id, written) in cases {
            let tokens = morphology.grammar_tokens(&root, text).unwrap();
            let chars = text.encode_utf16().collect::<Vec<_>>();
            let hits = rules::find(&tokens, (0, chars.len()));
            assert!(
                hits.iter().any(|hit| hit.id == id
                    && String::from_utf16(&chars[hit.start..hit.end]).unwrap() == written),
                "{text}: {id} {written}, {hits:?}, {tokens:?}"
            );
            tested.insert(id);
        }
        assert_eq!(tested.len(), rules::rule_ids().count());
        for (text, excluded) in [
            ("雨が降るそうだ。", "sou-da-appearance"),
            ("雨が降りそうだ。", "sou-da-hearsay"),
            ("残念ながら行けない。", "nagara"),
            ("東京から来た。", "kara-reason"),
            ("本を読ん。でいた。", "te-iru"),
            ("本をよんｄでた。", "te-iru"),
            ("窓を開けて海を見る。", "te-miru"),
            ("行かなければならない。", "ba"),
        ] {
            let tokens = morphology.grammar_tokens(&root, text).unwrap();
            let hits = rules::find(&tokens, (0, text.encode_utf16().count()));
            assert!(
                hits.iter().all(|hit| hit.id != excluded),
                "{text}: {hits:?}"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires ATOGAKI_DICTIONARY_SMOKE_DIR with a Sudachi Core package"]
    async fn cross_cue_grammar_saves_one_source_and_preserves_existing_learning() {
        let dictionaries = PathBuf::from(std::env::var_os("ATOGAKI_DICTIONARY_SMOKE_DIR").unwrap());
        let root = std::env::temp_dir().join(format!("atogaki-grammar-{}", uuid::Uuid::new_v4()));
        let database = LocalDatabase::open(root.join("atogaki.sqlite"))
            .await
            .unwrap();
        let learning = LocalLearningService::new(database.clone());
        let settings = DesktopSettingsService::new(
            database.clone(),
            MutableTranslationProvider::new(Arc::new(UnconfiguredTranslationProvider)),
            root.join("models"),
            None,
            None,
            None,
            None,
        );
        let dictionary = DictionaryLookupService::new(dictionaries, learning.clone(), settings);
        let job = Job::create_in(&root).unwrap();
        let mut manifest = JobManifest::new(&job, None, None, Default::default());
        manifest.mark(JobStatus::Done);
        let cues = [
            TranscriptSegment::new(0, 800, "別の話。".into()),
            TranscriptSegment::new(900, 2000, "📖まだ食べて".into()),
            TranscriptSegment::new(2090, 3500, "いなかった。".into()),
            TranscriptSegment::new(3650, 5100, "雨が降りそうだ。".into()),
            TranscriptSegment::new(5200, 6300, "また外で待ってる。".into()),
        ];
        let job_id = manifest.job_id.clone();
        database
            .sync_snapshot(&JobSnapshot {
                manifest,
                segments: cues.to_vec(),
            })
            .await
            .unwrap();
        let select = |index: usize, start, end| Request {
            job_id: job_id.clone(),
            segment_id: cues[index].id.clone(),
            source_text: cues[index].source_text.clone(),
            start,
            end,
        };
        let send = |selection, text: String, hit: &rules::Match| SaveRequest {
            selection,
            text,
            id: hit.id.into(),
            start: hit.start,
            end: hit.end,
        };

        let previewed = preview(&dictionary, &learning, &select(1, 4, 6))
            .await
            .unwrap();
        assert_eq!(previewed.text, "📖まだ食べていなかった。");
        let hit = &previewed.matches[0];
        assert_eq!(hit.id, "te-iru");
        assert_eq!(hit.form_note, Some("过去否定"));
        let collected = save(
            &dictionary,
            &learning,
            send(select(1, 4, 6), previewed.text.clone(), hit),
        )
        .await
        .unwrap();
        assert_eq!(collected.item.grammar_id.as_deref(), Some("te-iru"));
        assert_eq!(collected.occurrences[0].start_ms, 900);
        assert_eq!(collected.occurrences[0].end_ms, 3500);
        assert_eq!(
            collected.occurrences[0].segment_source_snapshot,
            previewed.text
        );
        assert_eq!(collected.occurrences[0].selection_start_utf16, 4);

        let other_anchor = preview(&dictionary, &learning, &select(2, 0, 4))
            .await
            .unwrap();
        let duplicate = save(
            &dictionary,
            &learning,
            send(
                select(2, 0, 4),
                other_anchor.text.clone(),
                &other_anchor.matches[0],
            ),
        )
        .await
        .unwrap();
        assert_eq!(duplicate.item.id, collected.item.id);
        assert_eq!(duplicate.occurrences.len(), 1);

        let old = learning
            .save_selection(NewLocalLearningSelection {
                job_id: job_id.clone(),
                segment_id: cues[1].id.clone(),
                item_type: "selection".into(),
                selection_start_utf16: 4,
                selection_end_utf16: 7,
            })
            .await
            .unwrap();
        assert_eq!(old.item.grammar_id, None);
        assert_eq!(old.item.source_text, "食べて");

        let second = preview(&dictionary, &learning, &select(4, 0, 9))
            .await
            .unwrap();
        let same_card = save(
            &dictionary,
            &learning,
            send(select(4, 0, 9), second.text.clone(), &second.matches[0]),
        )
        .await
        .unwrap();
        assert_eq!(same_card.item.id, collected.item.id);
        assert_eq!(same_card.occurrences.len(), 2);

        database
            .update_segment(
                &job_id,
                &cues[2].id,
                "いなかったね。".into(),
                None,
                2090,
                3500,
            )
            .await
            .unwrap();
        assert!(
            save(
                &dictionary,
                &learning,
                send(select(1, 4, 6), previewed.text.clone(), hit)
            )
            .await
            .is_err()
        );
        database.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}
