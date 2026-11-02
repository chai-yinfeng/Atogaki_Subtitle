#!/usr/bin/env python3
"""Convert one verified official checkpoint to unquantized MLX and GGML artifacts."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import urllib.request
import shutil
from run_asr import file_sha256

ROOT = Path(__file__).resolve().parents[2]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--size', choices=['small', 'base', 'tiny'], required=True)
    p.add_argument('--root', type=Path, default=ROOT / 'local-artifacts/online/controlled-models')
    a = p.parse_args()
    upstream = ROOT / 'local-artifacts/online/upstream'
    ss = upstream / 'SimulStreaming'
    sys.path.insert(0, str(ss))
    from simulstreaming.whisper.simul_whisper.whisper import _MODELS, _ALIGNMENT_HEADS
    import numpy as np
    import torch
    target = a.root / a.size
    target.mkdir(parents=True, exist_ok=True)
    url = _MODELS[a.size]
    digest = url.split('/')[-2]
    pt = target / (a.size + '.pt')
    existing = ROOT / 'local-artifacts/online/models' / pt.name
    if not pt.exists():
        if existing.exists() and file_sha256(existing) == digest:
            shutil.copyfile(existing, pt)
        else:
            part = pt.with_suffix('.pt.part')
            with urllib.request.urlopen(url, timeout=120) as src, part.open('wb') as dst:
                shutil.copyfileobj(src, dst)
            if file_sha256(part) != digest:
                raise ValueError('official checkpoint digest mismatch')
            part.rename(pt)
    if file_sha256(pt) != digest:
        raise ValueError('official checkpoint digest mismatch')
    mlx = target / 'mlx'
    mlx.mkdir(exist_ok=True)
    checkpoint = torch.load(pt, map_location='cpu', weights_only=True)
    arrays = {}
    for key, value in checkpoint['model_state_dict'].items():
        if key == 'encoder.positional_embedding':
            continue  # MLX computes the fixed sinusoidal buffer; not a learned weight.
        data = value.detach().cpu().numpy().astype(np.float16)
        if key in ['encoder.conv1.weight', 'encoder.conv2.weight']:
            data = data.transpose(0, 2, 1)
        key = key.replace('.mlp.0.', '.mlp1.').replace('.mlp.2.', '.mlp2.').replace('encoder.positional_embedding', 'encoder._positional_embedding')
        arrays[key] = data
    np.savez(mlx / 'weights.npz', **arrays)
    (mlx / 'config.json').write_text(json.dumps(checkpoint['dims'], indent=2) + '\n')
    pins = dict(line.split('=', 1) for line in (ROOT / 'scripts/sidecar-versions.zsh').read_text().splitlines()
                if line.startswith('WHISPER_COMMIT='))
    commit = pins['WHISPER_COMMIT'].strip('"')
    converter = upstream / ('whisper.cpp-' + commit) / 'models/convert-pt-to-ggml.py'
    assets = ss / 'simulstreaming/whisper/simul_whisper'
    subprocess.run([sys.executable, str(converter), str(pt), str(assets), str(target)], check=True,
                   stdout=subprocess.DEVNULL)
    metadata = dict(version=1, architecture=a.size, source_url=url, source_sha256=digest,
                    alignment_heads=_ALIGNMENT_HEADS[a.size].decode() if isinstance(_ALIGNMENT_HEADS[a.size], bytes) else _ALIGNMENT_HEADS[a.size],
                    source_dims=checkpoint['dims'], parameter_count=sum(v.numel() for v in checkpoint['model_state_dict'].values()),
                    driver_sha256=file_sha256(Path(__file__)), ggml_converter_sha256=file_sha256(converter),
                    ggml_source_commit=commit, asset_upstream_commit=json.loads((ROOT / 'experiments/online/upstream-pins.json').read_text())['simulstreaming']['commit'],
                    conversion_note='MLX conv axes transpose, MLP key rename; fixed encoder sinusoidal buffer recomputed by runtime; official alignment head dump retained',
                    dtype_note='MLX weights fp16; GGML converter f16 with designated small tensors fp32; no quantization. Runtime accumulation audited separately.',
                    assets_sha256={f.name: file_sha256(f) for f in (assets / 'whisper/assets').iterdir() if f.is_file()},
                    artifacts={str(f.relative_to(target)): file_sha256(f) for f in [pt, mlx / 'weights.npz', mlx / 'config.json', target / 'ggml-model.bin']})
    (target / 'provenance.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target)


if __name__ == '__main__':
    main()
