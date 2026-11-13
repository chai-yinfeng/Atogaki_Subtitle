#!/usr/bin/env python3
"""Prepare pinned upstream checkouts and independent uv environments."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    root = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['whisper-streaming', 'simulstreaming', 'all'], default='all')
    parser.add_argument('--upstream-root', type=Path, default=root.parents[1] / 'local-artifacts/online/upstream')
    args = parser.parse_args()
    pins = json.loads((root / 'upstream-pins.json').read_text())
    args.upstream_root.mkdir(parents=True, exist_ok=True)
    for backend, pin in pins.items():
        if args.backend not in ('all', backend):
            continue
        checkout = args.upstream_root / pin['directory']
        if not checkout.exists():
            subprocess.run(['git', 'clone', pin['url'], str(checkout)], check=True)
        dirty = subprocess.check_output(['git', '-C', str(checkout), 'status', '--porcelain'], text=True).strip()
        dirty = '\n'.join(line for line in dirty.splitlines() if '__pycache__/' not in line and not line.endswith('.pyc'))
        if dirty:
            raise RuntimeError(f'refusing to modify dirty upstream checkout: {checkout}')
        subprocess.run(['git', '-C', str(checkout), 'checkout', '--detach', pin['commit']], check=True)
        subprocess.run(['uv', 'sync', '--locked', '--project', str(root / 'envs' / backend)], check=True)
        print(f'{backend}: {pin["commit"]}', flush=True)


if __name__ == '__main__':
    main()
