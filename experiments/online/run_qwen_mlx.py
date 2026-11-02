#!/usr/bin/env python3
"""Run local whole-segment Qwen MLX ASR, optionally followed by local Hy-MT2."""
import argparse
from dataclasses import asdict
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import platform
import resource
import time
import wave

from harness import HyMT2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', type=Path, required=True)
    parser.add_argument('--audio', type=Path, action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=2)
    parser.add_argument('--translation-url', help='Optional loopback llama-server /v1 URL')
    args = parser.parse_args()
    if args.repeat < 1:
        parser.error('--repeat must be positive')
    # Refuse to overwrite evidence from a previous invocation.
    args.output.mkdir(parents=True, exist_ok=False)
    inputs = []
    for path in args.audio:
        with wave.open(str(path)) as audio:
            duration = audio.getnframes() / audio.getframerate()
        inputs.append(dict(path=str(path.resolve()), seconds=duration,
                           sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    started = time.monotonic()
    import mlx.core as mx
    from mlx_qwen3_asr import Session
    session = Session(model=str(args.model_dir.resolve()), dtype=mx.float16)
    mx.synchronize()
    load_s = time.monotonic() - started
    metadata = dict(model_dir=str(args.model_dir.resolve()), dtype='float16',
                    model_load_and_import_s=load_s, python=platform.python_version(),
                    platform=platform.platform(),
                    packages={p: version(p) for p in ['mlx', 'mlx-metal', 'mlx-qwen3-asr']},
                    inputs=inputs, language='Japanese', context='',
                    timestamps=False, streaming=False,
                    timing='complete WAV ready -> complete ASR -> optional complete local translation',
                    exclusions=['audio acquisition', 'VAD endpoint wait', 'first-pass ASR',
                                'model downloads', 'translation server startup'])
    (args.output / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    translator = (HyMT2(args.translation_url, 'Hy-MT2-1.8B-Q4_K_M', 'Simplified Chinese')
                  if args.translation_url else None)
    rows = []
    for repeat in range(args.repeat):
        for index, item in enumerate(inputs):
            before = time.monotonic()
            result = session.transcribe(item['path'], language='Japanese', return_chunks=True)
            mx.synchronize()
            asr_done = time.monotonic()
            row = dict(repeat=repeat, input_index=index, audio_seconds=item['seconds'],
                       asr_seconds=asr_done - before,
                       asr_result=asdict(result), status='completed')
            # Persist ASR even if the optional downstream request fails.
            (args.output / f'{repeat}-{index}-asr.json').write_text(
                json.dumps(row, ensure_ascii=False, indent=2) + '\n')
            if result.truncated or not result.text.strip():
                raise RuntimeError('ASR was truncated or empty; inspect saved result')
            if translator:
                row['translation'] = translator.translate(result.text)
                finished = time.monotonic()
                row.update(translation_seconds=finished - asr_done,
                           audio_ready_to_translation_seconds=finished - before)
            row.update(asr_rtf=row['asr_seconds'] / item['seconds'],
                       mlx_peak_bytes=mx.get_peak_memory(),
                       process_max_rss_bytes=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
            rows.append(row)
            (args.output / 'results.json').write_text(
                json.dumps(rows, ensure_ascii=False, indent=2) + '\n')
            print(json.dumps({k: v for k, v in row.items()
                              if k not in ('asr_result', 'translation')}), flush=True)
    print('Saved transcripts and timings:', args.output, flush=True)


if __name__ == '__main__':
    main()
