#!/usr/bin/env python3
"""Compile reviewed OpenJLPT formations into Atogaki's Sudachi predicates."""

import argparse
from collections import Counter
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "assets/japanese-grammar/openjlpt"
OUTPUT = ROOT / "src-tauri/src/japanese_grammar/generated_rules.rs"

# Participation is separate from parsing. These senses were reviewed against
# the meaning of the card and actual Sudachi tokens, not just their spelling.
ENABLED = frozenset("""
    n-desu-no-desu node kedo-keredomo to-omou kamo-shirenai kana-kashira
    deshou darou shi tai koto-ga-aru made-ni hou-ga-ii
    mitai rashii you-da ga-but nado mae-ni ato-de you-to-omou dake bakari
""".split())
EXTEND_AUX = frozenset("""
    to-omou tai koto-ga-aru hou-ga-ii mitai rashii you-da you-to-omou
""".split())

HOSTS = {
    "Verb-dict": ("v:plain",),
    "Verb-plain": ("v:predicate",),
    "Verb-ta": ("v:stem", "l:た&plain"),
    "Verb-te": ("v:stem", "connector"),
    "Verb-nai": ("v:negative", "l:ない&aux"),
    "Verb-stem": ("v:stem",),
    "Verb-volitional": ("volitional-verb",),
    "i-Adj": ("a:plain",),
    "i-Adj-ku": ("a:stem",),
    "na-Adj": ("adjectival-noun",),
    "Noun": ("nominal",),
    "Sentence": ("plain",),
}

# Convert a grammatical marker into Sudachi predicates. Alternatives here are
# equivalent forms within the same construction, with constraints on POS.
EXPLANATORY = ("s:ん|の&nominalizer", "s:です|だ&aux")
KAMO = ("s:か&adverbial-particle", "s:も&particle")
KANA = [("s:か&end-particle", "s:な&end-particle"), ("s:かな&end-particle",)]
QUOTED_THOUGHT = ("s:と&case-particle", "l:思う|おもう&verb")
LITERALS = {
    "な": [("s:な&l:だ&aux",)],
    "だ": [("s:だ&l:だ&aux",)],
    "の": [("s:の&particle",)],
    "んです": [EXPLANATORY],
    "ので": [("s:の&nominalizer", "s:で&l:だ&aux")],
    "けど": [("s:けど&clause-particle",)],
    "けれども": [
        ("s:けれども&clause-particle",),
        ("s:けれど&clause-particle", "s:も&particle"),
        ("s:けれど&clause-particle",),
    ],
    "と思う": [
        QUOTED_THOUGHT,
        (*QUOTED_THOUGHT, "connector"),
        (*QUOTED_THOUGHT, "connector", "l:いる&bound-verb"),
        (*QUOTED_THOUGHT, "l:てる|でる&aux"),
    ],
    "かもしれない": [
        (*KAMO, "l:しれる|知れる&verb", "l:ない&aux"),
        (*KAMO, "l:しれる|知れる&verb", "s:ませ&l:ます&aux", "s:ん&l:ぬ&aux"),
    ],
    "かな": [*KANA, *(("s:の&nominalizer", *tail) for tail in KANA)],
    "かしら": [("s:かしら&end-particle",)],
    "でしょう": [("s:でしょう|でしょ&l:です&aux",)],
    "だろう": [("s:だろう|だろ&l:だ&aux",)],
    "し": [("s:し&clause-particle",)],
    "たい": [("l:たい&aux",)],
    "ことがある": [("s:こと&noun", "s:が&case-particle", "l:ある&bound-verb")],
    "までに": [("s:まで&particle", "s:に&case-particle")],
    "ほうがいい": [("s:ほう|方&noun", "s:が&case-particle", "l:いい|よい&adjective")],
    "について": [("s:に&case-particle", "l:つく|就く&verb", "connector")],
    "にとって": [("s:に&case-particle", "l:とる|取る&verb", "connector")],
    "みたい": [("s:みたい&adjectival-noun",)],
    "らしい": [("l:らしい&aux",)],
    "ようだ": [("s:よう&adjectival-noun", "l:だ|です&aux")],
    "が": [("s:が&clause-particle",)],
    "など": [("s:など&adverbial-particle",)],
    "前に": [("s:前|まえ&noun", "s:に&case-particle")],
    "後で": [("s:後|あと&noun", "s:で&case-particle")],
    "だけ": [("s:だけ&adverbial-particle",)],
    "ばかり": [("s:ばかり&adverbial-particle",)],
    "である": [("s:で&l:だ&aux", "l:ある&bound-verb")],
    "いる": [("l:いる&bound-verb",)],
    "に": [("s:に&case-particle",)],
    "と": [("s:と&particle",)],
    "から": [("s:から&particle",)],
    "も": [("s:も&particle",)],
    "て": [("connector",)],
    "で": [("s:で&particle",)],
    "を": [("s:を&case-particle",)],
    "は": [("s:は&particle",)],
}

