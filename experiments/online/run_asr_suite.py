#!/usr/bin/env python3
"""Run a bounded, sequential ASR-only matrix; never start translation or cloud calls."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

from run_asr import file_sha256

ROOT = Path(__file__).resolve().parents[2]


def reusable(previous, command):
    # Exact effective invocation protects against silently reusing another model,
    # worker, upstream checkout, language, step or VAD setting under the same name.
    return previous.get('command', [])[1:] == command[2:]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--audio', type=Path, required=True)
    p.add_argument('--vad-plan', type=Path, required=True)
    p.add_argument('--output-dir', type=Path, required=True)
    p.add_argument('--cpp-model', type=Path, required=True)
    p.add_argument('--max-wall-seconds', type=int, default=180)
    args = p.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    digest = file_sha256(args.audio)
    variants = [(backend, mode, 500, 1) for mode in ['off', 'gate', 'endpoint']
                for backend in ['whisper-cpp', 'whisper-streaming', 'simulstreaming']]
    variants += [('whisper-cpp', 'endpoint', ms, 1) for ms in [250, 800]]
    variants += [('whisper-cpp', 'off', 500, .5)]
    summary = []
    for backend, mode, ms, step in variants:
        prefix = dict(zip(['whisper-cpp', 'whisper-streaming', 'simulstreaming'], ['cpp', 'ws', 'ss']))[backend]
        name = prefix + '-' + mode + (str(ms) if mode == 'endpoint' else '') + ('-step500' if step == .5 else '')
        folder = args.output_dir / name
        python = ROOT / 'experiments/online/envs' / ('whisper-streaming' if backend == 'whisper-streaming' else 'simulstreaming') / '.venv/bin/python'
        command = [str(python), str(ROOT / 'experiments/online/replay_asr.py'), '--backend', backend,
                   '--audio', str(args.audio.resolve()), '--output-dir', str(folder.resolve()),
                   '--step', str(step), '--vad', mode, '--vad-plan', str(args.vad_plan.resolve()),
                   '--silence-ms', str(ms), '--max-wall-seconds', str(args.max_wall_seconds)]
        artifacts = ROOT / 'local-artifacts/online'
        if backend == 'whisper-cpp':
            command += ['--cpp-worker', str(artifacts / 'whisper-cpp-build/infer-worker'), '--model', str(args.cpp_model.resolve())]
        elif backend == 'whisper-streaming':
            command += ['--checkout', str(artifacts / 'upstream/whisper_streaming'), '--model', str(artifacts / 'models/mlx-large-v3')]
        else:
            command += ['--checkout', str(artifacts / 'upstream/SimulStreaming'), '--model', str(artifacts / 'models/large-v3.pt')]
        if folder.exists():
            previous = json.loads((folder / 'run.json').read_text())
            if previous['audio_sha256'] != digest or previous['status'] != 'completed' or not reusable(previous, command):
                raise ValueError('refusing to overwrite or reuse incomplete/different run: ' + name)
            summary.append(dict(run=name, status='reused_completed'))
            print(name + ': reuse completed evidence', flush=True)
            continue
        print(name + ': start', flush=True)
        started = time.monotonic()
        with (args.output_dir / (name + '.log')).open('x') as log:
            process = subprocess.Popen(command, stdout=log, stderr=log, start_new_session=True)
            try:
                code = process.wait(timeout=args.max_wall_seconds + 60)
            except BaseException:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                raise
        result = dict(run=name, exit_code=code, elapsed_s=time.monotonic() - started)
        summary.append(result)
        (args.output_dir / 'suite.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(name + ': ' + ('completed' if code == 0 else 'failed (evidence retained)'), flush=True)
    folders = [args.output_dir / row['run'] for row in summary]
    subprocess.run([sys.executable, str(ROOT / 'experiments/online/asr_metrics.py'), '--runs',
                    *map(str, folders), '--output', str(args.output_dir / 'comparison.json'),
                    '--vad-reference', str(args.vad_plan)], check=True)


if __name__ == '__main__':
    main()
