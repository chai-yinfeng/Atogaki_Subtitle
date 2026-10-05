#!/usr/bin/env python3
"""Reference-aware v3 report; unreviewed model text never passes selection gates."""
import argparse
import json
import hashlib
from pathlib import Path
from asr_metrics import summarize, normalized
from harness import percentile

METRICS_SHA256 = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
ASR_METRICS_SHA256 = hashlib.sha256(Path(__file__).with_name("asr_metrics.py").read_bytes()).hexdigest()


def reference_score(events, anchors):
    history, snapshots = {}, []
    for e in events:
        if e['kind'] == 'display':
            history[e['slot']] = e['text']
            snapshots.append((e['emission_s'], normalized(''.join(history[k] for k in sorted(history)))))
    def match(variants, audio_end):
        variants = [normalized(t) for t in variants if normalized(t)]
        states = [(t, any(v in text for v in variants), max((text.count(v) for v in variants), default=0) > 1)
                  for t, text in snapshots]
        ambiguous = any(amb for _, yes, amb in states if yes)
        hit = next((t for t, yes, amb in states if yes and not amb), None)
        stable = None
        if states and states[-1][1] and not ambiguous:
            stable = next((t for t, yes, amb in states if yes and not any(not later[1] for later in states if later[0] > t)), None)
        return dict(ambiguous=ambiguous, matched=hit is not None and not ambiguous,
                    first_s=hit, stable_s=stable,
                    latency_s=hit - audio_end if hit is not None and not ambiguous else None,
                    stable_latency_s=stable - audio_end if stable is not None else None)

    rows = []
    for a in anchors:
        timing = a.get('timing_status') == 'verified'
        verified = timing and a.get('text_status') == 'verified_verbatim'
        semantic = timing and a.get('semantic_status') == 'verified' and bool(a.get('acceptable_texts'))
        model_match = match([a['text']], a['audio_end_s'])
        # A semantic equivalence never makes an unapproved model phrase correct,
        # and an approved paraphrase is not a verbatim transcription.
        text_match = match([a['text']], a['audio_end_s']) if verified else None
        semantic_match = match(a.get('acceptable_texts', []), a['audio_end_s']) if semantic else None
        effective = semantic_match if semantic else text_match if verified else model_match
        rows.append(dict(id=a['id'], reference_status=a.get('text_status'), timing_status=a.get('timing_status'),
                         verified_verbatim=verified, verified_semantic=semantic,
                         explicitly_uncertain=a.get('timing_status') == 'uncertain' or a.get('text_status') == 'uncertain',
                         text_match=text_match, semantic_match=semantic_match, model_match=model_match, **effective))
    def group(label, key):
        eligible = [r[key] for r in rows if r[label]]
        latencies = [r['latency_s'] for r in eligible if r['matched']]
        return dict(eligible=len(eligible), matched=len(latencies), missing_or_ambiguous=len(eligible) - len(latencies),
                    coverage=len(latencies) / len(eligible) if eligible else None,
                    p50_s=percentile(latencies, .5), p95_s=percentile(latencies, .95),
                    stable_p95_s=percentile([r['stable_latency_s'] for r in eligible if r['stable_latency_s'] is not None], .95))
    effective = [r for r in rows if r['verified_verbatim'] or r['verified_semantic']]
    latencies = [r['latency_s'] for r in effective if r['matched']]
    return dict(total_anchors=len(rows), anchors=rows, verified_text=group('verified_verbatim', 'text_match'),
                verified_semantic=group('verified_semantic', 'semantic_match'),
                effective=dict(eligible=len(effective), matched=len(latencies),
                    coverage=len(latencies) / len(effective) if effective else None,
                    p50_s=percentile(latencies, .5), p95_s=percentile(latencies, .95),
                    missing_or_ambiguous=len(effective)-len(latencies),
                    excluded_uncertain=sum(r['explicitly_uncertain'] and not (r['verified_verbatim'] or r['verified_semantic']) for r in rows),
                    pending=sum(not (r['verified_verbatim'] or r['verified_semantic'] or r['explicitly_uncertain']) for r in rows)),
                model_reference_diagnostic=dict(matched=sum(r['model_match']['matched'] for r in rows),
                    p95_s=percentile([r['model_match']['latency_s'] for r in rows if r['model_match']['matched']], .95),
                    note='Unreviewed text/provider time; NOT effective-text latency or selection evidence'))



