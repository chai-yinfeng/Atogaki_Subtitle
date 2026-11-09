#!/usr/bin/env python3
"""Prepare hash-bound posterior-anchor review; never infer semantic correctness from overlap."""
import argparse
import hashlib
import json
from collections import Counter
from harness import percentile
from pathlib import Path


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def overlaps(a, b):
    return max(a[0], b[0]) < min(a[1], b[1])


def anchor_row(anchor, result, source_events):
    offset = anchor['core_range_s'][0]
    bounds = [t-offset for t in anchor['prior_media_range_s']]
    if not (0 <= bounds[0] < bounds[1] <= result['duration_s']):
        return None
    source = [e for e in source_events if e['kind']=='source_final' and
              overlaps([e['start_s'],e['end_s']],bounds)]
    ids = {e['segment_id'] for e in source}
    groups = [e for e in result['events'] if e['kind'] in ('translation_final','translation_failed') and
              ids.intersection(e.get('source_revisions',{}))]
    finalized = {sid for e in groups if e['kind']=='translation_final' for sid in e['source_revisions']}
    complete = bool(ids) and ids <= finalized
    lock = max((e['replay_emission_s'] for e in groups),default=None) if complete else None
    interval = [lock-(anchor['end_sensitivity_range_s'][1]-offset),
                lock-(anchor['end_sensitivity_range_s'][0]-offset)] if lock is not None else None
    eof = any(e.get('completion_reason')=='input_eof' for e in groups)
    return dict(anchor_id=anchor['id'],case_id=anchor['case_id'],policy=result['policy']['id'],
                reference_source=anchor['source_text'],reference_semantics=anchor['target_semantic_reference'],
                reference_status=anchor['reference_status'],source_events=source,groups=groups,
                source_ids=sorted(ids),all_related_source_translated=complete,
                related_groups_locked_s=lock,lock_wait_sensitivity_s=interval,
                reference_end_s=anchor['estimated_end_s']-offset,
                timing_status=anchor['timing_status'],estimated_latency_eligible=anchor['estimated_latency_eligible'],
                eof_forced=eof,ordinary_lock_latency_eligible=complete and anchor['estimated_latency_eligible'] and not eof,
                semantic_judgment=None,review_note='',
                notes=['Overlap associates review material, not semantic equivalence.',
                       'Lock wait covers whole ASR units/groups containing the anchor; not first correct word latency.',
                       'Timing sensitivity is an estimator envelope, not a confidence interval.',
                       'Null judgment remains unreviewed; failed/missing groups remain in the denominator.'])


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--anchors',type=Path,required=True)
    parser.add_argument('--study',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--observations',type=Path)
    args=parser.parse_args()
    anchors=json.loads(args.anchors.read_text())['anchors'];rows=[];inputs={str(args.anchors):sha(args.anchors)}
    for path in sorted(args.study.glob('*/result.json')):
        result=json.loads(path.read_text());events_path=path.parent/'source-events.json'
        if sha(events_path)!=result['source_arrivals_sha256']:
            raise ValueError('source arrivals changed: '+str(path))
        source=json.loads(events_path.read_text());inputs[str(path)]=sha(path);inputs[str(events_path)]=sha(events_path)
        for anchor in anchors:
            if anchor['case_id']!=result['case_id']:continue
            row=anchor_row(anchor,result,source)
            if row is not None:
                row['result_path']=str(path.resolve());row['result_sha256']=sha(path)
                rows.append(row)
    if args.observations:
        inputs[str(args.observations)]=sha(args.observations)
        observations={}
        for record in json.loads(args.observations.read_text())['observations']:
            key=(record['case_id'],record['policy'],record['anchor_id'])
            if key in observations or record['judgment'] not in {'correct','incorrect','incomplete','missing','uncertain'}:
                raise ValueError('duplicate or invalid semantic observation')
            if record['reference_sha256']!=sha(args.anchors):
                raise ValueError('reference changed after semantic review')
            observations[key]=record
        for row in rows:
            record=observations.get((row['case_id'],row['policy'],row['anchor_id']))
            if record:
                if record['result_sha256']!=row['result_sha256']:
                    raise ValueError('result changed after semantic review')
                row['semantic_judgment']=record['judgment'];row['review_note']=record['note']
                row['reviewer']=record['reviewer'];row['source_support']=record['source_support']
                row['output_contract_issue']=record.get('output_contract_issue',False)
    summaries=[]
    for case, policy in sorted({(r['case_id'],r['policy']) for r in rows}):
        group=[r for r in rows if (r['case_id'],r['policy'])==(case,policy)]
        counts=Counter(r['semantic_judgment'] or 'pending' for r in group)
        usable=[r for r in group if r['semantic_judgment']=='correct' and
                not r.get('output_contract_issue') and r['all_related_source_translated']]
        timed=[r for r in usable if r['ordinary_lock_latency_eligible']]
        summaries.append(dict(case_id=case,policy=policy,anchors=len(group),judgments=dict(counts),
            usable=len(usable),contract_issue_anchors=sum(bool(r.get('output_contract_issue')) for r in group),
            usable_fraction=len(usable)/len(group) if not counts['pending'] else None,
            timed_usable_anchors=len(timed),
            related_lock_wait_p50_s=percentile([r['related_groups_locked_s']-r['reference_end_s'] for r in timed],.5),
            lock_wait_p50_sensitivity_s=[percentile([r['lock_wait_sensitivity_s'][i] for r in timed],.5) for i in (0,1)]))
    pending=sum(r['semantic_judgment'] is None for r in rows)
    output=dict(version=1,rows=rows,summaries=summaries,input_hashes=inputs,
                review_status='unreviewed' if pending==len(rows) else 'partially_reviewed' if pending else 'available_rows_reviewed',
                notes=['Only completed result files included; check study status for missing/error runs.',
                       'References were frozen before parameter screening; never supply these targets to online MT.'])
    with args.output.open('x') as stream:
        json.dump(output,stream,ensure_ascii=False,indent=2);stream.write('\n')
    print(len(rows),'anchor/policy review rows')


if __name__=='__main__':
    main()
