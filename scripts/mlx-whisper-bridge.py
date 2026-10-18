#!/usr/bin/env python3
"""Local-only mlx-whisper bridge. Dependencies and model weights are explicit.

Called by the shared Rust ASR executor; never downloads a model or reads App DB.
"""
import json
import os
import sys
import time
import uuid
from pathlib import Path


def main():
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    request = json.load(sys.stdin)
    model = Path(request["model"])
    if not model.is_dir():
        raise ValueError("MLX requires an existing local model directory")
    import mlx_whisper
    started = time.perf_counter()
    result = mlx_whisper.transcribe(
        request["audio"], path_or_hf_repo=str(model),
        language=request["language"], word_timestamps=True,
        initial_prompt=request.get("prompt"), verbose=False,
        condition_on_previous_text=request.get("max_context") != 0,
        no_speech_threshold=request["no_speech_threshold"],
    )
    prefix = Path(request["output_prefix"])
    raw = prefix.with_suffix(".json")
    raw.write_text(json.dumps(result, ensure_ascii=False), encoding="utf-8")
    units = []
    for segment in result["segments"]:
        words = segment.get("words") or []
        # Zero-duration punctuation is not evidence for a precise word span.
        # Fall back to the real enclosing segment, retaining all its text.
        evidence = words if words and all(round(w["end"]*1000)>round(w["start"]*1000) for w in words) else [segment]
        for unit in evidence:
            text = unit.get("word", unit.get("text", ""))
            if not text.strip():
                continue
            start, end = round(unit["start"] * 1000), round(unit["end"] * 1000)
            if end <= start:
                raise ValueError("MLX returned a non-positive timing interval")
            units.append({"id": str(uuid.uuid4()), "text": text,
                          "kind": "word" if "word" in unit else "provider_segment",
                          "start_ms": start, "end_ms": end, "timing_source": "model",
                          "provider_confidence": unit.get("probability")})
    prefix.with_suffix(".device").write_text("mlx-metal", encoding="utf-8")
    prefix.with_suffix(".metrics.json").write_text(json.dumps({
        "elapsed_ms": round((time.perf_counter() - started) * 1000),
        "peak_memory_bytes": __import__("mlx.core", fromlist=["get_peak_memory"]).get_peak_memory(),
    }), encoding="utf-8")
    json.dump({"raw_output_path": str(raw), "timed_units": units}, sys.stdout)


if __name__ == "__main__":
    main()
