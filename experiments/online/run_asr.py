#!/usr/bin/env python3
"""Capture an upstream computationally-aware file simulation in an isolated run."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import wave

from harness import SCHEMA, normalize, write_events


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['whisper-streaming', 'simulstreaming'], required=True)
    parser.add_argument('--checkout', type=Path, required=True)
    parser.add_argument('--python', type=Path, required=True)
    parser.add_argument('--audio', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--language', default='ja')
    parser.add_argument('--model', default='large-v3')
    parser.add_argument('--chunk-seconds', type=float, default=1)
    args = parser.parse_args()
    if not 0 < args.chunk_seconds <= 60:
        parser.error('chunk-seconds must be in (0, 60]')
    audio = args.audio.resolve()
    with wave.open(str(audio)) as wav:
        if (wav.getframerate(), wav.getnchannels(), wav.getsampwidth()) != (16000, 1, 2):
            parser.error('audio must be 16 kHz mono PCM16 WAV')
        duration = wav.getnframes() / wav.getframerate()
    checkout = args.checkout.resolve()
    commit = subprocess.check_output(['git', '-C', str(checkout), 'rev-parse', 'HEAD'], text=True).strip()
    dirty = subprocess.check_output(['git', '-C', str(checkout), 'status', '--porcelain'], text=True).strip()
    if dirty:
        parser.error('upstream checkout must be clean for reproducibility')
    entry = 'whisper_online.py' if args.backend == 'whisper-streaming' else 'simulstreaming_whisper.py'
    command = [str(args.python.resolve()), str(checkout / entry), str(audio),
               '--language', args.language, '--task', 'transcribe',
               '--min-chunk-size', str(args.chunk_seconds)]
    if args.backend == 'whisper-streaming':
        command += ['--backend', 'mlx-whisper', '--model', args.model]
    else:
        command += ['--model_path', args.model]
    args.output_dir.mkdir(parents=True, exist_ok=False)
    metadata = dict(schema=SCHEMA, backend=args.backend, upstream_commit=commit,
                    audio_sha256=hashlib.sha256(audio.read_bytes()).hexdigest(),
                    audio_duration_s=duration, command=command, status='running',
                    timing_mode='computationally-aware', vad=False)
    metadata_path = args.output_dir / 'run.json'
    metadata_path.write_text(json.dumps(metadata, indent=2) + '\n')
    started = time.monotonic()
    try:
        with (args.output_dir / 'raw.txt').open('x') as stdout, (args.output_dir / 'stderr.log').open('x') as stderr:
            process = subprocess.Popen(command, cwd=checkout, stdout=stdout, stderr=stderr)
            try:
                code = process.wait()
            except BaseException:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                raise
        if code:
            raise RuntimeError(f'upstream exited with {code}; inspect stderr.log')
        with (args.output_dir / 'raw.txt').open() as raw:
            write_events(args.output_dir / 'source.jsonl', normalize(raw, args.backend))
        metadata['status'] = 'completed'
    except BaseException:
        metadata['status'] = 'failed_or_cancelled'
        raise
    finally:
        metadata['wall_time_s'] = time.monotonic() - started
        metadata_path.write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
