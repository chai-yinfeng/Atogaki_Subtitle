"""Hash-bound semantic review of displayed draft versions; no model or GUI calls.

Association uses final ASR units overlapping the posterior anchor, not guessed
word times. Labels must be supplied by a reviewer; overlap/text equality never
establish meaning. Repeated text shares one label but keeps each state interval.
"""
import hashlib
import json
import math


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True).encode()).hexdigest()


def collect(anchor, events, presentation):
    offset = anchor['core_range_s'][0]
    lo, hi = [v-offset for v in anchor['prior_media_range_s']]
    source_ids = {e['segment_id'] for e in events if e['kind']=='source_final'
                  and max(lo,e['start_s']) < min(hi,e['end_s'])}
    variants, timeline = {}, []

    def variant(groups):
        texts = [g.get('text', '') for g in groups]
        key = digest(texts)
        variants.setdefault(key, dict(texts=texts, first_seen_s=None))
        return key

    previous, clock = None, -math.inf
    for snapshot in presentation['snapshots']:
        now = snapshot['time_s']
        if not math.isfinite(now) or now < clock:
            raise ValueError('display snapshots require finite monotonic clocks')
        clock = now
        groups = [g for g in snapshot['translations']
                  if source_ids.intersection(g.get('source_revisions', {}))]
        key = variant(groups) if groups else None
        signature = (key, tuple((g['group_id'],g['kind']) for g in groups))
        if signature != previous:
            timeline.append(dict(time_s=now, variant=key, groups=groups))
            previous = signature
        if key and variants[key]['first_seen_s'] is None:
            variants[key]['first_seen_s'] = now
    finals = [e for e in events if e['kind'] in ('translation_final','translation_failed')
              and source_ids.intersection(e.get('source_revisions', {}))]
    final_key = variant(finals) if finals else None
    translated = {sid for e in finals if e['kind']=='translation_final' for sid in e['source_revisions']}
    return dict(anchor_id=anchor['id'], case_id=anchor['case_id'],
                fact=anchor['target_semantic_reference'], source_ids=sorted(source_ids),
                source_start_s=lo, reference_end_s=anchor['estimated_end_s']-offset,
                timing_eligible=anchor['estimated_latency_eligible'],
                end_sensitivity_s=[v-offset for v in anchor['end_sensitivity_range_s']],
                variants=variants, timeline=timeline, final_variant=final_key, finals=finals,
                final_source_coverage_complete=bool(source_ids) and source_ids <= translated)


def summarize(packet, observations):
    if observations['packet_sha256'] != digest(packet):
        raise ValueError('packet changed after semantic review')
    labels = observations['judgments']
    if set(labels) != set(packet['variants']):
        raise ValueError('every variant requires exactly one explicit judgment')
    for label in labels.values():
        if label['judgment'] not in ('correct','incorrect','incomplete','uncertain') or not label.get('reviewer'):
            raise ValueError('invalid semantic judgment or missing reviewer')

    def usable(key):
        return key is not None and labels[key]['judgment']=='correct' and not labels[key].get('output_contract_issue')

    def visible_usable(row):
        return usable(row['variant']) and not any(g['kind']=='translation_failed' for g in row['groups'])

    visible = [r for r in packet['timeline'] if r['variant'] is not None]
    correct = [r for r in visible if visible_usable(r)]
    first = correct[0]['time_s'] if correct else None
    regressions = []
    for previous, current in zip(visible, visible[1:]):
        if visible_usable(previous) and not visible_usable(current):
            regressions.append(dict(from_s=previous['time_s'], to_s=current['time_s'],
                                    from_variant=previous['variant'], to_variant=current['variant']))
    final_correct = packet['final_source_coverage_complete'] and usable(packet['final_variant'])
    locked_s = max((e['emission_s'] for e in packet['finals']), default=None)
    bounds = packet['end_sensitivity_s']
    return dict(anchor_id=packet['anchor_id'], first_correct_visible_s=first,
                first_correct_wait_s=None if first is None else first-packet['reference_end_s'],
                first_correct_wait_sensitivity_s=None if first is None else [first-bounds[1],first-bounds[0]],
                regressions=regressions, final_correct=final_correct,
                final_lock_s=locked_s, timing_eligible=packet['timing_eligible'],
                notes=['No native IPC/paint or human acoustic gold.',
                       'Labels concern this anchor and assembled associated text; implicit cross-screen context is not certified.',
                       'First correct does not mean stable or safe to lock; partial and uncertain are not correct.',
                       'Regressions compare subsequent visible versions of this fact, including across a visibility gap.',
                       'Final lock time is independent of when identical draft text first appeared.'])
