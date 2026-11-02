#!/usr/bin/env python3
"""Common ASR replay metrics without invented commit or speech timing."""
import argparse
import json
from pathlib import Path
import unicodedata

from harness import percentile


def normalized(text):
    return ''.join(unicodedata.normalize('NFKC', text).split())


def withdrawn(previous, current):
    previous, current = normalized(previous), normalized(current)
    common = 0
    for a, b in zip(previous, current):
        if a != b:
            break
        common += 1
    return len(previous) - common


def summarize(events):
    calls = [e for e in events if e['kind'] == 'inference']
    updates = [e for e in events if e['kind'] == 'display']
    visible = [e for e in updates if normalized(e['text'])]
    commits = [e for e in events if e['kind'] == 'commit' and normalized(e['text'])]
    inference = [e for e in calls if e.get('model_decode', True)]
    lag = [e['emission_s'] - e['audio_available_s'] for e in inference]
    decode = [e['compute_s'] for e in inference]
    source = [e['emission_s'] - e['end_s'] for e in commits if e.get('end_s') is not None]
    history, changed, removed = {}, 0, 0
    for e in updates:
        slot = e['slot']
        previous = history.get(slot, '')
        count = withdrawn(previous, e['text'])
        removed += count
        changed += bool(count)
        history[slot] = e['text']
    end = [e for e in events if e['kind'] == 'finish']
    endpoint_latency, endpoint_service = [], []
    last_endpoint = None
    for event in events:
        if event['kind'] == 'vad_decision' and event.get('action') == 'end':
            last_endpoint = event
        elif event['kind'] == 'segment_end' and last_endpoint is not None:
            endpoint_latency.append(event['emission_s'] - last_endpoint['speech_end_s'])
            endpoint_service.append(event['emission_s'] - last_endpoint['decision_s'])
    return dict(first_visible_s=visible[0]['emission_s'] if visible else None,
                first_commit_s=commits[0]['emission_s'] if commits else None,
                processing_calls=len(calls), inference_calls=len(inference), decode_total_s=sum(decode),
                decode_p50_s=percentile(decode, .5), decode_p95_s=percentile(decode, .95),
                input_processing_lag_p50_s=percentile(lag, .5),
                input_processing_lag_p95_s=percentile(lag, .95),
                provider_commit_delay_p50_s=percentile(source, .5),
                provider_commit_delay_p95_s=percentile(source, .95),
                replacement_events=changed, withdrawn_characters=removed,
                final_text=''.join(history[k] for k in sorted(history)),
                eof_flush_s=end[-1]['flush_s'] if end else None,
                endpoint_flush_count=len(endpoint_latency),
                endpoint_from_vad_speech_end_p50_s=percentile(endpoint_latency, .5),
                endpoint_after_detection_p50_s=percentile(endpoint_service, .5),
                dropped_audio_s=0.0,
                note='Display replacements observed through adapters; SS exposes commits only. '
                     'Input lag uses consumed PCM end, not speech end. No verified CER/WER.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runs', nargs='+', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--vad-reference', type=Path, help='optional shared causal activity reference, not gold')
    args = parser.parse_args()
    results, hashes = [], set()
    activity = json.loads(args.vad_reference.read_text()) if args.vad_reference else None
    onset = next((f['end_s'] for f in activity['frames'] if f['probability'] >= .5), None) if activity else None
    for folder in args.runs:
        meta = json.loads((folder / 'run.json').read_text())
        hashes.add(meta['audio_sha256'])
        if activity and activity['audio_sha256'] != meta['audio_sha256']:
            parser.error('activity reference audio differs')
        events = [json.loads(line) for line in (folder / 'events.jsonl').read_text().splitlines()]
        metrics = summarize(events) if meta['status'] == 'completed' else None
        if metrics is not None and onset is not None:
            displays = [e for e in events if e['kind'] == 'display' and normalized(e['text'])]
            metrics['nonempty_updates_before_vad_start'] = sum(e['emission_s'] < onset for e in displays)
            after = [e for e in displays if e['emission_s'] >= onset]
            metrics['first_update_after_vad_start_s'] = after[0]['emission_s'] - onset if after else None
        results.append(dict(run=folder.name, metadata=meta, metrics=metrics))
    if len(hashes) != 1:
        parser.error('runs must use identical PCM audio')
    with args.output.open('x') as out:
        json.dump(dict(version=2, audio_sha256=next(iter(hashes)), activity_reference_onset_s=onset, runs=results), out,
                  ensure_ascii=False, indent=2)


if __name__ == '__main__':
    main()
