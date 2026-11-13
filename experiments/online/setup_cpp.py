#!/usr/bin/env python3
"""Build a private whisper.cpp PCM worker using the existing sidecar source pin."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT / 'local-artifacts/online')
    args = parser.parse_args()
    pins = dict(re.findall(r'^(WHISPER_\w+)="?([^"\n]+)"?$',
                         (ROOT / 'scripts/sidecar-versions.zsh').read_text(), re.M))
    upstream = args.root / 'upstream'
    upstream.mkdir(parents=True, exist_ok=True)
    archive = upstream / 'whisper-cpp.tar.gz'
    if not archive.exists():
        with urllib.request.urlopen(pins['WHISPER_SOURCE_URL'], timeout=120) as response:
            archive.write_bytes(response.read())
    if hashlib.sha256(archive.read_bytes()).hexdigest() != pins['WHISPER_SOURCE_SHA256']:
        raise ValueError('whisper.cpp archive digest mismatch')
    source = upstream / ('whisper.cpp-' + pins['WHISPER_COMMIT'])
    if not source.exists():
        with tarfile.open(archive) as tar:
            tar.extractall(upstream, filter='data')
    build = args.root / 'whisper-cpp-build'
    subprocess.run(['cmake', '-S', str(source), '-B', str(build),
                    '-DCMAKE_BUILD_TYPE=Release', '-DWHISPER_BUILD_EXAMPLES=OFF',
                    '-DWHISPER_BUILD_TESTS=OFF', '-DGGML_METAL_EMBED_LIBRARY=ON'], check=True)
    subprocess.run(['cmake', '--build', str(build), '-j', '4'], check=True)
    worker = build / 'infer-worker'
    subprocess.run(['c++', '-std=c++17', '-O2', str(ROOT / 'experiments/online/cpp/infer_worker.cpp'),
                    '-I' + str(source / 'include'), '-I' + str(source / 'ggml/include'), '-L' + str(build / 'src'), '-lwhisper',
                    '-Wl,-rpath,' + str((build / 'src').resolve()), '-o', str(worker)], check=True)
    print(worker)

if __name__ == '__main__':
    main()
