#!/usr/bin/env python3
"""Aggregate evidence without promoting proxy timings or incomplete human review."""
import argparse
import json
from pathlib import Path
from controlled_metrics import report
from harness import percentile


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--roots', nargs='+', type=Path, required=True)
    p.add_argument('--reference', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    ref = json.loads(a.reference.read_text())
    reports = [report(path.parent, ref) for root in a.roots for path in sorted(root.glob('*/run.json'))]
    candidates = []
    for backend in ['whisper-streaming','simulstreaming','whisper-cpp']:
        rows = [r for r in reports if r['metadata']['config']['backend']==backend]
        long = [r for r in rows if r['metadata']['case']['role']=='long_regression']
        full = [r for r in rows if r['run'].startswith('full-r')]
        reasons = []
        if len(full)!=3 or any(r['metadata']['status']!='completed' for r in full):
            reasons.append('three complete development repeats required')
        if len(long)!=1 or not long[0].get('sustained',{}).get('passed'):
            reasons.append('long-run stability not passed')
        if not full or any(r['eligibility']!='latency_pass_pending_semantic_and_long_run_review' for r in full):
            reasons.append('verified latency/coverage gate not passed or unavailable')
        holdout = [r for r in rows if r['metadata']['case']['role']=='holdout']
        if len(holdout)!=1 or holdout[0]['metadata']['status']!='completed':
            reasons.append('frozen holdout evaluation incomplete')
        # Human key-semantic observations are per candidate/run, not a property
        # silently inferred from matching a model-generated transcript.
        quality_complete = all(a.get('critical_error') is not None and
                               all(a.get('semantic_observations',{}).get(r['run']) in ['correct','critical_error'] for r in full)
                               for a in ref['anchors']) and bool(full)
        if not quality_complete:
            reasons.append('key-semantic human review incomplete')
        elif any(a['semantic_observations'][r['run']]=='critical_error' and not a['critical_error']
                 for a in ref['anchors'] for r in full):
            reasons.append('additional key-semantic error')
        # Holdout meaning must also be reviewed independently before declaring winner.
        held = ref.get('holdout_semantic_review', {}).get(backend, {})
        if not holdout or held.get('run_metadata_sha256') != holdout[0]['run_metadata_sha256'] or held.get('status') != 'verified':
            reasons.append('holdout key-semantic review pending')
        elif held.get('additional_critical_errors') != 0:
            reasons.append('holdout additional key-semantic error')
        latencies = [anchor['latency_s'] for r in full for anchor in r.get('reference',{}).get('anchors',[])
                     if (anchor['verified_verbatim'] or anchor['verified_semantic']) and anchor['matched']]
        stable = [anchor['stable_latency_s'] for r in full for anchor in r.get('reference',{}).get('anchors',[])
                  if (anchor['verified_verbatim'] or anchor['verified_semantic']) and anchor['stable_latency_s'] is not None]
        candidates.append(dict(backend=backend, eligible=not reasons, reasons=reasons,
                               effective_p95_s=percentile(latencies,.95), stable_p95_s=percentile(stable,.95)))
    eligible = [c for c in candidates if c['eligible']]
    winner = min(eligible, key=lambda c: (c['effective_p95_s'], c['stable_p95_s'] if c['stable_p95_s'] is not None else float('inf'), ['whisper-cpp','whisper-streaming','simulstreaming'].index(c['backend'])))['backend'] if eligible else None
    with a.output.open('x') as out:
        json.dump(dict(version=3, candidates=candidates, winner=winner, runs=reports,
                       note='No winner while reference or semantic review is incomplete; proxy timings remain diagnostics.'), out,
                  ensure_ascii=False, indent=2)
        out.write('\n')


if __name__ == '__main__':
    main()
