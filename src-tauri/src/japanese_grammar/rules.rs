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
    variants: &'static [&'static [&'static str]],
    extend_aux: bool,
}

const RULES: &[Rule] = &[
    Rule {
        id: "te-iru",
        variants: &[
            &["v:stem", "connector", "l:いる&verb"],
            &["v:stem", "l:てる|でる&aux"],
        ],
        extend_aux: true,
    },
    Rule {
        id: "te-shimau-chau",
        variants: &[
            &["v:stem", "connector", "l:しまう&verb"],
            &["v:stem", "l:ちゃう|じゃう&aux"],
        ],
        extend_aux: true,
    },
    Rule {
        id: "te-oku",
        variants: &[
            &["v:stem", "connector", "l:おく&verb"],
            &["v:stem", "l:とく|どく&aux"],
        ],
        extend_aux: true,
    },
    Rule {
        id: "te-miru",
        variants: &[&[
            "v:stem",
            "connector",
            "s:み|みて|みる|みよ|みよう&l:みる&verb",
        ]],
        extend_aux: true,
    },
    Rule {
        id: "te-kudasai",
        variants: &[&["v:stem", "connector", "s:ください"]],
        extend_aux: false,
    },
    Rule {
        id: "sou-da-appearance",
        variants: &[&["v:stem|a:stem|a:sa", "s:そう&na"], &["na", "s:そう&na"]],
        extend_aux: true,
    },
    Rule {
        id: "sou-da-hearsay",
        variants: &[&["plain", "s:そう&noun", "s:だ|です|でして&aux"]],
        extend_aux: true,
    },
    Rule {
        id: "tara",
        variants: &[&["v:stem|a:stem|aux:stem", "s:たら|だら&hyp"]],
        extend_aux: false,
    },
    Rule {
        id: "ba",
        variants: &[&["hyp", "s:ば&particle"]],
        extend_aux: false,
    },
    Rule {
        id: "nara",
        variants: &[&["noun|na|plain", "s:なら|ならば&l:だ&hyp"]],
        extend_aux: false,
    },
    Rule {
        id: "nagara",
        variants: &[&["v:stem", "s:ながら&particle"]],
        extend_aux: false,
    },
    Rule {
        id: "you-ni-suru",
        variants: &[&["v:plain|neg:plain", "s:よう&na", "s:に", "l:する&verb"]],
        extend_aux: true,
    },
    Rule {
        id: "koto-ni-suru",
        variants: &[&[
            "v:plain|neg:plain",
            "s:こと&noun",
            "s:に&particle",
            "l:する&verb",
        ]],
        extend_aux: true,
    },
    Rule {
        id: "koto-ni-naru",
        variants: &[&[
            "v:plain|neg:plain",
            "s:こと&noun",
            "s:に&particle",
            "l:なる&verb",
        ]],
        extend_aux: true,
    },
    Rule {
        id: "te-mo-de-mo",
        variants: &[
            &["v:stem|a:stem", "connector", "s:も&particle"],
            &["na|noun", "s:で&aux", "s:も&particle"],
        ],
        extend_aux: false,
    },
    Rule {
        id: "wake-de-wa-nai",
        variants: &[
            &["plain", "s:わけ|訳&noun", "s:で", "s:は", "l:ない"],
            &["plain", "s:わけ|訳&noun", "s:じゃ", "l:ない"],
        ],
        extend_aux: true,
    },
    Rule {
        id: "nakereba-naranai-nakute-wa-ikenai",
        variants: &[
            &["v:negative", "s:なけれ", "s:ば", "s:なら", "l:ない"],
            &["v:negative", "s:なく", "s:て", "s:は", "s:いけ", "l:ない"],
            &["v:negative", "s:なくちゃ|なきゃ"],
        ],
        extend_aux: true,
    },
    Rule {
        id: "nakute-mo-ii",
        variants: &[&["v:negative", "s:なく", "s:て", "s:も", "l:いい|よい"]],
        extend_aux: true,
    },
    Rule {
        id: "made-mo-nai",
        variants: &[&["v:plain", "s:まで&particle", "s:も&particle", "l:ない"]],
        extend_aux: true,
    },
    Rule {
        id: "ni-mo-kakawarazu",
        variants: &[&[
            "plain|noun|na",
            "s:に&particle",
            "s:も&particle",
            "l:かかわる|関わる|拘わる&verb",
            "s:ず&aux",
        ]],
        extend_aux: false,
    },
    Rule {
        id: "zaru-o-enai",
        variants: &[&[
            "v:negative",
            "s:ざる&aux",
            "s:を&particle",
            "l:得る|える&verb",
            "l:ない&aux",
        ]],
        extend_aux: true,
    },
    Rule {
        id: "ni-chigai-nai",
        variants: &[&[
            "plain|noun|na",
            "s:に&particle",
            "s:違い|ちがい&noun",
            "l:ない",
        ]],
        extend_aux: true,
    },
    Rule {
        id: "kara-reason",
        variants: &[&["plain", "s:から&particle"]],
        extend_aux: false,
    },
    Rule {
        id: "mono-no",
        variants: &[&["plain", "s:もの&noun", "s:の&particle", "s:、|，"]],
        extend_aux: false,
    },
];

