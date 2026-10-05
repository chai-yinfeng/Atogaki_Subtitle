#!/usr/bin/env python3
"""Cancel owned live-VAD sessions after first inference and verify process-group release."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time
from run_asr import file_sha256

ROOT = Path(__file__).resolve().parents[2]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--case', type=Path, required=True)
    p.add_argument('--frozen-config', type=Path, required=True)
    a = p.parse_args()
    a.root.mkdir(parents=True, exist_ok=False)
    rows = []
    for backend, cfg in json.loads(a.frozen_config.read_text())['candidates'].items():
        target = a.root / backend
        python = ROOT / 'experiments/online/envs' / ('whisper-streaming' if backend == 'whisper-streaming' else 'simulstreaming') / '.venv/bin/python'
        command = [str(python), str(ROOT / 'experiments/online/replay_controlled.py'),
                   '--backend', backend, '--model-root', str(ROOT / 'local-artifacts/online/controlled-models' / cfg['size']),
                   '--case', str(a.case.resolve()), '--decode-ms', str(cfg['interval']), '--vad', 'live',
                   '--output-dir', str(target.resolve())]
        started = time.monotonic()
        observed = False
        with (a.root / (backend + '.log')).open('x') as log:
            proc = subprocess.Popen(command, stdout=log, stderr=log, start_new_session=True)
            try:
                deadline = time.monotonic() + 90
                while proc.poll() is None and time.monotonic() < deadline:
                    events = target / 'events.jsonl'
                    if events.exists() and '"kind": "inference"' in events.read_text():
                        observed = True
                        break
                    time.sleep(.05)
                requested = time.monotonic()
                if proc.poll() is None:
                    # Signal only the owning runner: it must close its own ASR/VAD
                    # children. Group kill would mask faulty cleanup.
                    proc.send_signal(signal.SIGTERM)
                try:
                    code = proc.wait(timeout=15)
                    forced = False
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid, signal.SIGKILL)
                    code = proc.wait()
                    forced = True
                try:
                    os.killpg(proc.pid, 0)
                    released = False
                except ProcessLookupError:
                    released = True
                meta = json.loads((target / 'run.json').read_text()) if (target / 'run.json').exists() else {}
                result = dict(backend=backend, observed_inference=observed, exit_code=code,
                              cleanup_s=time.monotonic()-requested, total_wall_s=time.monotonic()-started,
                              forced_group_kill=forced, process_group_released=released,
                              metadata_status=meta.get('status'), error_type=meta.get('error_type'),
                              run_metadata_sha256=file_sha256(target / 'run.json') if meta else None,
                              passed=observed and not forced and released and meta.get('status') == 'failed_or_cancelled')
                if not released:
                    os.killpg(proc.pid, signal.SIGKILL)
                rows.append(result)
                print(json.dumps(result), flush=True)
            finally:
                if proc.poll() is None:
                    os.killpg(proc.pid, signal.SIGKILL)
                    proc.wait()
        (a.root / 'report.json').write_text(json.dumps(dict(version=3, frozen_config_sha256=file_sha256(a.frozen_config),
                                               cases=rows, passed=all(r['passed'] for r in rows)), indent=2)+'\n')
    if not all(r['passed'] for r in rows):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
