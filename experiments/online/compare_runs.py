#!/usr/bin/env python3
"""Compare completed ASR observations; no unverified output is treated as gold."""
import argparse
import json
from pathlib import Path

from harness import read_events, report


def load_run(directory):
    metadata = json.loads((directory / 'run.json').read_text())
    if metadata['status'] != 'completed':
        raise ValueError(f'incomplete run: {directory}')
    events = list(read_events(directory / 'source.jsonl'))
    return metadata, events


def compare(runs):
    results, hashes, durations = [], set(), set()
    for metadata, events in runs:
        hashes.add(metadata['audio_sha256'])
        durations.add(metadata['audio_duration_s'])
        if metadata['timing_mode'] != 'computationally-aware':
            raise ValueError('only computationally-aware runs can be compared here')
        text = ''.join(e['text'] for e in events if e['kind'] == 'source_append')
        results.append(dict(backend=metadata['backend'], model_sha256=metadata['model_sha256'],
                            upstream_commit=metadata['upstream_commit'],
                            wall_time_s=metadata['wall_time_s'], text=text,
                            observed_metrics=report(events)))
    if len(hashes) != 1 or len(durations) != 1:
        raise ValueError('runs must use exactly the same audio')
    return dict(version=1, audio_sha256=next(iter(hashes)),
                audio_duration_s=next(iter(durations)), runs=results,
                reference_status='no_verified_reference',
                limitations=['Different runtimes/devices/precision are not a controlled policy comparison.',
                             'Emission delay uses provider-estimated timing and only emitted text.',
                             'Wall time includes model startup and warmup.',
                             'No absolute accuracy, CER or WER is claimed.'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', type=Path, nargs='+', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = compare([load_run(path) for path in args.runs])
    with args.output.open('x') as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write('\n')


if __name__ == '__main__':
    main()
