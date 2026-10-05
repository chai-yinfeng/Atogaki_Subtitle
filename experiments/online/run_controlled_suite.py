#!/usr/bin/env python3
"""Sequential checkpoints, bounded child groups, provisional shortlist then frozen evaluation."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
from controlled_metrics import report

ROOT = Path(__file__).resolve().parents[2]
BACKENDS = ['whisper-streaming', 'simulstreaming', 'whisper-cpp']


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--cases', type=Path, required=True)
    p.add_argument('--reference', type=Path)
    p.add_argument('--phase', choices=['screen', 'evaluation', 'vad', 'boundaries', 'live-long'], required=True)
    p.add_argument('--live-backends', nargs='+', choices=BACKENDS, default=BACKENDS, help='live-long candidates; other phases keep the fixed matrix')
    p.add_argument('--sizes', nargs='+', default=['small'], choices=['small', 'base', 'tiny'])
    a = p.parse_args()
    a.root.mkdir(parents=True, exist_ok=True)
    rows = []
    ref = json.loads((a.reference or a.cases / 'reference.json').read_text())
    def run(name, backend, case='dev-0', size='small', interval=500, options=()):
        folder = a.root / name
        python = ROOT / 'experiments/online/envs' / ('whisper-streaming' if backend == 'whisper-streaming' else 'simulstreaming') / '.venv/bin/python'
        command = [str(python), str(ROOT / 'experiments/online/replay_controlled.py'), '--backend', backend,
                   '--model-root', str((ROOT / 'local-artifacts/online/controlled-models' / size).resolve()),
                   '--case', str((a.cases / case / 'case.json').resolve()), '--output-dir', str(folder.resolve()),
                   '--decode-ms', str(interval), *options]
        if case == 'holdout':
            command += ['--frozen-config', str((a.root / 'frozen-candidates.json').resolve())]
        if folder.exists():
            old = json.loads((folder / 'run.json').read_text())
            if old['command'][1:] != command[2:] or old['status'] != 'completed':
                raise ValueError('refusing incomplete or different evidence: ' + name)
            for file, sha in old['source_hashes'].items():
                from run_asr import file_sha256
                if file_sha256(ROOT / 'experiments/online' / file) != sha:
                    raise ValueError('driver changed; start new output directory')
        else:
            print(name + ': start', flush=True)
            with (a.root / (name + '.log')).open('x') as log:
                proc = subprocess.Popen(command, stdout=log, stderr=log, start_new_session=True)
                duration = (json.loads((a.cases / case / 'case.json').read_text())['end_ms'] - json.loads((a.cases / case / 'case.json').read_text())['start_ms']) / 1000
                try:
                    code = proc.wait(timeout=duration+180)
                except BaseException as error:
                    os.killpg(proc.pid, signal.SIGTERM)
                    try:
                        proc.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(proc.pid, signal.SIGKILL)
                        proc.wait()
                    if isinstance(error, subprocess.TimeoutExpired):
                        code = -9
                        (folder / 'suite-watchdog.json').write_text(json.dumps(dict(status='watchdog_killed', error='outer timeout; child group terminated'))+'\n')
                    else:
                        raise
                print(name + ': ' + ('completed' if code == 0 else 'FAILED; evidence retained'), flush=True)
        result = report(folder, ref)
        rows.append(result)
        (a.root / (a.phase + '-report.json')).write_text(json.dumps(dict(version=3, runs=rows), ensure_ascii=False, indent=2)+'\n')
        return result
    if a.phase == 'screen':
        for size in a.sizes:
            for interval in [500,1000]:
                for b in BACKENDS:
                    run(f'{size}-{b}-{interval}-off', b, size=size, interval=interval)
    elif a.phase == 'evaluation':
        frozen = a.root / 'frozen-candidates.json'
        if not frozen.exists():
            frozen.write_text(json.dumps(dict(status='provisional_resource_shortlist_not_quality_winner',
                 reason='No verified timing/semantic reference yet; fixed small/500 baseline for repeatability',
                 candidates={b: dict(size='small', interval=500) for b in BACKENDS}), indent=2)+'\n')
        cfg = json.loads(frozen.read_text())['candidates']
        for repeat in range(3):
            order = BACKENDS[repeat:] + BACKENDS[:repeat]
            for b in order:
                run(f'full-r{repeat}-{b}', b, 'dev-full', **cfg[b])
        for b in BACKENDS:
            run('long-'+b, b, 'long-10m', **cfg[b])
        if (a.cases / 'holdout/case.json').exists():
            for b in BACKENDS:
                run('holdout-'+b, b, 'holdout', **cfg[b])
    elif a.phase == 'live-long':
        cfg = json.loads((a.root/'frozen-candidates.json').read_text())['candidates']
        for b in a.live_backends:
            run('live-long-'+b,b,'long-10m',**cfg[b],options=['--vad','live'])
    elif a.phase == 'vad':
        for b in BACKENDS:
            for mode in ['cached','live']:
                run('pause-'+mode+'-'+b, b, options=['--vad',mode,'--vad-plan',str((a.cases/'dev-0/vad.json').resolve())])
        base = ['--vad','cached','--vad-plan',str((a.cases/'dev-0/vad.json').resolve())]
        for name, opts in [('tail',['--tail-decode']), ('finish',['--tail-decode','--finish-on-pause']),
                           ('reset',['--tail-decode','--finish-on-pause','--reset-on-pause']),
                           ('silence250',['--silence-ms','250']), ('silence800',['--silence-ms','800']),
                           ('mask',['--mask-silence'])]:
            run('ablation-'+name, 'simulstreaming', options=base+opts)
    else:
        for case in ['silence', 'music-intro', 'long-gap', 'quiet-onset', 'mid-sentence-eof', 'repeat-regression']:
            for b in BACKENDS:
                run('boundary-'+case+'-off-'+b, b, case)
                if case != 'repeat-regression':
                    run('boundary-'+case+'-live-'+b, b, case, options=['--vad','live'])


if __name__ == '__main__':
    main()
