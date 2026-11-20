use serde::Serialize;

use crate::japanese_morphology::GrammarToken;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Match {
    pub id: &'static str,
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_note: Option<&'static str>,
}

struct Rule {
    id: &'static str,
    pattern: Pattern,
    extend_aux: bool,
}

type Sequence = &'static [&'static str];

enum Pattern {
    Variants(&'static [Sequence]),
    Suffix {
        prefix: Sequence,
        tail: Sequence,
        ending: Ending,
    },
    Context {
        left: &'static [Step],
        core: &'static [Step],
        right: &'static [Step],
    },
}

enum Step {
    Token(&'static str),
    Optional(Sequence),
    Alternatives(&'static [Sequence]),
    Gap { through: &'static str, max: usize },
}

impl Step {
    fn max_width(&self) -> usize {
        match self {
            Self::Token(_) => 1,
            Self::Optional(sequence) => sequence.len(),
            Self::Alternatives(choices) => {
                choices.iter().map(|choice| choice.len()).max().unwrap_or(0)
            }
            Self::Gap { max, .. } => *max,
        }
    }
}

#[derive(Clone, Copy)]
enum Ending {
    Regular,
    ContractedTe(&'static str),
    Negative,
}

const MASEN: Sequence = &["s:ませ&l:ます&aux", "s:ん&l:ぬ&aux"];
const NEGATIVE_AUXILIARIES: &[Sequence] = &[&["l:ない&aux"], MASEN];
const NEGATIVE_PREDICATE_ENDINGS: &[Sequence] = &[&["l:ない&adjective|aux"], MASEN];

const ARGUMENT_GAP: Step = Step::Gap {
    through: "argument&!s:しか|さえ",
    max: 12,
};
const PREDICATE_GAP: Step = Step::Gap {
    through: "nonfinal-predicate",
    max: 6,
};

const VERB_STEM: Sequence = &["v:stem"];
const VERB_TE: Sequence = &["v:stem", "connector"];
const VERB_NAI: Sequence = &["v:negative", "l:ない&aux"];
const VERB_TA: Sequence = &["v:stem", "l:た&plain"];
const PREDICATE_TE: Sequence = &["v:stem|a:stem", "connector"];
const VERB_YOU_NI: Sequence = &["v:plain|neg:plain", "s:よう&na", "s:に"];
const VERB_KOTO_NI: Sequence = &["v:plain|neg:plain", "s:こと&noun", "s:に&particle"];
const PREDICATE_NOUN_PREFIX: Step = Step::Alternatives(&[
    &["plain"],
    &["na", "s:な&l:だ&aux"],
    &["nominal", "s:の&particle"],
]);

impl Rule {
    const fn variants(id: &'static str, variants: &'static [Sequence], extend_aux: bool) -> Self {
        Self {
            id,
            pattern: Pattern::Variants(variants),
            extend_aux,
        }
    }

    const fn suffix(id: &'static str, prefix: Sequence, tail: Sequence, extend_aux: bool) -> Self {
        Self {
            id,
            pattern: Pattern::Suffix {
                prefix,
                tail,
                ending: Ending::Regular,
            },
            extend_aux,
        }
    }

    const fn te_aux(id: &'static str, tail: Sequence, contraction: &'static str) -> Self {
        Self {
            id,
            pattern: Pattern::Suffix {
                prefix: VERB_TE,
                tail,
                ending: Ending::ContractedTe(contraction),
            },
            extend_aux: true,
        }
    }

    const fn negative_suffix(id: &'static str, prefix: Sequence, tail: Sequence) -> Self {
        Self {
            id,
            pattern: Pattern::Suffix {
                prefix,
                tail,
                ending: Ending::Negative,
            },
            extend_aux: true,
        }
    }

    const fn context(
        id: &'static str,
        left: &'static [Step],
        core: &'static [Step],
        right: &'static [Step],
        extend_aux: bool,
    ) -> Self {
        Self {
            id,
            pattern: Pattern::Context { left, core, right },
            extend_aux,
        }
    }

    fn end(&self, tokens: &[GrammarToken], start: usize) -> Option<usize> {
        match self.pattern {
            Pattern::Variants(variants) => variants
                .iter()
                .filter_map(|variant| sequence_end(tokens, start, variant.iter().copied()))
                .max(),
            Pattern::Suffix {
                prefix,
                tail,
                ending,
            } => {
                let regular = sequence_end(tokens, start, prefix.iter().chain(tail).copied());
                match ending {
                    Ending::Regular => regular,
                    Ending::ContractedTe(form) => regular.or_else(|| {
                        sequence_end(
                            tokens,
                            start,
                            VERB_STEM.iter().copied().chain(std::iter::once(form)),
                        )
                    }),
                    Ending::Negative => {
                        let end = regular?;
                        NEGATIVE_AUXILIARIES
                            .iter()
                            .find_map(|suffix| sequence_end(tokens, end, suffix.iter().copied()))
                    }
                }
            }
            Pattern::Context { left, core, right } => match_steps(tokens, start, core, &|end| {
                let left_width = left.iter().map(Step::max_width).sum::<usize>();
                let left_matches = (start.saturating_sub(left_width)..=start).any(|from| {
                    match_steps(&tokens[..start], from, left, &|left_end| {
                        (left_end == start).then_some(left_end)
                    })
                    .is_some()
                });
                if !left_matches {
                    return None;
                }
                match_steps(tokens, end, right, &|_| Some(end))
            }),
        }
    }
}

// Core, context and bounded gaps follow the design of nnj-grammar. Match the
// shortest viable gap first so a later predicate can't replace a nearer match.
fn match_steps(
    tokens: &[GrammarToken],
    position: usize,
    steps: &[Step],
    accept: &impl Fn(usize) -> Option<usize>,
) -> Option<usize> {
    let Some((step, rest)) = steps.split_first() else {
        return accept(position);
    };
    let continue_at = |end| match_steps(tokens, end, rest, accept);
    match step {
        Step::Token(spec) => {
            sequence_end(tokens, position, std::iter::once(*spec)).and_then(continue_at)
        }
        Step::Optional(sequence) => sequence_end(tokens, position, sequence.iter().copied())
            .and_then(continue_at)
            .or_else(|| continue_at(position)),
        Step::Alternatives(choices) => choices.iter().find_map(|choice| {
            sequence_end(tokens, position, choice.iter().copied()).and_then(continue_at)
        }),
        Step::Gap { through, max } => {
            let mut end = position;
            let limit = tokens.len().min(position + max);
            loop {
                if let Some(result) = continue_at(end) {
                    return Some(result);
                }
                if end == limit || !can_skip(&tokens[end], through) {
                    return None;
                }
                end += 1;
            }
        }
    }
}

fn can_skip(token: &GrammarToken, spec: &str) -> bool {
    // Sudachi includes punctuation, brackets and quotes in 補助記号.
    // The surface check also protects newlines and punctuation marked unknown.
    token.pos != "補助記号"
        && !token
            .surface
            .contains(['\n', '\r', '「', '」', '『', '』', '"', '“', '”'])
        && satisfies(token, spec)
}

fn sequence_end<'a>(
    tokens: &[GrammarToken],
    start: usize,
    pattern: impl Iterator<Item = &'a str>,
) -> Option<usize> {
    let mut end = start;
    for spec in pattern {
        if !tokens.get(end).is_some_and(|token| satisfies(token, spec)) {
            return None;
        }
        end += 1;
    }
    Some(end)
}

const RULES: &[Rule] = &[
    Rule::te_aux("te-iru", &["l:いる&verb"], "l:てる|でる&aux"),
    Rule::te_aux("te-shimau-chau", &["l:しまう&verb"], "l:ちゃう|じゃう&aux"),
    Rule::te_aux("te-oku", &["l:おく&verb"], "l:とく|どく&aux"),
    Rule::suffix(
        "te-miru",
        VERB_TE,
        &["s:み|みて|みる|みよ|みよう&l:みる&verb"],
        true,
    ),
    Rule::suffix("te-kudasai", VERB_TE, &["s:ください"], false),
    Rule::suffix("te-ageru", VERB_TE, &["l:あげる&bound-verb"], true),
    Rule::suffix("te-kureru", VERB_TE, &["l:くれる&bound-verb"], true),
    Rule::suffix("te-morau", VERB_TE, &["l:もらう&bound-verb"], true),
    Rule::suffix(
        "te-kudasaru-te-itadaku",
        VERB_TE,
        &["l:くださる|下さる|いただく&bound-verb"],
        true,
    ),
    Rule::suffix("te-aru", VERB_TE, &["l:ある&bound-verb"], true),
    Rule::suffix(
        "te-hoshii",
        VERB_TE,
        &["l:ほしい|欲しい&bound-adjective"],
        true,
    ),
    Rule::suffix("te-kara", VERB_TE, &["s:から&particle"], false),
    Rule::suffix(
        "te-mo-ii",
        PREDICATE_TE,
        &["s:も&particle", "l:いい|よい&adjective"],
        true,
    ),
    Rule::negative_suffix(
        "te-mo-kamawanai",
        PREDICATE_TE,
        &["s:も&particle", "l:構う|かまう&verb"],
    ),
    Rule::negative_suffix(
        "te-wa-ikenai",
        PREDICATE_TE,
        &["s:は&particle", "s:いけ&verb"],
    ),
    Rule {
        id: "sou-da-appearance",
        pattern: Pattern::Variants(&[&["v:stem|a:stem|a:sa", "s:そう&na"], &["na", "s:そう&na"]]),
        extend_aux: true,
    },
    Rule {
        id: "sou-da-hearsay",
        pattern: Pattern::Variants(&[&["plain", "s:そう&noun", "s:だ|です|でして&aux"]]),
        extend_aux: true,
    },
    Rule {
        id: "tara",
        pattern: Pattern::Variants(&[&["v:stem|a:stem|aux:stem", "s:たら|だら&hyp"]]),
        extend_aux: false,
    },
    Rule {
        id: "ba",
        pattern: Pattern::Variants(&[&["hyp", "s:ば&particle"]]),
        extend_aux: false,
    },
    Rule {
        id: "nara",
        pattern: Pattern::Variants(&[&["noun|na|plain", "s:なら|ならば&l:だ&hyp"]]),
        extend_aux: false,
    },
    Rule {
        id: "nagara",
        pattern: Pattern::Variants(&[&["v:stem", "s:ながら&particle"]]),
        extend_aux: false,
    },
    Rule::suffix("you-ni-suru", VERB_YOU_NI, &["l:する&verb"], true),
    Rule::suffix("you-ni-naru", VERB_YOU_NI, &["l:なる&verb"], true),
    Rule::suffix("koto-ni-suru", VERB_KOTO_NI, &["l:する&verb"], true),
    Rule::suffix("koto-ni-naru", VERB_KOTO_NI, &["l:なる&verb"], true),
    Rule::suffix(
        "koto-ga-dekiru",
        &["v:plain"],
        &["s:こと&noun", "s:が&particle", "l:できる|出来る&bound-verb"],
        true,
    ),
    Rule::suffix(
        "ta-koto-ga-aru",
        VERB_TA,
        &["s:こと&noun", "s:が&particle", "l:ある&bound-verb"],
        true,
    ),
    Rule {
        id: "te-mo-de-mo",
        pattern: Pattern::Variants(&[
            &["v:stem|a:stem", "connector", "s:も&particle"],
            &["na|noun", "s:で&aux", "s:も&particle"],
        ]),
        extend_aux: false,
    },
    Rule {
        id: "wake-de-wa-nai",
        pattern: Pattern::Variants(&[
            &["plain", "s:わけ|訳&noun", "s:で", "s:は", "l:ない"],
            &["plain", "s:わけ|訳&noun", "s:じゃ", "l:ない"],
        ]),
        extend_aux: true,
    },
    Rule {
        id: "nakereba-naranai-nakute-wa-ikenai",
        pattern: Pattern::Variants(&[
            &["v:negative", "s:なけれ", "s:ば", "s:なら", "l:ない"],
            &["v:negative", "s:なく", "s:て", "s:は", "s:いけ", "l:ない"],
            &["v:negative", "s:なくちゃ|なきゃ"],
        ]),
        extend_aux: true,
    },
    Rule {
        id: "nakute-mo-ii",
        pattern: Pattern::Variants(&[&["v:negative", "s:なく", "s:て", "s:も", "l:いい|よい"]]),
        extend_aux: true,
    },
    Rule {
        id: "made-mo-nai",
        pattern: Pattern::Variants(&[&["v:plain", "s:まで&particle", "s:も&particle", "l:ない"]]),
        extend_aux: true,
    },
    Rule {
        id: "ni-mo-kakawarazu",
        pattern: Pattern::Variants(&[&[
            "plain|noun|na",
            "s:に&particle",
            "s:も&particle",
            "l:かかわる|関わる|拘わる&verb",
            "s:ず&aux",
        ]]),
        extend_aux: false,
    },
    Rule {
        id: "zaru-o-enai",
        pattern: Pattern::Variants(&[&[
            "v:negative",
            "s:ざる&aux",
            "s:を&particle",
            "l:得る|える&verb",
            "l:ない&aux",
        ]]),
        extend_aux: true,
    },
    Rule {
        id: "ni-chigai-nai",
        pattern: Pattern::Variants(&[&[
            "plain|noun|na",
            "s:に&particle",
            "s:違い|ちがい&noun",
            "l:ない",
        ]]),
        extend_aux: true,
    },
    Rule {
        id: "kara-reason",
        pattern: Pattern::Variants(&[&["plain", "s:から&particle"]]),
        extend_aux: false,
    },
    Rule::context(
        "mono-no",
        &[],
        &[
            Step::Token("plain"),
            Step::Token("s:もの&noun"),
            Step::Token("s:の&particle"),
        ],
        &[Step::Token("s:、|，")],
        false,
    ),
    Rule::context(
        "shika-nai",
        &[Step::Token("nominal"), Step::Optional(&["case-particle"])],
        &[
            Step::Token("s:しか&particle"),
            ARGUMENT_GAP,
            PREDICATE_GAP,
            Step::Optional(&[
                "connector",
                "l:いる|おく|しまう|みる|ある&nonfinal-predicate",
            ]),
            Step::Alternatives(NEGATIVE_PREDICATE_ENDINGS),
        ],
        &[],
        true,
    ),
    Rule::context(
        "shika-nai-no-choice",
        &[],
        &[
            Step::Token("v:plain"),
            Step::Token("s:しか&particle"),
            Step::Alternatives(&[
                &["l:ない&adjective|aux"],
                &["l:ある&v:stem", "s:ませ&l:ます&aux", "s:ん&l:ぬ&aux"],
            ]),
        ],
        &[],
        true,
    ),
    Rule::context(
        "sae-ba",
        &[Step::Alternatives(&[
            &["nominal", "case-particle"],
            &["nominal|v:stem"],
            VERB_TE,
        ])],
        &[
            Step::Token("s:さえ&particle"),
            ARGUMENT_GAP,
            PREDICATE_GAP,
            Step::Optional(&["connector"]),
            Step::Token("hyp&verb|adjective|aux"),
            Step::Token("s:ば&particle"),
        ],
        &[],
        false,
    ),
    Rule::suffix("te-iku", VERB_TE, &["l:いく|行く&bound-verb"], true),
    Rule::suffix("te-kuru", VERB_TE, &["l:くる|来る&bound-verb"], true),
    Rule::suffix(
        "sugiru",
        &["v:stem|a:root|adjectival-noun"],
        &["l:すぎる|過ぎる&bound-verb"],
        true,
    ),
    Rule::suffix(
        "yasui-nikui",
        VERB_STEM,
        &["l:やすい|易い|にくい&adjectival-suffix"],
        true,
    ),
    Rule::suffix("zurai", VERB_STEM, &["l:づらい&adjectival-suffix"], true),
    Rule::suffix(
        "hajimeru-owaru-tsuzukeru",
        VERB_STEM,
        &["l:始める|はじめる|終わる|おわる|続ける|つづける&verb"],
        true,
    ),
    Rule::suffix("tsumori", &["v:plain|neg:plain"], &["s:つもり&noun"], true),
    Rule::variants(
        "yotei",
        &[
            &["v:plain", "s:予定&noun", "l:だ|です&aux"],
            &["nominal", "s:の&particle", "s:予定&noun", "l:だ|です&aux"],
        ],
        true,
    ),
    Rule::context(
        "hazu-da",
        &[],
        &[
            PREDICATE_NOUN_PREFIX,
            Step::Token("s:はず|筈&noun"),
            Step::Token("l:だ|です&aux"),
        ],
        &[],
        true,
    ),
    Rule::context(
        "hazu-ga-nai",
        &[],
        &[
            PREDICATE_NOUN_PREFIX,
            Step::Token("s:はず|筈&noun"),
            Step::Token("s:が&particle"),
            Step::Token("l:ない&adjective|aux"),
        ],
        &[],
        true,
    ),
    Rule::suffix("ta-bakari", VERB_TA, &["s:ばかり&particle"], true),
    Rule::context(
        "ta-totan-ta-totan-ni",
        &[],
        &[
            Step::Alternatives(&[VERB_TA]),
            Step::Token("s:とたん|途端&noun"),
            Step::Optional(&["s:に&particle"]),
        ],
        &[],
        false,
    ),
    Rule::suffix("naide", VERB_NAI, &["s:で&particle"], false),
    Rule::suffix(
        "naide-kudasai",
        VERB_NAI,
        &["s:で&particle", "s:ください"],
        false,
    ),
    Rule::suffix(
        "zu-ni",
        &["v:negative"],
        &["s:ず&aux", "s:に&particle"],
        false,
    ),
];

pub(super) fn rule_ids() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|rule| rule.id)
}

pub(super) fn find(tokens: &[GrammarToken], focus: (usize, usize)) -> Vec<Match> {
    let mut hits = Vec::new();
    for start in 0..tokens.len() {
        for rule in RULES {
            let Some(mut end) = rule.end(tokens, start) else {
                continue;
            };
            if rule.extend_aux {
                while tokens.get(end).is_some_and(|token| {
                    token.pos == "助動詞"
                        && matches!(
                            token.lemma.as_str(),
                            "た" | "ない" | "だ" | "です" | "ます" | "ぬ"
                        )
                }) {
                    end += 1;
                }
            }
            let start_offset = match rule.pattern {
                Pattern::Context { left, .. } if !left.is_empty() => tokens[start].start,
                _ => predicate_start(tokens, start),
            };
            let span = (start_offset, tokens[end - 1].end);
            if span.0 >= focus.1 || span.1 <= focus.0 {
                continue;
            }
            let form_note = if rule.id == "te-iru" {
                let tail = &tokens[start + 1..end];
                let negative = tail
                    .iter()
                    .any(|token| matches!(token.lemma.as_str(), "ない" | "ぬ"));
                let past = tail.iter().any(|token| token.lemma == "た");
                match (negative, past) {
                    (true, true) => Some("过去否定"),
                    (true, false) => Some("否定"),
                    (false, true) => Some("过去形"),
                    _ if tail
                        .iter()
                        .any(|token| matches!(token.lemma.as_str(), "てる" | "でる")) =>
                    {
                        Some("口语缩略")
                    }
                    _ => None,
                }
            } else {
                None
            };
            hits.push(Match {
                id: rule.id,
                start: span.0,
                end: span.1,
                form_note,
            });
        }
    }
    // Prefer the larger expression when an internal connector belongs to a
    // conventional expression, while preserving genuine nested constructions.
    const NESTED_CONNECTORS: &[(&str, &str)] = &[
        ("nakereba-naranai-nakute-wa-ikenai", "ba"),
        ("sae-ba", "ba"),
        ("nakute-mo-ii", "te-mo-de-mo"),
        ("te-mo-ii", "te-mo-de-mo"),
        ("te-mo-kamawanai", "te-mo-de-mo"),
        ("you-ni-suru", "te-iru"),
        ("naide-kudasai", "naide"),
    ];
    let covered_connectors: Vec<_> = hits
        .iter()
        .filter(|hit| NESTED_CONNECTORS.iter().any(|(outer, _)| *outer == hit.id))
        .map(|hit| (hit.id, hit.start, hit.end))
        .collect();
    hits.retain(|hit| {
        !covered_connectors.iter().any(|&(outer, start, end)| {
            start <= hit.start && end >= hit.end && NESTED_CONNECTORS.contains(&(outer, hit.id))
        })
    });
    hits.sort_by_key(|hit| (hit.start, std::cmp::Reverse(hit.end), hit.id));
    hits.dedup();
    hits
}

fn predicate_start(tokens: &[GrammarToken], start: usize) -> usize {
    if tokens[start].pos != "助動詞" {
        return tokens[start].start;
    }
    let mut cursor = start;
    let copula = tokens[start].lemma == "だ";
    while cursor > 0 && tokens[cursor - 1].pos == "助動詞" {
        cursor -= 1;
    }
    if let Some(mut predicate) = cursor.checked_sub(1) {
        let previous = &tokens[predicate];
        if !previous.unknown
            && (matches!(previous.pos.as_str(), "動詞" | "形容詞")
                || (copula && matches!(previous.pos.as_str(), "形状詞" | "名詞" | "代名詞")))
        {
            // Sudachi A may split a compound verb before a non-independent verb.
            while predicate > 0
                && satisfies(&tokens[predicate], "bound-verb")
                && satisfies(&tokens[predicate - 1], "v:stem")
                && tokens[predicate - 1].end == tokens[predicate].start
            {
                predicate -= 1;
            }
            return tokens[predicate].start;
        }
    }
    tokens[cursor].start
}

fn satisfies(token: &GrammarToken, spec: &str) -> bool {
    !token.unknown
        && spec.split('&').all(|clause| {
            if let Some(excluded) = clause.strip_prefix('!') {
                !matches_condition(token, excluded)
            } else {
                matches_condition(token, clause)
            }
        })
}

fn matches_condition(token: &GrammarToken, conditions: &str) -> bool {
    // For written alternatives, the prefix applies to every choice. An
    // unprefixed group such as v:stem|a:stem is evaluated by predicate.
    if let Some(written) = conditions.strip_prefix("s:") {
        return written.split('|').any(|value| value == token.surface);
    }
    if let Some(lemmas) = conditions.strip_prefix("l:") {
        return lemmas.split('|').any(|value| value == token.lemma);
    }
    conditions.split('|').any(|condition| {
        let plain = token.form.starts_with("終止形") || token.form.starts_with("連体形");
        let stem = token.form.starts_with("連用形");
        match condition {
            "verb" => token.pos == "動詞",
            "adjective" => token.pos == "形容詞",
            "adjectival-noun" => {
                token.pos == "形状詞"
                    || (token.pos == "名詞" && token.pos_detail.ends_with("形状詞可能"))
            }
            "adjectival-suffix" => token.pos == "接尾辞" && token.subpos == "形容詞的",
            "bound-verb" => token.pos == "動詞" && token.subpos == "非自立可能",
            "bound-adjective" => token.pos == "形容詞" && token.subpos == "非自立可能",
            "noun" => matches!(token.pos.as_str(), "名詞" | "代名詞"),
            "nominal" => {
                matches!(token.pos.as_str(), "名詞" | "代名詞")
                    || (token.pos == "接尾辞" && token.subpos == "名詞的")
            }
            "case-particle" => token.pos == "助詞" && token.subpos == "格助詞",
            "argument" => {
                matches!(
                    token.pos.as_str(),
                    "名詞" | "代名詞" | "接尾辞" | "副詞" | "連体詞"
                ) || (token.pos == "助詞"
                    && matches!(token.subpos.as_str(), "格助詞" | "係助詞" | "副助詞"))
            }
            "nonfinal-predicate" => {
                matches!(token.pos.as_str(), "動詞" | "形容詞" | "助動詞")
                    && (stem || token.form.starts_with("未然形"))
            }
            "na" => token.pos == "形状詞",
            "aux" => token.pos == "助動詞",
            "particle" => token.pos == "助詞",
            "connector" => {
                token.pos == "助詞"
                    && token.subpos == "接続助詞"
                    && matches!(token.surface.as_str(), "て" | "で")
            }
            "plain" => plain && matches!(token.pos.as_str(), "動詞" | "形容詞" | "助動詞"),
            "hyp" => token.form.starts_with("仮定形"),
            "v:plain" => token.pos == "動詞" && plain,
            "neg:plain" => token.lemma == "ない" && plain,
            "v:stem" => token.pos == "動詞" && stem,
            "a:stem" => token.pos == "形容詞" && stem,
            "a:root" => token.pos == "形容詞" && token.form.starts_with("語幹"),
            "a:sa" => token.pos == "形容詞" && token.form == "語幹-サ",
            "aux:stem" => token.pos == "助動詞" && stem,
            "v:negative" => token.pos == "動詞" && token.form.starts_with("未然形"),
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contextual_matches_respect_scope_and_exclude_evidence_from_the_span() {
        const RULE: Rule = Rule::context(
            "test",
            &[Step::Token("s:left"), Step::Optional(&["s:optional"])],
            &[
                Step::Token("s:anchor"),
                Step::Gap {
                    through: "s:filler|end|bridge|」",
                    max: 3,
                },
                Step::Token("s:end"),
            ],
            &[Step::Token("s:right")],
            false,
        );
        for (text, expected) in [
            ("left anchor end right", Some(3)),
            ("left optional anchor filler end right", Some(5)),
            ("left anchor end bridge end right", Some(5)),
            ("left anchor end right end right", Some(3)),
            ("left anchor filler filler filler end right", Some(6)),
            ("left anchor filler filler filler filler end right", None),
            ("left anchor filler 」 end right", None),
            ("other anchor end right", None),
            ("left anchor end other", None),
        ] {
            let tokens: Vec<_> = text
                .split_whitespace()
                .enumerate()
                .map(|(index, surface)| GrammarToken {
                    start: index,
                    end: index + 1,
                    surface: surface.into(),
                    lemma: String::new(),
                    pos: String::new(),
                    subpos: String::new(),
                    pos_detail: String::new(),
                    form: String::new(),
                    unknown: false,
                })
                .collect();
            let anchor = tokens
                .iter()
                .position(|token| token.surface == "anchor")
                .unwrap();
            assert_eq!(RULE.end(&tokens, anchor), expected, "{text}");
        }
    }
}
