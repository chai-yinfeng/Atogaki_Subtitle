#!/usr/bin/env python3
"""Run real local ASR with one decode failure and a later export failure.

Everything written by this script lives in a new evaluation directory. The
original media/model are read-only. No cloud provider or production DB is used.
"""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input',required=True,type=Path)
    parser.add_argument('--model',required=True,type=Path)
    parser.add_argument('--engine', choices=['whisper.cpp', 'mlx-whisper'], default='whisper.cpp')
    parser.add_argument('--python', type=Path, default=Path(sys.executable))
    parser.add_argument('--minimum-duration-seconds', type=int, default=3600,
                        help='Default acceptance gate is one hour; use an explicit shorter gate for runtime experiments')
    parser.add_argument('--cli',type=Path,default=Path('target/debug/atogaki-subtitle'))
    parser.add_argument('--ffmpeg',type=Path,default=Path('/Applications/Atogaki.app/Contents/MacOS/ffmpeg'))
    parser.add_argument('--whisper-cli',type=Path,default=Path('/Applications/Atogaki.app/Contents/MacOS/whisper-cli'))
    parser.add_argument('--fail-at',type=int,default=17)
    parser.add_argument('--output',type=Path,default=Path('local-artifacts/recovery'))
    args=parser.parse_args()
    output=args.output.resolve()/str(uuid.uuid4());output.mkdir(parents=True)
    # Later builds must not replace the executable between recovery attempts.
    cli=output/'atogaki-subtitle';shutil.copy2(args.cli.resolve(),cli)
    counter=output/'calls.jsonl';marker=output/'failed-once';wrapper=output/'whisper-fault.py'
    wrapper.write_text(f'''#!{sys.executable}
import json,os,sys
from pathlib import Path
prefix=Path(sys.argv[sys.argv.index('-of')+1])
index=int(prefix.parent.name.split('-',2)[1])
with open({str(counter)!r},'a') as stream:
 stream.write(json.dumps(index)+'\\n');stream.flush();os.fsync(stream.fileno())
marker=Path({str(marker)!r})
if index=={args.fail_at} and not marker.exists():
 marker.write_text('failed')
 print('injected recoverable decode failure',file=sys.stderr)
 sys.exit(42)
os.execv({str(args.whisper_cli.resolve())!r},[{str(args.whisper_cli.resolve())!r},*sys.argv[1:]])
''')
    wrapper.chmod(0o755)
    if args.engine == 'mlx-whisper':
        wrapper.write_text(f'''#!{sys.executable}
import json,os,sys
from pathlib import Path
payload=sys.stdin.buffer.read()
request=json.loads(payload)
prefix=Path(request['output_prefix'])
index=int(prefix.parent.name.split('-',2)[1])
with open({str(counter)!r},'a') as stream:
 stream.write(json.dumps(index)+'\\n');stream.flush();os.fsync(stream.fileno())
marker=Path({str(marker)!r})
if index=={args.fail_at} and not marker.exists():
 marker.write_text('failed')
 print('injected recoverable decode failure',file=sys.stderr)
 sys.exit(42)
saved=prefix.parent/'bridge-request.json'
saved.write_bytes(payload)
with saved.open('rb') as stream:
 os.dup2(stream.fileno(),0)
os.execv({os.path.abspath(args.python)!r},[{os.path.abspath(args.python)!r},*sys.argv[1:]])
''')
    job=output/'job'
    command=[str(cli),'--ffmpeg',str(args.ffmpeg),'--whisper-cli',str(wrapper),'transcribe',str(args.input.resolve()),'--engine',args.engine,'--model',str(args.model.resolve()),'--output-dir',str(job)]
    if args.engine == 'mlx-whisper':command.extend(['--python',str(wrapper)])
    report={'schema_version':1,'engine':args.engine,'cli_sha256':hashlib.sha256(cli.read_bytes()).hexdigest(),'fault_chunk':args.fail_at,'runs':[],'human_boundary_review':'pending'}
    def run(label):
        started=time.perf_counter()
        with (output/f'{label}.log').open('w') as stream:
            result=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT)
        report['runs'].append({'label':label,'exit_code':result.returncode,'elapsed_ms':round((time.perf_counter()-started)*1000)})
        (output/'report.json').write_text(json.dumps(report,indent=2))
        print(label,result.returncode,flush=True)
        return result.returncode
    def evidence():
        return {str(p.relative_to(job)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (job/'asr-checkpoint').glob('chunk-*/*') if p.name in {'evidence.json','provider-output.json','sha256'}}
    assert run('injected-decode-failure')!=0
    committed=evidence();report['committed_files_before_resume']=len(committed)
    assert committed,'failure must happen after completed checkpoints'
    assert run('resume')==0
    after=evidence()
    assert all(after.get(path)==value for path,value in committed.items()),'completed evidence changed'
    plan=json.loads((job/'asr-checkpoint/plan.json').read_text())
    assert plan['duration_ms']>=args.minimum_duration_seconds*1000,'input is shorter than the explicit acceptance gate'
    calls=Counter(json.loads(line) for line in counter.read_text().splitlines())
    assert len(calls)==len(plan['chunks'])
    assert calls[args.fail_at]==2 and all(count==1 for chunk,count in calls.items() if chunk!=args.fail_at)
    report.update(duration_ms=plan['duration_ms'],chunk_count=len(plan['chunks']),only_failed_chunk_repeated=True)
    manifest=json.loads((job/'status.json').read_text());srt=Path(manifest['outputs']['source_srt'])
    srt_bytes=srt.read_bytes();srt.unlink();srt.mkdir()
    cue_ids=[cue['id'] for cue in json.loads((job/'segments.json').read_text())]
    before=counter.read_bytes()
    try:
        assert run('injected-export-failure')!=0
    finally:
        srt.rmdir();srt.write_bytes(srt_bytes)
    assert run('resume-export')==0
    assert counter.read_bytes()==before,'export recovery restarted ASR'
    assert [cue['id'] for cue in json.loads((job/'segments.json').read_text())]==cue_ids
    report.update(export_recovery_did_not_run_asr=True,cue_ids_preserved=True,completed_evidence_unchanged=True)
    (output/'report.json').write_text(json.dumps(report,indent=2))
    print(output/'report.json',flush=True)


if __name__=='__main__':
    main()
