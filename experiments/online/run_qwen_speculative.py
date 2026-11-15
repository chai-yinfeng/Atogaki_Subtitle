#!/usr/bin/env python3
"""Compare isolated Python ASR decoding on the explicit confirmed-crop inputs."""
import argparse
from dataclasses import asdict
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import platform
import resource
import sys
import time
import wave

import mlx.core as mx
import numpy as np
from mlx_qwen3_asr import Session
from mlx_qwen3_asr.audio import compute_features
from mlx_qwen3_asr.tokenizer import parse_asr_output

from qwen_speculative import Target, Draft, decode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--input-sha256', required=True)
    parser.add_argument('--model-dir', type=Path, required=True)
    parser.add_argument('--draft-dir', type=Path)
    parser.add_argument('--mode', choices=['original', 'plain', 'dflash', 'dspark'], required=True)
    parser.add_argument('--dtype', choices=['float16', 'bfloat16'], default='bfloat16')
    parser.add_argument('--draft-dtype', choices=['float16', 'bfloat16'], default='bfloat16')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=3)
    args = parser.parse_args()
    if args.repeat < 1 or (args.draft_dir is not None) != (args.mode in ('dflash', 'dspark')):
        parser.error('positive repeats and a draft directory exactly for draft mode are required')
    if hashlib.sha256(args.inputs.read_bytes()).hexdigest() != args.input_sha256:
        parser.error('frozen input hash mismatch')
    if version('mlx-qwen3-asr') != '0.4.4':
        parser.error('this adapter is pinned to mlx-qwen3-asr 0.4.4')
    inputs = json.loads(args.inputs.read_text())
    profile = Path(__file__).parent / 'datasets/japanese-radio/confirmed-text-baseline.json'
    active = json.loads(profile.read_text())['active_segment_ids']
    rows = inputs['rows']
    if [r['id'] for r in rows] != active or any(r['source_status'] != 'user_corrected_semantic_text' for r in rows):
        parser.error('only the exact current confirmed crop list is allowed')
    audio = {}
    for row in rows:
        path = Path(row['audio_path'])
        if hashlib.sha256(path.read_bytes()).hexdigest() != row['audio_sha256']:
            parser.error('crop hash mismatch')
        with wave.open(str(path)) as stream:
            if (stream.getnchannels(), stream.getframerate(), stream.getsampwidth()) != (1, 16000, 2):
                parser.error('expected mono PCM16 at 16 kHz')
            audio[row['id']] = np.frombuffer(stream.readframes(stream.getnframes()), dtype='<i2').astype(np.float32) / 32768.0
    args.output.mkdir(parents=True, exist_ok=False)
    mx.set_cache_limit(512000000)
    before = time.monotonic()
    session = Session(model=str(args.model_dir.resolve()), dtype=getattr(mx, args.dtype))
    target = Target(session.model)
    draft = Draft.load(args.draft_dir, session.model.config.text_config, dtype=getattr(mx, args.draft_dtype)) if args.draft_dir else None
    if draft and draft.algorithm != args.mode:
        parser.error('draft algorithm does not match selected mode')
    mx.synchronize()
    load_seconds = time.monotonic() - before
    weight_hashes = {}
    for directory in [args.model_dir, *([args.draft_dir] if args.draft_dir else [])]:
        for path in directory.glob('*.safetensors'):
            with path.open('rb') as stream:
                weight_hashes[str(path.resolve())] = hashlib.file_digest(stream, 'sha256').hexdigest()
    metadata = dict(python=platform.python_version(), platform=platform.platform(),
                    packages={p: version(p) for p in ['mlx', 'mlx-metal', 'mlx-qwen3-asr', 'numpy']},
                    dtype=args.dtype, mode=args.mode, model_load_seconds=load_seconds,
                    weight_hashes=weight_hashes, draft_compute_dtype=args.draft_dtype if draft else None,
                    confidence_policy='reported raw logits, fixed block, no threshold or adaptive shortening',
                    input_sha256=args.input_sha256, active_ids=active, max_new_tokens=512,
                    language='Japanese', context='', draft_dir=str(args.draft_dir) if draft else None,
                    commands=sys.argv, script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                    adapter_sha256=hashlib.sha256(Path(__file__).with_name('qwen_speculative.py').read_bytes()).hexdigest(),
                    cache_limit_bytes=512000000, streaming=False)
    (args.output / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    results = []
    for repeat in range(args.repeat):
        for row in (rows[::-1] if repeat % 2 else rows):
            mx.synchronize()
            mx.reset_peak_memory()
            before = time.monotonic()
            if args.mode == 'original':
                result = session.transcribe(audio[row['id']], language='Japanese', max_new_tokens=512, return_chunks=True)
                output = dict(text=result.text, finish_reason=result.finish_reason, original=asdict(result))
            else:
                mel, lengths = compute_features(audio[row['id']])
                features, _ = session.model.audio_tower(mel.astype(session.dtype), lengths)
                prompt = session.tokenizer.build_prompt_tokens(n_audio_tokens=features.shape[1], language='Japanese', context='')
                prompt_ids = mx.array([prompt], dtype=mx.int32)
                embeddings = session.model._embed_tokens(prompt_ids)
                embeddings = session.model._inject_audio_features(embeddings, features, prompt_ids == session.model.audio_token_id)
                try:
                    result = decode(target, embeddings, 512, draft)
                except FloatingPointError as error:
                    (args.output / 'failure.json').write_text(json.dumps(dict(
                        id=row['id'], round=repeat, error_type=type(error).__name__, reason=str(error)), indent=2) + '\n')
                    raise
                _, text = parse_asr_output(session.tokenizer.decode(result.tokens), user_language='Japanese')
                output = dict(text=text, **asdict(result))
            mx.synchronize()
            output.update(id=row['id'], round=repeat, seconds=time.monotonic() - before,
                          peak_active_bytes=mx.get_peak_memory(), active_bytes=mx.get_active_memory(),
                          cache_bytes=mx.get_cache_memory(), max_rss_bytes=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
            results.append(output)
            (args.output / 'results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2, allow_nan=False) + '\n')
            print(json.dumps({k: output[k] for k in ['id', 'round', 'seconds', 'finish_reason']}), flush=True)
            if output['finish_reason'] == 'length' or not output['text'].strip():
                raise RuntimeError('ASR empty or token budget exhausted; inspect saved evidence')
            if args.mode != 'original':
                del features, mel, embeddings
            mx.clear_cache()


if __name__ == '__main__':
    main()