# Some published formations need reviewed corrections for reading or scope.
# E.g. adjective/noun + らしい also has a different "typical of" sense.
FORMATION_OVERRIDES = {
    "rashii": "Verb-plain / i-Adj + らしい",
}
EXCLUDED_FOLLOWING = {
    # のでは and のでも also have nominalized or hypothetical uses.
    "node": "s:は|も&particle",
}

def branches(formation):
    """Expand slash groups with a shared trailing anchor or leading host."""
    waiting = []
    previous = None
    for text in formation.split(" / "):
        parts = tuple(text.split(" + "))
        if len(parts) == 1 and text in HOSTS:
            waiting.append(text)
            continue
        if len(parts) == 1 and text in LITERALS and previous and not waiting:
            for prefix in previous:
                yield (*prefix, text)
            continue
        if len(parts) < 2 or parts[0] not in HOSTS:
            raise ValueError(f"unsupported branch {text!r}")
        hosts = (*waiting, parts[0])
        waiting.clear()
        previous = [(host, *parts[1:-1]) for host in hosts]
        for host in hosts:
            yield (host, *parts[1:])
    if waiting:
        raise ValueError(f"no grammatical anchor for {waiting!r}")


def compile_formation(formation):
    variants = {}
    for branch in branches(formation):
        variants.update(dict.fromkeys(compile_branch(branch)))
    if not variants:
        raise ValueError("no compiled variants")
    return list(variants)


def compile_branch(branch):
    sequences = [()]
    for part in branch:
        choices = [HOSTS[part]] if part in HOSTS else LITERALS.get(part)
        if not choices:
            raise ValueError(f"unmapped marker {part!r}")
        sequences = [prefix + choice for prefix in sequences for choice in choices]
    return sequences


def render(rows):
    lines = [
        "// @generated by scripts/generate-japanese-grammar-rules.py. Do not edit.",
        "&[",
    ]
    for row in sorted(rows, key=lambda row: row["id"]):
        rule_id = row["id"]
        formation = FORMATION_OVERRIDES.get(rule_id, row["formation"])
        try:
            variants = compile_formation(formation)
        except ValueError as error:
            raise SystemExit(f"{rule_id}: {error}") from error
        clause_final = []
        if rule_id == "kamo-shirenai":
            # Bare かも requires an utterance boundary, unlike the full form.
            clause_final = [(*HOSTS[host], *KAMO) for host in ("Verb-plain", "i-Adj", "na-Adj", "Noun")]
        excluded = json.dumps(EXCLUDED_FOLLOWING.get(rule_id, ""), ensure_ascii=False)
        suffix = "true" if rule_id in EXTEND_AUX else "false"
        lines.extend(["    Rule::compiled(", f'        "{rule_id}",'])
        for sequences in (variants, clause_final):
            if not sequences:
                lines.append("        &[],")
                continue
            lines.append("        &[")
            for seq in sequences:
                tokens = ", ".join(json.dumps(token, ensure_ascii=False) for token in seq)
                lines.append(f"            &[{tokens}],")
            lines.append("        ],")
        lines.extend([f"        {excluded},", f"        {suffix},", "    ),"])
    return "\n".join([*lines, "]", ""])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="check that the Rust output is current")
    mode.add_argument("--audit", action="store_true", help="summarize the current compiler's catalog vocabulary")
    args = parser.parse_args()
    rows = [row for path in sorted(CATALOG.glob("n?.json")) for row in json.loads(path.read_text())]
    by_id = {row["id"]: row for row in rows}
    if len(by_id) != len(rows) or ENABLED - by_id.keys():
        raise SystemExit("duplicate or missing OpenJLPT IDs")
    output = render([by_id[rule_id] for rule_id in ENABLED])
    if args.audit:
        reasons = Counter()
        candidates = []
        for row in rows:
            if row["id"] in ENABLED:
                continue
            try:
                compile_formation(row["formation"])
            except ValueError as error:
                reasons[str(error)] += 1
            else:
                candidates.append(row["id"])
        print(f"{len(rows)} catalog entries; {len(ENABLED)} enabled; {len(candidates)} additional parseable")
        print("Additional parseable entries need review:", ", ".join(candidates) or "none")
        for reason, count in reasons.most_common(12):
            print(f"{count:>3}  {reason}")
    elif args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != output:
            raise SystemExit("generated rule output is stale; rerun this script")
    else:
        OUTPUT.write_text(output)


if __name__ == "__main__":
    main()
