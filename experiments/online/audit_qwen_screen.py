#!/usr/bin/env python3
"""Audit a completed screen and derive a small follow-up listening batch.

The original frozen 22-item selection remains unchanged. The follow-up includes
every observed 1.7B quantization disagreement, every failed request, and agreement
controls. It is diagnostic sampling, never an estimate of population accuracy.
"""
import argparse
from collections import Counter
import json
from pathlib import Path
import random
import re

from screen_qwen_candidates import CASES, CANDIDATES, SEED, normalize, sha, verify, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--study", type=Path, required=True)
    args = parser.parse_args()
    root = args.study.resolve()
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    expected = {row["id"]: row for row in manifest["segments"]}
    for case in manifest["cases"]:
        verify(root / "inputs" / case["id"] / "audio.wav", case["audio_sha256"])
    for segment in expected.values():
        verify(root / segment["audio_path"], segment["audio_sha256"])
        verify(root / segment["context_path"], segment["context_sha256"])
    inputs = {str(manifest_path): sha(manifest_path)}
    runs, failures = {}, []
    for round_index in range(2):
        for candidate in CANDIDATES:
            folder = root / "runs" / f"round-{round_index}" / candidate
            metadata_path, rows_path = folder / "metadata.json", folder / "segments.jsonl"
            if json.loads(metadata_path.read_text())["manifest_sha256"] != sha(manifest_path):
                raise ValueError("run manifest differs")
            rows = [json.loads(line) for line in rows_path.read_text().splitlines()]
            if len(rows) != len(expected) or Counter(row["segment_id"] for row in rows) != Counter(expected.keys()):
                raise ValueError("missing/duplicate/unexpected segment")
            for row in rows:
                if row["audio_sha256"] != expected[row["segment_id"]]["audio_sha256"]:
                    raise ValueError("run audio differs")
                if row["status"] != "completed":
                    failures.append(dict(candidate=candidate, round=round_index,
                                         segment_id=row["segment_id"], status=row["status"]))
            runs[round_index, candidate] = {row["segment_id"]: row for row in rows}
            inputs[str(metadata_path)] = sha(metadata_path)
            inputs[str(rows_path)] = sha(rows_path)
    first, second = runs[0, "qwen-1.7b"], runs[0, "qwen-1.7b-8bit"]
    different = {key for key in expected if normalize(first[key]["result"]["text"]) != normalize(second[key]["result"]["text"])}
    focus = different | {row["segment_id"] for row in failures}
    review = json.loads((root / "review.json").read_text())
    for case_id in CASES:
        controls = [row["id"] for row in review["items"] if row["case_id"] == case_id
                    and row["disagreement"] == 0 and row["id"] not in focus]
        random.Random(SEED + case_id).shuffle(controls)
        focus.update(controls[:1])
    source = root / "review-with-medium.html"
    page = source.read_text()
    def keep(match):
        return match.group(0).replace('data-priority="false"', 'data-priority="true"') if match.group(1) in focus else ""
    page = re.sub(r'<section data-id="([^"]+)"[^>]*>.*?</section>', keep, page, flags=re.S)
    explanation = (f"<p>首批 {len(focus)} 段：包含全部 {len(different)} 段 1.7B 未量化／8bit 分歧、"
                   "失败请求和每集一个模型一致的抽查。原始完整材料与其 22 段优先选择保留；"
                   "本批不能估计总体错误率。若一个词在切段边界外才说完整，不把模型没有听到的部分直接判成漏词。</p>")
    explanation += '<details><summary>需要更长上下文：回听完整 3 分钟片段</summary>'
    for case in manifest["cases"]:
        explanation += (f'<label>{case["id"]}（播放 0 秒对应原节目 {case["start_ms"]/1000:g} 秒）'
                        f'<audio controls preload="none" src="inputs/{case["id"]}/audio.wav"></audio></label>')
    explanation += '</details>'
    page = page.replace("</header>", explanation + "</header>", 1)
    output = root / "review-first.html"
    with output.open("x") as stream:
        stream.write(page)
    write_json(root / "audit.json", dict(version=1, input_hashes=inputs, failures=failures,
               request_count=len(expected)*len(CANDIDATES)*2, complete_request_count=len(expected)*len(CANDIDATES)*2-len(failures),
               quantization_disagreement_ids=sorted(different), first_review_ids=sorted(focus),
               full_review_sha256=sha(source), first_review_sha256=sha(output),
               auditor_sha256=sha(Path(__file__)), semantic_winner=None,
               notes=["Normalized output disagreement is not CER or semantic accuracy.",
                      "The small follow-up batch is selected after observing outputs; not unseen validation."]))
    print(json.dumps(dict(requests=len(expected)*8, failures=len(failures),
                          quantization_disagreements=len(different), first_review=len(focus))))


if __name__ == "__main__":
    main()
