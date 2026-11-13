#!/usr/bin/env python3
"""Freeze shared online-VAD inputs, measure Qwen candidates, and prepare blind review.

Private media/results stay under --study. This is an ASR-only controlled screen,
not a live-caption latency benchmark or automatic semantic-quality ranking.
"""
import argparse
from dataclasses import asdict
import hashlib
import html
from importlib.metadata import version
import json
from pathlib import Path
import platform
import random
import resource
import subprocess
import time
import unicodedata
import wave
from difflib import SequenceMatcher

import _worker_path
from online_worker.pcm_input import AudioRing, PCMInput


CANDIDATES = ["qwen-0.6b", "qwen-0.6b-4bit", "qwen-1.7b-8bit", "qwen-1.7b"]
CASES = ["radio-01", "radio-11"]
SEED = "qwen-radio-screen-20261113-v1"
REPO = Path(__file__).resolve().parents[2]


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(4 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path, value):
    with Path(path).open("x") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write("\n")


def verify(path, digest):
    if sha(path) != digest:
        raise ValueError("frozen input changed: " + str(path))


def read_pcm(path):
    import numpy as np
    with wave.open(str(path)) as source:
        if (source.getnchannels(), source.getsampwidth(), source.getframerate()) != (1, 2, 16000):
            raise ValueError("expected mono PCM16 16kHz")
        return np.frombuffer(source.readframes(source.getnframes()), dtype="<i2")


def write_pcm(path, samples):
    with wave.open(str(path), "wb") as target:
        target.setparams((1, 2, 16000, 0, "NONE", "not compressed"))
        target.writeframes(samples.astype("<i2").tobytes())


def vad_segments(path, vad_path):
    """Freeze worker.py's 512-sample feed, VAD and final-snapshot boundaries."""
    import sherpa_onnx
    config = sherpa_onnx.VadModelConfig()
    config.silero_vad.model = str(vad_path)
    config.silero_vad.threshold = .5
    config.silero_vad.min_silence_duration = .5
    config.silero_vad.min_speech_duration = .25
    config.silero_vad.max_speech_duration = 12
    detector = sherpa_onnx.VoiceActivityDetector(config, buffer_size_in_seconds=60)
    pcm, last_end = AudioRing(), 0

    def closed(eof):
        nonlocal last_end
        while not detector.empty():
            segment = detector.front
            start = max(last_end, segment.start - 3200, 0)
            end = min(segment.start + len(segment.samples), pcm.end)
            if end > start:
                yield dict(start_sample=start, end_sample=end, decision_sample=pcm.end,
                           eof=eof, samples=pcm.snapshot(start, end))
                last_end = end
            detector.pop()

    for chunk in PCMInput(path):
        pcm.append(chunk)
        detector.accept_waveform(chunk)
        yield from closed(False)
    detector.flush()
    yield from closed(True)


def catalog_items():
    return {row["id"]: row for row in json.loads((REPO / "src-tauri/src/model-catalog.json").read_text())}


