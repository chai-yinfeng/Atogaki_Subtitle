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
    end_cue: Option<SelectionEnd>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectionEnd {
    segment_id: String,
    source_text: String,
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

impl Request {
    fn end_segment_id(&self) -> &str {
        self.end_cue
            .as_ref()
            .map_or(&self.segment_id, |end| &end.segment_id)
    }
}

const MAX_CUE_GAP_MS: i64 = 750;

fn near(left: &LocalSubtitleSegmentRecord, right: &LocalSubtitleSegmentRecord) -> bool {
    right.start_ms.saturating_sub(left.end_ms) <= MAX_CUE_GAP_MS
}

struct Sentence {
    text: String,
    start_in_cues: usize,
    focus: (usize, usize),
    breaks: Vec<usize>,
    cues: Vec<LocalSubtitleSegmentRecord>,
}

impl Sentence {
    fn from_cues(mut cues: Vec<LocalSubtitleSegmentRecord>, request: &Request) -> Result<Self> {
        let selection_first = cues
            .iter()
            .position(|cue| cue.id == request.segment_id)
            .ok_or_else(|| anyhow!("分析所需的字幕已变化"))?;
        let selection_last = cues
            .iter()
            .position(|cue| cue.id == request.end_segment_id())
            .filter(|&index| index >= selection_first)
            .ok_or_else(|| anyhow!("分析所需的字幕已变化"))?;
        let first_cue = &cues[selection_first];
        let last_cue = &cues[selection_last];
        let last_snapshot = request
            .end_cue
            .as_ref()
            .map_or(&request.source_text, |end| &end.source_text);
        ensure!(
            first_cue.source_text == request.source_text && last_cue.source_text == *last_snapshot,
            "字幕已变化，请重新选句"
        );
        let valid_offset = |text: &str, offset: usize| {
            let chars = text.encode_utf16().collect::<Vec<_>>();
            offset <= chars.len()
                && !chars
                    .get(offset)
                    .is_some_and(|ch| (0xdc00..=0xdfff).contains(ch))
        };
        ensure!(
            valid_offset(&first_cue.source_text, request.start)
                && valid_offset(&last_cue.source_text, request.end),
            "请选择有效的字幕范围"
        );

        // Extend the outer boundaries for context. Explicitly selected cues on
        // either side of a pause are analyzed independently.
        let first = selection_first
            - usize::from(selection_first > 0 && near(&cues[selection_first - 1], first_cue));
        let last = selection_last
            + usize::from(
                cues.get(selection_last + 1)
                    .is_some_and(|next| near(last_cue, next)),
            );
        let mut joined = Vec::new();
        let mut breaks = Vec::new();
        let mut start = 0;
        let mut end = 0;
        for index in first..=last {
            if index > first && !near(&cues[index - 1], &cues[index]) {
                breaks.push(joined.len());
            }
            if index == selection_first {
                start = joined.len() + request.start;
            }
            if index == selection_last {
                end = joined.len() + request.end;
            }
            joined.extend(cues[index].source_text.encode_utf16());
        }
        ensure!(start < end, "请选择有效的字幕范围");
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
        breaks.retain(|offset| left < *offset && *offset < right);
        breaks.iter_mut().for_each(|offset| *offset -= left);
        breaks.dedup();
        Ok(Self {
            text: String::from_utf16(&joined[left..right])?,
            start_in_cues: left,
            focus: (start - left, end - left),
            breaks,
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
        .grammar_context(
            &request.job_id,
            &request.segment_id,
            request.end_segment_id(),
        )
        .await?;
    let sentence = Sentence::from_cues(cues, request)?;
    let mut matches = Vec::new();
    if sentence.breaks.is_empty() {
        let tokens = dictionary.grammar_tokens(sentence.text.clone()).await?;
        matches = rules::find(&tokens, sentence.focus);
    } else {
        let text = sentence.text.encode_utf16().collect::<Vec<_>>();
        let mut start = 0;
        for end in sentence
            .breaks
            .iter()
            .copied()
            .chain(std::iter::once(text.len()))
        {
            if start < sentence.focus.1 && end > sentence.focus.0 {
                let focus = (
                    sentence.focus.0.saturating_sub(start),
                    sentence.focus.1.min(end) - start,
                );
                let tokens = dictionary
                    .grammar_tokens(String::from_utf16(&text[start..end])?)
                    .await?;
                matches.extend(rules::find(&tokens, focus).into_iter().map(|mut hit| {
                    hit.start += start;
                    hit.end += start;
                    hit
                }));
            }
            start = end;
        }
    }
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
            ("荷物を持ってあげた。", "te-ageru", "持ってあげた"),
            ("友達が手伝ってくれた。", "te-kureru", "手伝ってくれた"),
            ("先輩に荷物を運んでもらった。", "te-morau", "運んでもらった"),
            (
                "予定を送っていただいた。",
                "te-kudasaru-te-itadaku",
                "送っていただいた",
            ),
            ("予定が書いてある。", "te-aru", "書いてある"),
            ("ここで待ってほしい。", "te-hoshii", "待ってほしい"),
            ("ご飯を食べてから出かける。", "te-kara", "食べてから"),
            ("写真を撮ってもいいですか。", "te-mo-ii", "撮ってもいいです"),
            ("風が強くてもいい。", "te-mo-ii", "強くてもいい"),
            (
                "写真を撮っても構わない。",
                "te-mo-kamawanai",
                "撮っても構わない",
            ),
            (
                "写真を撮ってはいけない。",
                "te-wa-ikenai",
                "撮ってはいけない",
            ),
            (
                "毎日走れるようになった。",
                "you-ni-naru",
                "走れるようになった",
            ),
            (
                "誰でも参加することができる。",
                "koto-ga-dekiru",
                "することができる",
            ),
            ("留学したことがある。", "ta-koto-ga-aru", "したことがある"),
            (
                "展示品に触ってはいけません。",
                "te-wa-ikenai",
                "触ってはいけません",
            ),
            (
                "返事は遅くなっても構いません。",
                "te-mo-kamawanai",
                "なっても構いません",
            ),
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
            ("パンしか買わない。", "shika-nai", "しか買わない"),
            ("パンしか買いません。", "shika-nai", "しか買いません"),
            ("財布に五百円しかない。", "shika-nai", "しかない"),
            (
                "このことは、あなたにしか話していません。",
                "shika-nai",
                "しか話していません",
            ),
            (
                "荷物しか家では見つからなかった。",
                "shika-nai",
                "しか家では見つからなかった",
            ),
            (
                "今日は歩くしかなかった。",
                "shika-nai-no-choice",
                "歩くしかなかった",
            ),
            (
                "今は待つしかありません。",
                "shika-nai-no-choice",
                "待つしかありません",
            ),
            ("時間さえあれば行ける。", "sae-ba", "さえあれば"),
            (
                "薬を飲んで寝てさえいれば、すぐに治りますよ。",
                "sae-ba",
                "さえいれば",
            ),
            (
                "報告書さえ明日までに書けばいい。",
                "sae-ba",
                "さえ明日までに書けば",
            ),
        ];
        let new_cases = [
            ("声が小さくなっていった。", "te-iku", "なっていった"),
            ("風が強くなってきました。", "te-kuru", "なってきました"),
            ("あの箱は重すぎた。", "sugiru", "重すぎた"),
            ("設定が複雑すぎます。", "sugiru", "複雑すぎます"),
            (
                "明日は帰らないつもりです。",
                "tsumori",
                "帰らないつもりです",
            ),
            (
                "明日は出かけないつもりです。",
                "tsumori",
                "出かけないつもりです",
            ),
            ("来月また訪ねる予定だった。", "yotei", "訪ねる予定だった"),
            ("開催は夕方の予定です。", "yotei", "夕方の予定です"),
            ("あの紙は丈夫なはずだった。", "hazu-da", "丈夫なはずだった"),
            ("店が開いたはずがない。", "hazu-ga-nai", "開いたはずがない"),
            ("引っ越したばかりの家だ。", "ta-bakari", "引っ越したばかり"),
            (
                "目を閉じたとたんに音がした。",
                "ta-totan-ta-totan-ni",
                "閉じたとたんに",
            ),
            ("この箱は開けにくかった。", "yasui-nikui", "開けにくかった"),
            ("この道は歩きやすい。", "yasui-nikui", "歩きやすい"),
            ("初対面では話しづらいです。", "zurai", "話しづらいです"),
            (
                "途中で雨が降り始めた。",
                "hajimeru-owaru-tsuzukeru",
                "降り始めた",
            ),
            (
                "お皿を洗い終わりました。",
                "hajimeru-owaru-tsuzukeru",
                "洗い終わりました",
            ),
            ("何も言わないで帰った。", "naide", "言わないで"),
            (
                "先に帰らないでください。",
                "naide-kudasai",
                "帰らないでください",
            ),
            ("連絡せずに帰った。", "zu-ni", "せずに"),
        ];
        let mut tested = std::collections::HashSet::new();
        let mut failures = Vec::new();
        for (text, id, written) in cases.into_iter().chain(new_cases) {
            let tokens = morphology.grammar_tokens(&root, text).unwrap();
            let chars = text.encode_utf16().collect::<Vec<_>>();
            let hits = rules::find(&tokens, (0, chars.len()));
            if !hits.iter().any(|hit| {
                hit.id == id && String::from_utf16(&chars[hit.start..hit.end]).unwrap() == written
            }) {
                failures.push(format!("{text}: {id} {written}, {hits:?}, {tokens:?}"));
            }
            tested.insert(id);
        }
        for id in new_cases
            .into_iter()
            .map(|(_, id, _)| id)
            .collect::<std::collections::BTreeSet<_>>()
        {
            for example in &catalog::get(id).unwrap().examples {
                let tokens = morphology.grammar_tokens(&root, &example.ja).unwrap();
                let hits = rules::find(&tokens, (0, example.ja.encode_utf16().count()));
                if !hits.iter().any(|hit| hit.id == id) {
                    failures.push(format!(
                        "目录例句 {}: {id}, {hits:?}, {tokens:?}",
                        example.ja
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        assert_eq!(tested.len(), rules::rule_ids().count());
        for (text, excluded) in [
            ("雨が降るそうだ。", "sou-da-appearance"),
            ("雨が降りそうだ。", "sou-da-hearsay"),
            ("残念ながら行けない。", "nagara"),
            ("東京から来た。", "kara-reason"),
            ("本を読ん。でいた。", "te-iru"),
            ("本をよんｄでた。", "te-iru"),
            ("窓を開けて海を見る。", "te-miru"),
            ("駅で荷物をもらった。", "te-morau"),
            ("駅で降りて、お菓子をあげた。", "te-ageru"),
            ("荷物がある。", "te-aru"),
            ("会場で会ってから帰る。", "kara-reason"),
            ("朝は駅から歩く。", "te-kara"),
            ("写真を撮ってもいいですか。", "te-mo-de-mo"),
            ("写真を撮っても構わない。", "te-mo-de-mo"),
            ("雨が降っても行く。", "te-mo-ii"),
            ("批判されても構わず続ける。", "te-mo-kamawanai"),
            ("洋服を着てはいける。", "te-wa-ikenai"),
            ("東京へ行くことがある。", "ta-koto-ga-aru"),
            ("行かなければならない。", "ba"),
            ("歩くしかない。", "shika-nai"),
            ("本しかない。", "shika-nai-no-choice"),
            ("「パンしか」とは言っていない。", "shika-nai"),
            ("パンしか買う人が来ない。", "shika-nai"),
            ("パンしか買って、紅茶は飲まない。", "shika-nai"),
            ("パンしか買ったが何も食べない。", "shika-nai"),
            ("パンしか買った。", "shika-nai"),
            ("時間さえあれば行ける。", "ba"),
            ("時間さえないが、お金があれば行ける。", "sae-ba"),
            ("この本を読む人さえいない。", "sae-ba"),
            ("本さえある人が来ればいい。", "sae-ba"),
            ("せっかく来たものの値段を調べた。", "mono-no"),
            ("駅で降りて市場に行った。", "te-iku"),
            ("先に電話して、友達が来た。", "te-kuru"),
            ("バスは学校の前を過ぎた。", "sugiru"),
            ("そのつもりです。", "tsumori"),
            ("来週の予定表を作った。", "yotei"),
            ("きっと届くはずがない。", "hazu-da"),
            ("きっと届くはずです。", "hazu-ga-nai"),
            ("水ばかり飲んでいる。", "ta-bakari"),
            ("屋根はトタンでできている。", "ta-totan-ta-totan-ni"),
            ("安い箱を開けた。", "yasui-nikui"),
            ("彼が失敗するとは想像し難い。", "yasui-nikui"),
            ("この料理は辛い。", "zurai"),
            ("料理を作って、片づけを始めた。", "hajimeru-owaru-tsuzukeru"),
            ("先に帰らないでください。", "naide"),
            ("明日は来ないでしょう。", "naide-kudasai"),
            ("静かになってから席に着いた。", "zu-ni"),
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
            TranscriptSegment::new(6450, 7300, "📖その話は君にしか".into()),
            TranscriptSegment::new(7400, 8700, "伝えていません。".into()),
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
            end_cue: None,
        };
        let send = |selection, text: String, hit: &rules::Match| SaveRequest {
            selection,
            text,
            id: hit.id.into(),
            start: hit.start,
            end: hit.end,
        };

        let select_cross_cue = || Request {
            end_cue: Some(SelectionEnd {
                segment_id: cues[2].id.clone(),
                source_text: cues[2].source_text.clone(),
            }),
            ..select(1, 4, 4)
        };
        let previewed = preview(&dictionary, &learning, &select_cross_cue())
            .await
            .unwrap();
        assert_eq!(previewed.text, "📖まだ食べていなかった。");
        assert_eq!((previewed.focus_start, previewed.focus_end), (4, 11));
        let hit = &previewed.matches[0];
        assert_eq!(hit.id, "te-iru");
        assert_eq!(hit.form_note, Some("过去否定"));
        let collected = save(
            &dictionary,
            &learning,
            send(select_cross_cue(), previewed.text.clone(), hit),
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

        let select_restriction = || Request {
            end_cue: Some(SelectionEnd {
                segment_id: cues[6].id.clone(),
                source_text: cues[6].source_text.clone(),
            }),
            ..select(5, 8, 4)
        };
        let restricted = preview(&dictionary, &learning, &select_restriction())
            .await
            .unwrap();
        let hit_restriction = restricted
            .matches
            .iter()
            .find(|hit| hit.id == "shika-nai")
            .unwrap();
        assert_eq!((hit_restriction.start, hit_restriction.end), (8, 17));
        let item = save(
            &dictionary,
            &learning,
            send(select_restriction(), restricted.text, hit_restriction),
        )
        .await
        .unwrap();
        assert_eq!(item.item.grammar_id.as_deref(), Some("shika-nai"));
        assert_eq!(item.occurrences[0].start_ms, 6450);
        assert_eq!(item.occurrences[0].end_ms, 8700);
        database
            .update_segment(
                &job_id,
                &cues[6].id,
                cues[6].source_text.clone(),
                None,
                8200,
                8700,
            )
            .await
            .unwrap();
        let separated = preview(&dictionary, &learning, &select_restriction())
            .await
            .unwrap();
        assert!(separated.matches.iter().all(|hit| hit.id != "shika-nai"));

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
                send(select_cross_cue(), previewed.text.clone(), hit)
            )
            .await
            .is_err()
        );
        database
            .update_segment(
                &job_id,
                &cues[2].id,
                cues[2].source_text.clone(),
                None,
                3000,
                3500,
            )
            .await
            .unwrap();
        let separated = preview(&dictionary, &learning, &select_cross_cue())
            .await
            .unwrap();
        assert!(separated.matches.iter().all(|hit| hit.id != "te-iru"));
        database.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}
