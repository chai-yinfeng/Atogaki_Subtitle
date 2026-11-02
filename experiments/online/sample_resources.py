#!/usr/bin/env python3
"""Observe only this experiment's runner and child RSS; never log unrelated argv."""
import argparse
import datetime
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]


def pids(args):
    p = subprocess.run(['pgrep', *args], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    return [int(x) for x in p.stdout.split()] if p.returncode == 0 else []


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--stop-file', type=Path, required=True)
    p.add_argument('--interval', type=float, default=30)
    a = p.parse_args()
    if a.interval < 5:
        p.error('interval must be at least 5s')
    with a.output.open('x') as out:
        while not a.stop_file.exists():
            owners = pids(['-f', str(ROOT/'experiments/online/replay_controlled.py')])
            loading = []
            for path in a.root.glob('*/run.json'):
                try:
                    meta = json.loads(path.read_text())
                    if meta['status'] == 'loading':
                        loading.append((path.stat().st_mtime, path, meta))
                except (ValueError, OSError):
                    pass
            if len(owners) == 1 and len(loading) == 1:
                _, path, meta = loading[0]
                family, todo = set(owners), list(owners)
                while todo:
                    children = set(pids(['-P', str(todo.pop())]))-family
                    family.update(children)
                    todo.extend(children)
                ps = subprocess.run(['ps','-o','pid=,ppid=,rss=,vsz=', '-p', ','.join(map(str,sorted(family)))],
                                    capture_output=True,text=True)
                processes = []
                for line in ps.stdout.splitlines():
                    pid, ppid, rss, vsz = map(int,line.split())
                    processes.append(dict(pid=pid,ppid=ppid,role='runner' if pid==owners[0] else 'child',rss_bytes=rss*1024,vsz_bytes=vsz*1024))
                emission = None
                events = path.parent/'events.jsonl'
                if events.exists():
                    with events.open('rb') as f:
                        f.seek(max(0,events.stat().st_size-16384))
                        last = f.read().splitlines()
                    try:
                        emission = json.loads(last[-1])['emission_s'] if last else None
                    except (ValueError,KeyError):
                        pass
                out.write(json.dumps(dict(observed_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                          run=path.parent.name, run_age_s=time.time()-path.stat().st_mtime,
                          last_event_emission_s=emission, backend=meta['config']['backend'],processes=processes,
                          note='ps current RSS per process; shared pages/Metal allocations not an additive total; last event may lag a busy call'))+'\n')
                out.flush()
            deadline = time.monotonic()+a.interval
            while time.monotonic()<deadline and not a.stop_file.exists():
                time.sleep(min(1,max(0,deadline-time.monotonic())))


if __name__ == '__main__':
    main()