def prepare(args):
    root = args.study.resolve()
    root.mkdir(parents=True, exist_ok=True)
    if (root / "manifest.json").exists():
        raise FileExistsError("study is already frozen; use a new study directory")
    dataset = args.dataset.resolve()
    selection = dataset / "selection.json"
    ledger_path = dataset / "reference-ledger-20261108.json"
    selection_data = json.loads(selection.read_text())
    ledger = json.loads(ledger_path.read_text())
    write_json(root / "reference-ledger.snapshot.json", ledger)
    rows, cases = [], []
    for case_id in CASES:
        case = next(c for c in selection_data["cases"] if c["id"] == case_id)
        source = Path(case["audio_path"])
        verify(source, case["audio_sha256"])
        verify(Path(case["context_path"]), case["context_sha256"])
        folder = root / "inputs" / case_id
        folder.mkdir(parents=True)
        raw, context = read_pcm(source), read_pcm(case["context_path"])
        write_pcm(folder / "audio.wav", raw)
        case_record = {key: case[key] for key in ["id", "source_id", "start_ms", "end_ms", "topic"]}
        case_record.update(audio_sha256=sha(folder / "audio.wav"), original_audio_sha256=case["audio_sha256"],
                           context_sha256=case["context_sha256"], seconds=len(raw)/16000)
        cases.append(case_record)
        for index, segment in enumerate(vad_segments(source, args.vad)):
            key = f"{case_id}-{index:03d}"
            audio = folder / (key + ".wav")
            context_audio = folder / (key + "-context.wav")
            samples = segment.pop("samples")
            write_pcm(audio, samples * 32768)
            start, end = segment["start_sample"] / 16000, segment["end_sample"] / 16000
            media_start, media_end = start + case["start_ms"]/1000, end + case["start_ms"]/1000
            context_offset = case["context_start_ms"]/1000
            lo = max(0, round((media_start - 5 - context_offset)*16000))
            hi = min(len(context), round((media_end + 5 - context_offset)*16000))
            write_pcm(context_audio, context[lo:hi])
            rows.append(dict(id=key, case_id=case_id, **segment, seconds=end-start,
                             media_range_s=[media_start, media_end],
                             audio_path=str(audio.relative_to(root)), audio_sha256=sha(audio),
                             context_path=str(context_audio.relative_to(root)), context_sha256=sha(context_audio),
                             context_media_range_s=[lo/16000+context_offset, hi/16000+context_offset]))
    catalog = catalog_items()
    models = []
    for candidate in CANDIDATES:
        item = catalog[candidate]
        model = args.original_models / candidate if candidate in ["qwen-0.6b", "qwen-1.7b"] else root / "models" / candidate
        models.append(dict(id=candidate, path=str(model.resolve()), catalog=item))
    sources = [Path(__file__), REPO / "src-tauri/src/model-catalog.json", selection, ledger_path,
               REPO / "src/infrastructure/online_worker/worker.py", REPO / "src/infrastructure/online_worker/pcm_input.py"]
    runtime_package = args.runtime_package
    sources += sorted(runtime_package.glob("*.py"))
    metadata = dict(version=1, seed=SEED, code_base_commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
                    runner_sha256=sha(Path(__file__)), cases=cases, segments=rows, candidates=models,
                    source_hashes={str(p.resolve()): sha(p) for p in sources},
                    vad_sha256=sha(args.vad), vad=dict(threshold=.5, min_silence_s=.5, min_speech_s=.25,
                    max_speech_s=12, pre_roll_s=.2, frame_samples=512),
                    rounds=[CANDIDATES, list(reversed(CANDIDATES))],
                    inference=dict(language="Japanese", context="", activation_dtype="float16", max_new_tokens=None,
                                   timestamps=False, translation=False, draft_model=False, api="Session.transcribe((samples,16000))"),
                    review_policy=dict(disagreements_per_case=8, agreements_per_case=2, include_preapproved_overlaps=True),
                    notes=["6 minutes, two development episodes; not unseen holdout.",
                           "Fixed causal VAD audio per request; human context is never supplied to ASR.",
                           "Two fresh-process rounds in reverse model order; one excluded warmup per process.",
                           "Original BF16 files are loaded with FP16; quantized integer weights retain their format.",
                           "Compute timing excludes acquisition, VAD wait, draft, MT, queueing and UI.",
                           "No automatic CER, semantic judgments, shortlist or quality winner."])
    write_json(root / "manifest.json", metadata)
    print(json.dumps(dict(cases=len(cases), core_seconds=sum(c["seconds"] for c in cases),
                          fixed_segments=len(rows), asr_audio_seconds=sum(r["seconds"] for r in rows))))