def resource_growth(folder, metadata):
    path = folder.parent / 'resource-samples.jsonl'
    if not path.exists():
        return dict(status='not_sampled')
    samples, child_series = [], {}
    for line in path.read_text().splitlines():
        try:
            value = json.loads(line)
        except ValueError:  # A concurrent sampler may be appending the final line.
            continue
        if value['run'] == folder.name:
            age = value['run_age_s']-metadata.get('load_s', 0)-metadata.get('warmup_s', 0)
            rss = next((p['rss_bytes'] for p in value['processes'] if p['role']=='runner'), None)
            if rss is not None:
                samples.append((age,rss))
            for child in value['processes']:
                if child['role'] == 'child':
                    child_series.setdefault(child['pid'],[]).append((age,child['rss_bytes']))
    duration = metadata['audio_duration_s']
    early = [rss for t,rss in samples if 60 <= t < 180]
    late = [rss for t,rss in samples if duration-120 <= t < duration]
    children = []
    for pid, series in child_series.items():
        child_early = [rss for t,rss in series if 60 <= t < 180]
        child_late = [rss for t,rss in series if duration-120 <= t < duration]
        children.append(dict(pid=pid,samples=len(series),peak_sampled_bytes=max(rss for _,rss in series),
            early_samples=len(child_early),late_samples=len(child_late),
            rss_delta_bytes=percentile(child_late,.5)-percentile(child_early,.5) if duration>=300 and child_early and child_late else None))
    return dict(status='sampled' if samples else 'not_sampled_for_run', samples=len(samples),children=children,
                runner_peak_sampled_bytes=max((rss for _,rss in samples), default=None),
                early_samples=len(early),late_samples=len(late),
                runner_rss_delta_bytes=percentile(late,.5)-percentile(early,.5) if duration>=300 and early and late else None,
                note='30s ps current RSS of runner; time approximates replay clock from metadata creation minus load/warmup. Children remain separate in raw samples; not total GPU/system peak.')


def report(folder, reference=None):
    meta = json.loads((folder / 'run.json').read_text())
    if (folder / 'suite-watchdog.json').exists():
        meta = dict(meta, **json.loads((folder / 'suite-watchdog.json').read_text()))
    events = [json.loads(l) for l in (folder / 'events.jsonl').read_text().splitlines()] if (folder / 'events.jsonl').exists() else []
    metrics = summarize(events) if meta['status'] == 'completed' else None
    result = dict(metrics_driver_sha256=METRICS_SHA256, asr_metrics_sha256=ASR_METRICS_SHA256, run_metadata_sha256=hashlib.sha256((folder / 'run.json').read_bytes()).hexdigest(),
                  evaluated_reference_sha256=hashlib.sha256(json.dumps(reference, sort_keys=True, ensure_ascii=False).encode()).hexdigest() if reference else None, run=folder.name, metadata=meta, metrics=metrics, eligibility='provisional_no_verified_reference')
    if meta['config']['backend'] == 'simulstreaming':
        result['native_provider_timing_note'] = 'Pinned SS insert_audio returns only the last evicted segment duration when multiple segments expire; native finish clears audio without fully resetting online timestamp offset. Provider timestamps remain raw diagnostics, not unified anchor/media timing.'
    result['resource_growth'] = resource_growth(folder, meta)
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
        g = result['reference']['effective']
        if g['eligible'] and not g['pending']:
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
