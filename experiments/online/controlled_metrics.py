#!/usr/bin/env python3
"""Reference-aware v3 report; unreviewed model text never passes selection gates."""
import argparse
import json
import hashlib
from pathlib import Path
from asr_metrics import summarize, normalized
from harness import percentile


def reference_score(events, anchors):
    history, snapshots = {}, []
    for e in events:
        if e['kind'] == 'display':
            history[e['slot']] = e['text']
            snapshots.append((e['emission_s'], normalized(''.join(history[k] for k in sorted(history)))))
    rows = []
    for a in anchors:
        variants = [normalized(t) for t in [a['text'], *a.get('acceptable_texts', [])] if normalized(t)]
        states = [(t, any(v in text for v in variants), max((text.count(v) for v in variants), default=0) > 1)
                  for t, text in snapshots]
        hit = next((t for t, yes, ambiguous in states if yes and not ambiguous), None)
        ambiguous = any(amb for _, yes, amb in states if yes)
        stable = None
        if states and states[-1][1] and not ambiguous:
            stable = next((t for t, yes, amb in states if yes and not any(not later[1] for later in states if later[0] > t)), None)
        timing = a.get('timing_status') == 'verified'
        verified = timing and a.get('text_status') == 'verified_verbatim'
        semantic = timing and a.get('semantic_status') == 'verified' and bool(a.get('acceptable_texts'))
        rows.append(dict(id=a['id'], reference_status=a.get('text_status'), timing_status=a.get('timing_status'),
                         verified_verbatim=verified, verified_semantic=semantic, ambiguous=ambiguous,
                         matched=hit is not None and not ambiguous,
                         first_s=hit, stable_s=stable,
                         latency_s=hit - a['audio_end_s'] if hit is not None and not ambiguous else None,
                         stable_latency_s=stable - a['audio_end_s'] if stable is not None else None))
    def group(label):
        eligible = [r for r in rows if r[label]]
        latencies = [r['latency_s'] for r in eligible if r['matched']]
        return dict(eligible=len(eligible), matched=len(latencies), missing_or_ambiguous=len(eligible) - len(latencies),
                    coverage=len(latencies) / len(eligible) if eligible else None,
                    p50_s=percentile(latencies, .5), p95_s=percentile(latencies, .95),
                    stable_p95_s=percentile([r['stable_latency_s'] for r in eligible if r['stable_latency_s'] is not None], .95))
    return dict(total_anchors=len(rows), anchors=rows, verified_text=group('verified_verbatim'),
                verified_semantic=group('verified_semantic'),
                model_reference_diagnostic=dict(matched=sum(r['matched'] for r in rows),
                    p95_s=percentile([r['latency_s'] for r in rows if r['matched']], .95),
                    note='Unreviewed text/provider time; NOT effective-text latency or selection evidence'))


def report(folder, reference=None):
    meta = json.loads((folder / 'run.json').read_text())
    if (folder / 'suite-watchdog.json').exists():
        meta = dict(meta, **json.loads((folder / 'suite-watchdog.json').read_text()))
    events = [json.loads(l) for l in (folder / 'events.jsonl').read_text().splitlines()] if (folder / 'events.jsonl').exists() else []
    metrics = summarize(events) if meta['status'] == 'completed' else None
    result = dict(run_metadata_sha256=hashlib.sha256((folder / 'run.json').read_bytes()).hexdigest(),
                  evaluated_reference_sha256=hashlib.sha256(json.dumps(reference, sort_keys=True, ensure_ascii=False).encode()).hexdigest() if reference else None, run=folder.name, metadata=meta, metrics=metrics, eligibility='provisional_no_verified_reference')
    if metrics:
        calls = [e for e in events if e['kind'] == 'inference' and e['model_decode']]
        duration = meta['audio_duration_s']
        minute = []
        for lo in range(0, int(duration), 60):
            lag = [e['emission_s'] - e['audio_available_s'] for e in calls if lo <= e['audio_available_s'] < lo + 60]
            minute.append(dict(start_s=lo, lag_p50_s=percentile(lag, .5), samples=len(lag)))
        result['minute_backlog'] = minute
        early = [e['emission_s'] - e['audio_available_s'] for e in calls if 60 <= e['audio_available_s'] < 180]
        late = [e['emission_s'] - e['audio_available_s'] for e in calls if duration - 120 <= e['audio_available_s'] < duration]
        delta = percentile(late, .5) - percentile(early, .5) if duration >= 300 and early and late else None
        result['sustained'] = dict(backlog_delta_s=delta, passed=(delta <= 1 and meta['dropped_audio_s'] == 0) if delta is not None else None)
        anchors = []
        if reference and reference.get('media_sha256') == meta['case']['media_sha256']:
            lo, hi = meta['case']['start_ms'] / 1000, meta['case']['end_ms'] / 1000
            for a in reference['anchors']:
                end = a['audio_end_s'] + int(a['case'].split('-')[-1]) * 60
                if lo < end <= hi:
                    anchors.append(dict(a, audio_end_s=end - lo))
        result['reference'] = reference_score(events, anchors)
        g = result['reference']['verified_text']
        if not g['eligible']:
            g = result['reference']['verified_semantic']
        if len(anchors) and g['eligible'] == len(anchors):
            if g['coverage'] >= .95 and g['p50_s'] is not None and g['p50_s'] <= 1 and g['p95_s'] <= 2:
                result['eligibility'] = 'latency_pass_pending_semantic_and_long_run_review'
            else:
                result['eligibility'] = 'latency_or_coverage_failed'
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--runs', nargs='+', type=Path, required=True)
    p.add_argument('--reference', type=Path)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    ref = json.loads(a.reference.read_text()) if a.reference else None
    with a.output.open('x') as out:
        json.dump(dict(version=3, runs=[report(f, ref) for f in a.runs]), out, ensure_ascii=False, indent=2)
        out.write('\n')


if __name__ == '__main__':
    main()
