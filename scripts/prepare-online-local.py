#!/usr/bin/env python3
"""Import already verified local assets into a desktop profile.

No downloads, cloud requests, credentials, or stable database writes. A public
runtime/model downloader remains a separate delivery from this local preview.
"""
import argparse
import json
from pathlib import Path
import subprocess

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--python-root',type=Path,required=True)
p.add_argument('--site-packages',type=Path,required=True)
p.add_argument('--asr-assets',type=Path,required=True)
p.add_argument('--hy-model',type=Path,required=True)
p.add_argument('--destination',type=Path,required=True)
a=p.parse_args()
root=a.destination.resolve();root.mkdir(parents=True,exist_ok=True)
def copy(source,destination):
    if not source.exists(): raise FileNotFoundError(source)
    if destination.exists(): return
    destination.parent.mkdir(parents=True,exist_ok=True)
    # APFS clones save space; -L makes imported HF weights independent of
    # research cache symlinks. Runtime internal links are kept relative.
    subprocess.run(['cp','-cRL',str(source),str(destination)],check=True)
runtime=root/'runtimes/online'
copy(a.python_root,runtime)
site=runtime/'lib/python3.11/site-packages'
site.mkdir(exist_ok=True)
for item in a.site_packages.iterdir():
    if item.name=='__pycache__' or item.name.startswith('_virtualenv'):continue
    copy(item,site/item.name)
models=root/'models/online';models.mkdir(parents=True,exist_ok=True)
assets=a.asr_assets
names={
'ja':('SenseVoiceSmall','sensevoice-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17'),
'en':('Zipformer English','zipformer-models/sherpa-onnx-streaming-zipformer-en-2023-06-26'),
'zh':('Zipformer Chinese','zipformer-models/sherpa-onnx-streaming-zipformer-zh-int8-2025-06-30'),
'ko':('Zipformer Korean','zipformer-models/sherpa-onnx-streaming-zipformer-korean-2024-06-16')}
manifest=dict(drafts={},finals=[],vad=str(models/'silero_vad.onnx'),hyMt2=str(models/'Hy-MT2-1.8B-Q4_K_M.gguf'))
for lang,(name,relative) in names.items():
    target=models/lang;copy(assets/relative,target)
    manifest['drafts'][lang]=dict(id=lang,name=name,path=str(target))
for size in ['0.6B','1.7B']:
    target=models/('qwen-'+size.lower());copy(assets/'qwen-mlx-models'/size,target)
    manifest['finals'].append(dict(id='qwen-'+size.lower(),name='Qwen3-ASR '+size,path=str(target)))
copy(assets/'silero-v6.2.3/silero_vad.onnx',models/'silero_vad.onnx')
copy(a.hy_model,models/'Hy-MT2-1.8B-Q4_K_M.gguf')
(models/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
subprocess.run([str(runtime/'bin/python3'),'-s','-c','import sys; import sherpa_onnx; import mlx.core; from mlx_qwen3_asr import Session; print("Private online runtime imports passed:",sys.prefix)'],check=True)
print('Prepared local online assets:',root)
