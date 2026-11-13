#!/usr/bin/env python3
"""Add the installed desktop's Medium as a labeled auxiliary reference.

Uses the same frozen PCM requests, one process, no cross-request text context.
Its beam/fallback defaults differ from Qwen; do not include it in Qwen timing ranks.
"""
import argparse
import html
import json
from pathlib import Path
import subprocess
import time

from screen_qwen_candidates import catalog_items, sha, verify, write_json


def render_reference(root):
    manifest_path = root / "manifest.json"
    request_path = root / "whisper-medium-reference/request.json"
    request = json.loads(request_path.read_text())
    if request["manifest_sha256"] != sha(manifest_path):
        raise ValueError("Whisper reference uses different frozen inputs")
    manifest = json.loads(manifest_path.read_text())
    expected = {row["id"]: row["audio_sha256"] for row in manifest["segments"]}
    result_path = root / "whisper-medium-reference/results.json"
    rows = json.loads(result_path.read_text())["rows"]
    if len(rows) != len(expected) or {row["segment_id"] for row in rows} != set(expected):
        raise ValueError("Whisper reference segment set differs")
    page_path = root / "review.html"
    page = page_path.read_text()
    for row in rows:
        if row["audio_sha256"] != expected[row["segment_id"]]:
            raise ValueError("Whisper reference audio differs")
        raw = root / "whisper-medium-reference" / (row["segment_id"] + ".json")
        if row["raw_sha256"]:
            verify(raw, row["raw_sha256"])
        start = page.index(f'<section data-id="{row["segment_id"]}"')
        end = page.index("</section>", start)
        addition = ('<details><summary>Whisper Medium 辅助参考（已知模型，不参与 A–D 匿名选择）</summary>'
                    f'<p>{html.escape(row["text"])}</p><small>状态：{html.escape(row["status"])}。'
                    '同一段输入，使用桌面 beam 5／fallback 设置；不是人工标准答案。</small></details>')
        page = page[:end] + addition + page[end:]
    with (root / "review-with-medium.html").open("x") as stream:
        stream.write(page)
    write_json(root / "review-with-medium.provenance.json", dict(
        original_review_sha256=sha(page_path), whisper_request_sha256=sha(request_path),
        whisper_results_sha256=sha(result_path), rendered_sha256=sha(root / "review-with-medium.html"),
        renderer_sha256=sha(Path(__file__))))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--study", type=Path, required=True)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--model", type=Path)
    parser.add_argument("--render-only", action="store_true")
    args = parser.parse_args()
    root = args.study.resolve()
    if args.render_only:
        render_reference(root)
        return
    if not args.cli or not args.model:
        parser.error("--cli and --model are required for inference")
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    verify(args.model, catalog_items()["whisper-medium"]["files"][0]["sha256"])
    output = root / "whisper-medium-reference"
    output.mkdir(exist_ok=False)
    command = [str(args.cli), "-m", str(args.model), "-l", "ja", "-sns", "-mc", "0",
               "-bo", "5", "-bs", "5", "-tp", "0", "-tpi", "0.2", "--device", "0", "-oj"]
    for row in manifest["segments"]:
        audio = root / row["audio_path"]
        verify(audio, row["audio_sha256"])
        command += ["-f", str(audio), "-of", str(output / row["id"])]
    write_json(output / "request.json", dict(command=command, manifest_sha256=sha(manifest_path),
               cli_sha256=sha(args.cli), model_sha256=sha(args.model), prompt="", context_tokens=0,
               role="labeled auxiliary reference, not semantic gold or an online candidate",
               note="Desktop-compatible beam 5 / best-of 5 / fallback; not Qwen timing-equivalent."))
    before = time.monotonic()
    with (output / "process.log").open("x") as stream:
        process = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
    rows = []
    for row in manifest["segments"]:
        path = output / (row["id"] + ".json")
        raw = json.loads(path.read_text()) if path.exists() else None
        text = "".join(item["text"] for item in raw["transcription"]) if raw else ""
        rows.append(dict(segment_id=row["id"], audio_sha256=row["audio_sha256"], text=text,
                         status="completed" if text.strip() else "empty" if raw else "missing",
                         raw_sha256=sha(path) if raw else None))
    write_json(output / "results.json", dict(status="completed" if process.returncode == 0 else "failed",
               returncode=process.returncode, wall_s=time.monotonic()-before, rows=rows))
    print(json.dumps(dict(returncode=process.returncode, expected=len(rows),
                          completed=sum(row["status"] == "completed" for row in rows))))
    render_reference(root)


if __name__ == "__main__":
    main()
