#!/usr/bin/env python3
"""Import a validated, isolated alignment dependency set without changing online packages."""
import argparse
from importlib.metadata import distributions
import json
from pathlib import Path
import shutil


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site-packages',type=Path,required=True)
    parser.add_argument('--destination',type=Path,required=True,help='App data directory')
    args=parser.parse_args()
    source=args.site_packages.resolve()
    versions={d.metadata['Name'].lower():d.version for d in distributions(path=[str(source)])}
    if not all(name in versions for name in ['nagisa','soynlp','dynet38']):
        parser.error('The source must include nagisa, soynlp and DyNET38')
    target=args.destination.resolve()/'runtimes/qwen-alignment'
    if target.exists():
        parser.error('Refusing to overwrite an existing alignment runtime')
    target.parent.mkdir(parents=True,exist_ok=True)
    staged=target.with_name(target.name+'.importing')
    if staged.exists():parser.error('A previous import needs inspection')
    shutil.copytree(source,staged,ignore=shutil.ignore_patterns('__pycache__'))
    (staged/'.atogaki-alignment.json').write_text(json.dumps(dict(version=1,packages=versions),indent=2)+'\n')
    staged.rename(target)
    print('Imported separate Qwen alignment dependencies:',target)


if __name__=='__main__':main()
