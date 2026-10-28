"""Sample explicit native QA process IDs; never attach to or control their UI.

RSS includes native/WebKit caches and is not a measurement of JavaScript heap.
Keep the PID/command baseline so exits and PID reuse cannot look like recovery.
"""
import argparse
import json
from pathlib import Path
import subprocess
import time


def parse_processes(output):
    processes = {}
    for line in output.splitlines():
        if not line.strip():
            continue
        pid, rss, weekday, month, day, clock, year, command = line.strip().split(None, 7)
        processes[int(pid)] = {
            'rss_kib': int(rss), 'command': command,
            'started_at': ' '.join([weekday, month, day, clock, year]),
        }
    return processes


def changed_processes(baseline, processes):
    return [pid for pid in processes if pid not in baseline or
            any(processes[pid][key] != baseline[pid][key]
                for key in ('command', 'started_at'))]


def read_processes(pids):
    result = subprocess.run(
        ['ps', '-p', ','.join(map(str, pids)), '-o', 'pid=,rss=,lstart=,comm='],
        capture_output=True, text=True, check=False,
    )
    if result.returncode not in (0, 1) or result.stderr:
        raise RuntimeError(result.stderr or f'ps exited {result.returncode}')
    return parse_processes(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pids', required=True, type=int, nargs='+')
    parser.add_argument('--duration', type=float, default=600)
    parser.add_argument('--interval', type=float, default=5)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.duration <= 0 or args.interval <= 0 or any(pid <= 0 for pid in args.pids):
        parser.error('duration, interval and PIDs must be positive')

    baseline = read_processes(args.pids)
    if set(baseline) != set(args.pids):
        raise SystemExit('One or more requested processes are already missing')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    # Refuse to overwrite evidence from an earlier run.
    with args.output.open('x') as stream:
        while True:
            processes = read_processes(args.pids)
            missing = [pid for pid in args.pids if pid not in processes]
            reused = changed_processes(baseline, processes)
            sample = {'elapsed_s': round(time.monotonic() - started, 3),
                      'unix_s': time.time(), 'processes': processes,
                      'total_rss_kib': sum(p['rss_kib'] for p in processes.values()),
                      'missing_pids': missing, 'reused_pids': reused}
            stream.write(json.dumps(sample) + '\n')
            stream.flush()
            if missing or reused:
                raise SystemExit('Sampled process exited or changed identity; see evidence')
            if sample['elapsed_s'] >= args.duration:
                break
            time.sleep(min(args.interval, args.duration - sample['elapsed_s']))
    print(json.dumps({'output': str(args.output), 'elapsed_s': sample['elapsed_s'],
                      'pids': args.pids}))


if __name__ == '__main__':
    main()
