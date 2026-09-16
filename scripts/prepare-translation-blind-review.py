#!/usr/bin/env python3
"""Build a provider-blind subtitle translation review set and separate answer key."""

from __future__ import annotations

import argparse
import hashlib
import json
import random
from pathlib import Path
from typing import Any


def parse_candidate(value: str) -> tuple[str, Path]:
    label, separator, path = value.partition("=")
    if not separator or not label.strip() or not path.strip():
        raise argparse.ArgumentTypeError("candidate must use LABEL=PATH")
    return label.strip(), Path(path)


def load_run(path: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, list):
        segments = payload
        metadata: dict[str, Any] = {}
    elif isinstance(payload, dict) and isinstance(payload.get("segments"), list):
        segments = payload["segments"]
        metadata = {
            key: payload[key]
            for key in ("schema_version", "elapsed_ms", "summary", "options")
            if key in payload
        }
    else:
        raise ValueError(f"{path} must contain a segments array")
    if not segments:
        raise ValueError(f"{path} contains no subtitle segments")
    return segments, metadata


def cue_identity(segment: dict[str, Any]) -> tuple[Any, ...]:
    return (
        segment.get("id"),
        segment.get("start_ms"),
        segment.get("end_ms"),
        segment.get("source_text"),
    )


def deterministic_rng(seed: str, cue_id: str) -> random.Random:
    digest = hashlib.sha256(f"{seed}\0{cue_id}".encode()).digest()
    return random.Random(int.from_bytes(digest[:8], "big"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--candidate",
        action="append",
        required=True,
        type=parse_candidate,
        metavar="LABEL=PATH",
        help="provider label and evaluation JSON; repeat at least twice",
    )
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--answer-key", required=True, type=Path)
    parser.add_argument("--seed", default="atogaki-translation-review-v1")
    args = parser.parse_args()

    if len(args.candidate) < 2:
        parser.error("at least two --candidate inputs are required")
    labels = [label for label, _ in args.candidate]
    if len(set(labels)) != len(labels):
        parser.error("candidate labels must be unique")

    loaded = [(label, path, *load_run(path)) for label, path in args.candidate]
    baseline = loaded[0][2]
    for label, path, segments, _ in loaded[1:]:
        if len(segments) != len(baseline):
            raise ValueError(
                f"{label} ({path}) has {len(segments)} cues; expected {len(baseline)}"
            )
        for index, (expected, actual) in enumerate(zip(baseline, segments, strict=True)):
            if cue_identity(actual) != cue_identity(expected):
                raise ValueError(f"{label} cue {index + 1} does not match the source timeline")

    review_items = []
    answer_items = []
    letters = [chr(ord("A") + index) for index in range(len(loaded))]
    for index, source in enumerate(baseline):
        cue_id = str(source.get("id") or f"cue-{index + 1}")
        order = list(range(len(loaded)))
        deterministic_rng(args.seed, cue_id).shuffle(order)
        candidates = []
        mapping = {}
        for letter, candidate_index in zip(letters, order, strict=True):
            label, _, segments, _ = loaded[candidate_index]
            translation = segments[index].get("translated_text")
            candidates.append({"candidate": letter, "translated_text": translation})
            mapping[letter] = label
        review_items.append(
            {
                "cue_index": index + 1,
                "cue_id": cue_id,
                "start_ms": source.get("start_ms"),
                "end_ms": source.get("end_ms"),
                "source_text": source.get("source_text"),
                "candidates": candidates,
                "review": {
                    "preferred": None,
                    "acceptable": [],
                    "omission_or_added_meaning": [],
                    "term_or_name_error": [],
                    "pronoun_or_reference_error": [],
                    "cross_cue_coherence_error": [],
                    "naturalness_1_to_5": {},
                    "estimated_edit_effort_0_to_3": {},
                    "notes": "",
                },
            }
        )
        answer_items.append({"cue_id": cue_id, "candidates": mapping})

    review = {
        "schema_version": 1,
        "seed": args.seed,
        "instructions": {
            "preferred": "Choose one candidate letter, or null when tied/none are acceptable.",
            "acceptable": "List every candidate that can be used without a meaning correction.",
            "naturalness_1_to_5": "Score each candidate from unusable (1) to natural subtitle Chinese (5).",
            "estimated_edit_effort_0_to_3": "0=no edit, 1=wording, 2=meaning correction, 3=retranslate.",
        },
        "items": review_items,
    }
    answer_key = {
        "schema_version": 1,
        "seed": args.seed,
        "providers": {
            label: {"path": str(path), "metadata": metadata}
            for label, path, _, metadata in loaded
        },
        "items": answer_items,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.answer_key.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(review, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    args.answer_key.write_text(
        json.dumps(answer_key, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"Blind review written to {args.output}")
    print(f"Answer key written to {args.answer_key}")


if __name__ == "__main__":
    main()