def run(args):
    root = args.study.resolve()
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    verify(Path(__file__), manifest["runner_sha256"])
    for source, digest in manifest["source_hashes"].items():
        verify(source, digest)
    candidate = next(c for c in manifest["candidates"] if c["id"] == args.candidate)
    model_path = Path(candidate["path"])
    output = root / "runs" / f"round-{args.round}" / args.candidate
    output.mkdir(parents=True, exist_ok=False)
    for file in candidate["catalog"]["files"]:
        path = model_path / file["name"]
        if path.stat().st_size != file["bytes"]:
            raise ValueError("model file size mismatch: " + file["name"])
        verify(path, file["sha256"])
    inputs = []
    for row in manifest["segments"]:
        path = root / row["audio_path"]
        verify(path, row["audio_sha256"])
        inputs.append((row, path))
    started = time.monotonic()
    import mlx.core as mx
    from mlx_qwen3_asr import Session
    if not mx.metal.is_available():
        raise RuntimeError("Metal unavailable; do not replace this run with CPU results")
    session = Session(model=str(model_path), dtype=mx.float16)
    mx.synchronize()
    model_load_s = time.monotonic() - started
    load_peak = mx.get_peak_memory()
    def infer(path):
        samples = read_pcm(path).astype("float32") / 32768
        before = time.monotonic()
        result = session.transcribe((samples, 16000), language="Japanese")
        mx.synchronize()
        return result, time.monotonic() - before
    warmup, warmup_s = infer(inputs[0][1])
    write_json(output / "warmup.json", dict(seconds=warmup_s, excluded=True, result=asdict(warmup)))
    config = json.loads((model_path / "config.json").read_text())
    metadata = dict(manifest_sha256=sha(manifest_path), candidate=args.candidate, round=args.round,
                    platform=platform.platform(), machine=platform.machine(), python=platform.python_version(),
                    hardware=subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string", "hw.memsize"], text=True).splitlines(),
                    packages={p: version(p) for p in ["mlx", "mlx-metal", "mlx-qwen3-asr", "sherpa-onnx"]},
                    activation_dtype="float16", quantization=config.get("quantization", config.get("quantization_config")),
                    model_load_and_import_s=model_load_s, load_mlx_peak_bytes=load_peak,
                    warmup_seconds=warmup_s, warmup_status="truncated" if warmup.truncated else "empty" if not warmup.text.strip() else "completed",
                    timing="preloaded PCM -> Session.transcribe -> Metal synchronize; no live latency claim")
    write_json(output / "metadata.json", metadata)
    failures = 0
    with (output / "segments.jsonl").open("x") as stream:
        for row, path in inputs:
            try:
                result, compute_s = infer(path)
                status = "truncated" if result.truncated else "empty" if not result.text.strip() else "completed"
                payload = dict(segment_id=row["id"], audio_sha256=row["audio_sha256"], seconds=row["seconds"],
                               status=status, compute_s=compute_s, result=asdict(result))
            except Exception as error:
                payload = dict(segment_id=row["id"], audio_sha256=row["audio_sha256"], status="error", error=repr(error))
            payload.update(mlx_process_peak_bytes=mx.get_peak_memory(),
                           process_max_rss_bytes=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
            failures += payload["status"] != "completed"
            stream.write(json.dumps(payload, ensure_ascii=False) + "\n")
            stream.flush()
            print(json.dumps(dict(candidate=args.candidate, round=args.round, segment_id=row["id"],
                                  status=payload["status"], compute_s=payload.get("compute_s"))), flush=True)
            if payload["status"] == "error":
                break
    write_json(output / "status.json", dict(status="completed" if not failures else "has_failures", failures=failures))


def normalize(text):
    return "".join(c for c in unicodedata.normalize("NFKC", text) if not c.isspace() and not unicodedata.category(c).startswith("P"))


def anonymize(segment_id, candidates):
    order = list(candidates)
    random.Random(hashlib.sha256((SEED + segment_id).encode()).hexdigest()).shuffle(order)
    mapping = {chr(65+i): candidate for i, candidate in enumerate(order)}
    return [{"label": label, **candidates[candidate]} for label, candidate in mapping.items()], mapping


def choose_review(items, approved_ids):
    chosen = set(approved_ids)
    for case in CASES:
        group = [row for row in items if row["case_id"] == case]
        different = sorted((row for row in group if row["disagreement"] > 0), key=lambda row: (-row["disagreement"], row["id"]))
        chosen.update(row["id"] for row in different[:5])
        remaining = [row for row in group if row["id"] not in chosen and row["disagreement"] > 0]
        if remaining:
            chosen.update(remaining[round(i*(len(remaining)-1)/2)]["id"] for i in range(3))
        same = [row for row in group if row["disagreement"] == 0]
        random.Random(SEED+case).shuffle(same)
        chosen.update(row["id"] for row in same[:2])
    return chosen


def report(args):
    root = args.study.resolve()
    manifest_path = root / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    ledger = json.loads((root / "reference-ledger.snapshot.json").read_text())
    evidence = {case["case_id"]: case for case in ledger["cases"]}
    outputs, metadata, metrics = {}, {}, []
    def percentile(values, p):
        if not values:
            return None
        data = sorted(values)
        return data[round((len(data)-1)*p)]
    for round_index, order in enumerate(manifest["rounds"]):
        for candidate in order:
            folder = root / "runs" / f"round-{round_index}" / candidate
            meta = json.loads((folder / "metadata.json").read_text())
            if meta["manifest_sha256"] != sha(manifest_path):
                raise ValueError("run uses different frozen inputs")
            rows = [json.loads(line) for line in (folder / "segments.jsonl").read_text().splitlines()]
            outputs[round_index, candidate] = {row["segment_id"]: row for row in rows}
            metadata[round_index, candidate] = meta
            complete = [row for row in rows if row["status"] == "completed"]
            times = [row["compute_s"] for row in complete]
            metrics.append(dict(round=round_index, candidate=candidate, expected=len(manifest["segments"]), returned=len(rows),
                                completed=len(complete), failures=len(rows)-len(complete),
                                compute_total_s=sum(row.get("compute_s", 0) for row in rows),
                                completed_compute_p50_s=percentile(times, .5), completed_compute_p95_s=percentile(times, .95),
                                model_load_and_import_s=meta["model_load_and_import_s"],
                                peak_mlx_bytes=max((row["mlx_process_peak_bytes"] for row in rows), default=0),
                                peak_process_rss_bytes=max((row["process_max_rss_bytes"] for row in rows), default=0)))
    review_items, keys, approved_ids = [], [], set()
    for segment in manifest["segments"]:
        candidates = {}
        for candidate in CANDIDATES:
            attempts = [outputs[r, candidate].get(segment["id"], {"status": "missing"}) for r in range(2)]
            candidates[candidate] = {"attempts": [{"status": row["status"], "text": row.get("result", {}).get("text", "")} for row in attempts]}
        variants = [normalize(item["attempts"][0]["text"]) for item in candidates.values()]
        disagreement = max(1-SequenceMatcher(None, a, b).ratio() for a in variants for b in variants)
        lo, hi = segment["media_range_s"]
        cues = [cue for cue in evidence[segment["case_id"]]["cues"] if cue["media_range_s"][0] < hi and cue["media_range_s"][1] > lo]
        if any(cue["text_status"] == "user_approved" for cue in cues):
            approved_ids.add(segment["id"])
        displayed = [row for row in evidence[segment["case_id"]]["display_evidence"] if row["display_range_s"][0] < hi and row["display_range_s"][1] > lo]
        anonymous, mapping = anonymize(segment["id"], candidates)
        review_items.append(dict(**segment, candidates=anonymous, disagreement=disagreement,
                                 prior_cues=cues, display_evidence=displayed, review_status="pending"))
        keys.append(dict(segment_id=segment["id"], labels=mapping))
    priority = choose_review(review_items, approved_ids)
    for row in review_items:
        row["priority"] = row["id"] in priority
    write_json(root / "metrics.json", dict(metrics=metrics, semantic_ranking=None, notes=manifest["notes"]))
    write_json(root / "review.json", dict(version=1, manifest_sha256=sha(manifest_path), items=review_items,
                                          notes=["A-D shuffled separately for each item; model identity in separate answer key.",
                                                 "First round text shown; second round retained for repeat disagreement.",
                                                 "Prior references and displayed subtitles are not automatically gold."]))
    write_json(root / "answer-key.json", dict(items=keys))
    html_report(root, review_items)
    print(json.dumps(dict(segments=len(review_items), priority=len(priority), all_agree=sum(r["disagreement"] == 0 for r in review_items),
                          repeat_changes={candidate: sum(outputs[0, candidate].get(s["id"], {}).get("result", {}).get("text") != outputs[1, candidate].get(s["id"], {}).get("result", {}).get("text") for s in manifest["segments"]) for candidate in CANDIDATES})))


def html_report(root, items):
    escape = lambda value: html.escape(str(value), quote=True)
    cards = []
    for row in sorted(items, key=lambda row: (not row["priority"], row["id"])):
        outputs = []
        for candidate in row["candidates"]:
            first, second = candidate["attempts"]
            repeat = f'<details><summary>两次输出不同，查看第二次</summary><p>{escape(second["text"])}</p></details>' if first != second else ""
            outputs.append(f'<div class="candidate"><strong>{candidate["label"]}</strong><p>{escape(first["text"] or "（未输出）")}</p><small>运行状态：{escape(first["status"])}</small>{repeat}<label>判断 <select data-rating="{candidate["label"]}"><option value="pending">待核对</option><option value="acceptable">可接受</option><option value="critical_error">关键事实错误</option><option value="omission">遗漏／不完整</option><option value="uncertain">无法判断</option></select></label></div>')
        references = "".join(f'<p>{escape(cue["text"])} <small>〔{escape(cue["text_status"])}；仅按估计时间重叠关联〕</small></p>' for cue in row["prior_cues"])
        displays = "".join(f'<p>{escape(line["text"])} <small>〔{escape(line["reference_status"])}；画面显示时间，不是发声对齐〕</small></p>' for line in row["display_evidence"])
        cards.append(f'<section data-id="{row["id"]}" data-priority="{str(row["priority"]).lower()}"><h2>{row["id"]} {"· 优先听审" if row["priority"] else ""}</h2><p>原节目 {row["media_range_s"][0]:.2f}–{row["media_range_s"][1]:.2f} 秒。先听本段输入，再按需听前后文。</p><label>模型实际输入 <audio controls preload="none" src="{escape(row["audio_path"])}"></audio></label><label>听审前后文（未输入模型）<audio controls preload="none" src="{escape(row["context_path"])}"></audio></label><div class="candidates">{"".join(outputs)}</div><details><summary>已有参考（辅助核对，不自动作为标准答案）</summary>{references}<h3>画面文字候选</h3>{displays}</details><label>核对后的日语原文（可留空；听不清不猜写）<textarea data-reference></textarea></label><label>备注：否定、人物、专名、漏句或切段问题<textarea data-note></textarea></label></section>')
    page = '''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Qwen 电台候选听审</title><style>body{font:16px/1.6 system-ui,sans-serif;max-width:1100px;margin:auto;padding:24px;background:#f5f3ed;color:#263432}header{position:sticky;top:0;background:#f5f3ed;padding:12px 0;border-bottom:1px solid #bcbfb8;z-index:1}button,select{font:inherit;padding:8px}section{background:white;border:1px solid #ccd0c6;border-radius:10px;padding:20px;margin:20px 0}h2{font-size:20px}.candidates{display:grid;grid-template-columns:1fr 1fr;gap:12px}.candidate{border:1px solid #ddd;padding:12px}.candidate p{white-space:pre-wrap}label{display:block;margin:12px 0}audio{display:block;max-width:100%;width:500px}textarea{display:block;width:98%;min-height:60px;font:inherit}small{color:#656e69}details{margin:12px 0}[hidden]{display:none!important}@media(max-width:700px){.candidates{grid-template-columns:1fr}}</style><header><strong>Qwen 电台候选 · 日语识别听审</strong><p>先核对含义和关键事实，再看措辞。A–D 每段分别打乱，不代表固定模型；同意可多选，无法确认就留待核对。所有判断保存在本页内存，导出后再关闭。</p><label><input id="priority" type="checkbox" checked>只看优先片段</label><button id="export">导出本轮核对 JSON</button></header>'''+"".join(cards)+'''<script>const sections=[...document.querySelectorAll('section')];function filter(){for(const s of sections)s.hidden=document.querySelector('#priority').checked&&s.dataset.priority!=='true'}document.querySelector('#priority').onchange=filter;filter();document.querySelector('#export').onclick=()=>{const data={version:1,reviewer:'user',items:sections.map(s=>({segment_id:s.dataset.id,ratings:Object.fromEntries([...s.querySelectorAll('[data-rating]')].map(e=>[e.dataset.rating,e.value])),source_text:s.querySelector('[data-reference]').value,note:s.querySelector('[data-note]').value}))};const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([JSON.stringify(data,null,2)],{type:'application/json'}));a.download='qwen-radio-user-review.json';a.click();setTimeout(()=>URL.revokeObjectURL(a.href),1000)}</script></html>'''
    with (root / "review.html").open("x") as stream:
        stream.write(page)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--study", type=Path, required=True)
    commands = parser.add_subparsers(dest="command", required=True)
    setup = commands.add_parser("prepare")
    setup.add_argument("--dataset", type=Path, required=True)
    setup.add_argument("--original-models", type=Path, required=True)
    setup.add_argument("--vad", type=Path, required=True)
    setup.add_argument("--runtime-package", type=Path, required=True)
    runner = commands.add_parser("run")
    runner.add_argument("--candidate", choices=CANDIDATES, required=True)
    runner.add_argument("--round", type=int, choices=[0, 1], required=True)
    commands.add_parser("report")
    args = parser.parse_args()
    {"prepare": prepare, "run": run, "report": report}[args.command](args)


if __name__ == "__main__":
    main()
