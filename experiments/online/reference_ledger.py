#!/usr/bin/env python3
"""Join private reference evidence without promoting candidates or display times to gold."""
import argparse
import hashlib
import json
from pathlib import Path


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def overlap(a, b):
    return max(a[0], b[0]) < min(a[1], b[1])


def build_case(case, candidate, lines, decisions):
    start, end = case["start_ms"] / 1000, case["end_ms"] / 1000
    offset = case["context_start_ms"] / 1000
    approvals = [s for d in decisions if d["case_id"] == case["id"]
                 and d.get("source_text_status") == "user_approved"
                 for s in d.get("source_segments", [])]
    masks = [d["conservative_exclusion_media_range_s"] for d in decisions
             if d["case_id"] == case["id"] and "conservative_exclusion_media_range_s" in d]
    cues = []
    for index, segment in enumerate(candidate["result"]["segments"]):
        bounds = [offset + segment["start"], offset + segment["end"]]
        if not overlap(bounds, [start, end]):
            continue
        approved = any(segment["text"] == s["text"] and
                       abs(segment["start"] - s["start"]) < .001 and
                       abs(segment["end"] - s["end"]) < .001 for s in approvals)
        excluded = any(overlap(bounds, mask) for mask in masks)
        cues.append(dict(id=f"{case['id']}-source-{index:04}", text=segment["text"],
                         media_range_s=bounds,
                         core_overlap_s=[max(start, bounds[0]), min(end, bounds[1])],
                         crosses_core_boundary=bounds[0] < start or bounds[1] > end,
                         text_status="uncertain" if excluded else "user_approved" if approved else "model_reference_unreviewed",
                         timing_status="provider_estimate_not_human_verified",
                         precise_latency_eligible=False, excluded=excluded,
                         source_segment_index=index))
    evidence = []
    for line in lines:
        if line["case_id"] != case["id"]:
            continue
        # Strict observed display overlap only: neither a linguistic pairing nor
        # a speech boundary. Preserve one-to-many and many-to-one candidates.
        bounds = [line["first_seen_s"], line["last_seen_s"]]
        ids = [cue["id"] for cue in cues if overlap(cue["media_range_s"], bounds)]
        evidence.append(dict(screen_id=line["id"], source_cue_ids=ids,
                             relation_status="temporal_overlap_candidate_not_verified_translation",
                             language_hint=line["language_hint"], display_range_s=bounds,
                             timing_status=line["timing_status"], text=line["text"],
                             reference_status=line["reference_status"]))
    return dict(case_id=case["id"], core_range_s=[start, end], cues=cues,
                display_evidence=evidence, exclusion_ranges_s=masks,
                unmapped_display_count=sum(not e["source_cue_ids"] for e in evidence))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.dataset
    selection = root / "selection.json"
    adjudications = root / "ocr-20261108/adjudications.json"
    screens = root / "ocr-20261108/verified/screen-lines-v2.json"
    decisions = json.loads(adjudications.read_text())["decisions"]
    lines = json.loads(screens.read_text())
    inputs = [selection, adjudications, screens]
    cases = []
    for case in json.loads(selection.read_text())["cases"]:
        folder = "whisper-no-history" if case["id"] in {"radio-06", "radio-09"} else "whisper-no-history-extra" if case["id"] == "radio-10" else "whisper"
        path = root / "reference-local-20261108" / folder / (case["id"] + ".json")
        inputs.append(path)
        row = build_case(case, json.loads(path.read_text()), lines, decisions)
        row["candidate_path"] = str(path.resolve())
        row["candidate_sha256"] = sha(path)
        cases.append(row)
    result = dict(version=1, cases=cases, input_hashes={str(p): sha(p) for p in inputs},
                  notes=["No candidate is automatically gold; user approval is exact segment scoped.",
                         "Core overlap is an inclusion range, not an interpolated speech time.",
                         "OCR text/display intervals remain separate from ASR speech estimates.",
                         "Chinese or kanji-only language hints are ambiguous; links are N:M temporal candidates.",
                         "Qwen alternative transcripts remain in the paired reference batch, not overwritten."])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({r["case_id"]: dict(cues=len(r["cues"]), approved=sum(c["text_status"] == "user_approved" for c in r["cues"]), uncertain=sum(c["excluded"] for c in r["cues"]), display=len(r["display_evidence"]), unmapped=r["unmapped_display_count"]) for r in cases}, indent=2))


if __name__ == "__main__":
    main()