pub(super) fn rule_ids() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|rule| rule.id)
}

pub(super) fn find(tokens: &[GrammarToken], focus: (usize, usize)) -> Vec<Match> {
    let mut hits = Vec::new();
    for start in 0..tokens.len() {
        for rule in RULES {
            let matched = rule
                .variants
                .iter()
                .filter_map(|variant| {
                    let end = start + variant.len();
                    let window = tokens.get(start..end)?;
                    window
                        .iter()
                        .zip(variant.iter())
                        .all(|(token, spec)| satisfies(token, spec))
                        .then_some(end)
                })
                .max();
            let Some(mut end) = matched else { continue };
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
            // A comma can disambiguate a connective such as ものの, but is not part of it.
            let last = if tokens[end - 1].pos == "補助記号" {
                end - 2
            } else {
                end - 1
            };
            let span = (predicate_start(tokens, start), tokens[last].end);
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
    let covered_connectors: Vec<_> = hits
        .iter()
        .filter(|hit| {
            matches!(
                hit.id,
                "nakereba-naranai-nakute-wa-ikenai" | "nakute-mo-ii" | "you-ni-suru"
            )
        })
        .map(|hit| (hit.id, hit.start, hit.end))
        .collect();
    hits.retain(|hit| {
        !covered_connectors.iter().any(|&(outer, start, end)| {
            start <= hit.start
                && end >= hit.end
                && matches!(
                    (outer, hit.id),
                    ("nakereba-naranai-nakute-wa-ikenai", "ba")
                        | ("nakute-mo-ii", "te-mo-de-mo")
                        | ("you-ni-suru", "te-iru")
                )
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
    if let Some(previous) = cursor.checked_sub(1).and_then(|index| tokens.get(index)) {
        if !previous.unknown
            && (matches!(previous.pos.as_str(), "動詞" | "形容詞")
                || (copula && matches!(previous.pos.as_str(), "形状詞" | "名詞" | "代名詞")))
        {
            return previous.start;
        }
    }
    tokens[cursor].start
}

fn satisfies(token: &GrammarToken, spec: &str) -> bool {
    !token.unknown
        && spec.split('&').all(|conditions| {
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
                    "noun" => matches!(token.pos.as_str(), "名詞" | "代名詞"),
                    "na" => token.pos == "形状詞",
                    "aux" => token.pos == "助動詞",
                    "particle" => token.pos == "助詞",
                    "connector" => {
                        token.pos == "助詞" && matches!(token.surface.as_str(), "て" | "で")
                    }
                    "plain" => plain && matches!(token.pos.as_str(), "動詞" | "形容詞" | "助動詞"),
                    "hyp" => token.form.starts_with("仮定形"),
                    "v:plain" => token.pos == "動詞" && plain,
                    "neg:plain" => token.lemma == "ない" && plain,
                    "v:stem" => token.pos == "動詞" && stem,
                    "a:stem" => token.pos == "形容詞" && stem,
                    "a:sa" => token.pos == "形容詞" && token.form == "語幹-サ",
                    "aux:stem" => token.pos == "助動詞" && stem,
                    "v:negative" => token.pos == "動詞" && token.form.starts_with("未然形"),
                    _ => false,
                }
            })
        })
}
