#!/usr/bin/env python3
"""Audit saved Whisper evidence against published cues without running a model.

The report is structural evidence, not a transcription accuracy score. Keep its
output in a private evaluation directory. Existing reports are never overwritten.
"""
import argparse
import hashlib
import json
from pathlib import Path


def read(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def audit(job):
    root = job / "asr-checkpoint"
    plan = read(root / "plan.json")
    units = read(root / "merged-units.json")
    candidates = read(root / "final-cues.json")
    published = read(job / "segments.json")
    fields = ("id", "start_ms", "end_ms", "source_text")
    project = lambda cue: {key: cue[key] for key in fields}
    candidate_rows = [project(cue) for cue in candidates["cues"]]
    published_rows = [project(cue) for cue in published]
    unit_rows = [dict(id=u["id"], start_ms=u["start_ms"], end_ms=u["end_ms"],
                      source_text=u["text"]) for u in units]
    merged_ids = {unit["id"] for unit in units}
    chunks = []
    seam_review = []
    for chunk in plan["chunks"]:
        directory = root / f'chunk-{chunk["index"]:06}'
        evidence = read(directory / "evidence.json")
        raw_path = directory / "provider-output.json"
        raw = read(raw_path)
        # Ownership is reported separately from publication. Context observations
        # omitted by ownership are not automatically classified as lost speech.
        owned = [u for u in evidence["units"]
                 if chunk["start_ms"] <= (u["start_ms"] + u["end_ms"]) // 2
                 < chunk["end_ms"]]
        for boundary in (chunk["start_ms"], chunk["end_ms"]):
            if boundary in (0, plan["duration_ms"]):
                continue
            crossing = [u for u in evidence["units"]
                        if u["start_ms"] < boundary < u["end_ms"]]
            for unit in crossing:
                if unit["id"] not in merged_ids:
                    seam_review.append({
                        "boundary_ms": boundary, "chunk_index": chunk["index"],
                        "unit": unit,
                        "reason": "Crossing observation absent from merged evidence; compare both sides and audio",
                    })
        chunks.append({
            "chunk": chunk,
            "evidence_checksum_ok": sha(directory / "evidence.json")
            == (directory / "sha256").read_text().strip(),
            "raw_checksum_ok": sha(raw_path) == evidence["raw_sha256"],
            "raw_nonempty_segments": sum(bool(u["text"].strip())
                                         for u in raw["transcription"]),
            "mapped_segments": len(evidence["units"]),
            "excluded_overrun_segments": len(evidence.get("excluded_right_context_units", [])),
            "owned_segments": len(owned),
            "owned_ids_absent_from_merged": [u["id"] for u in owned if u["id"] not in merged_ids],
            "device": evidence["device"],
        })
    return {
        "schema_version": 1,
        "job": str(job),
        "strategy": plan["strategy"],
        "segmentation_policy": candidates["segmentation_policy"],
        "duration_ms": plan["duration_ms"],
        "chunks": chunks,
        "unselected_crossing_observations": seam_review,
        "merged_units": len(units), "candidate_cues": len(candidate_rows),
        "published_cues": len(published_rows),
        "merged_ids_unique": len(merged_ids) == len(units),
        "merged_equals_candidates": unit_rows == candidate_rows,
        "candidates_equal_publication": candidate_rows == published_rows,
        "subsecond_cues": sum(c["end_ms"] - c["start_ms"] < 1000 for c in published_rows),
        "limits": ["No reference transcript or human listening accuracy measurement",
                   "Publication equality does not prove ownership selection retained all speech",
                   "Unselected crossing observations are review candidates, not proof of missing speech by themselves",
                   "Owned observations can be removed by cross-chunk duplicate handling; review absent IDs",
                   "Exact evidence-to-cue equality is expected only for unchanged, glossary-free Whisper output"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("job", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = audit(args.job.resolve())
    # Exclusive creation also protects reports when two audits run concurrently.
    with args.output.open("x") as stream:
        json.dump(report, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({key: value for key, value in report.items()
                      if key not in ("chunks", "limits", "unselected_crossing_observations")}, ensure_ascii=False))


if __name__ == "__main__":
    main()
