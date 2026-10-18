#!/usr/bin/env python3
"""Check pinned experimental models; downloading requires --download."""
import argparse
import importlib.util
import json
from pathlib import Path


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,default=Path('local-artifacts/mlx/models'))
    parser.add_argument('--download',action='store_true')
    args=parser.parse_args()
    spec=importlib.util.spec_from_file_location('comparison',Path(__file__).with_name('compare-local-engines.py'))
    comparison=importlib.util.module_from_spec(spec);spec.loader.exec_module(comparison)
    manifest=json.loads(Path(__file__).with_name('mlx-models.json').read_text())
    for item in manifest['models']:
        model=args.directory/item['repo'].split('/')[-1]
        if args.download:
            from huggingface_hub import snapshot_download
            snapshot_download(item['repo'],revision=item['revision'],local_dir=model)
        if not model.is_dir():
            raise SystemExit(f'Missing model {item["repo"]}; explicitly pass --download to fetch it')
        if comparison.digest(model)!=item['tree_sha256']:
            raise SystemExit(f'Model digest mismatch: {item["repo"]}')
        print(item['repo']+' verified')


if __name__=='__main__':
    main()
