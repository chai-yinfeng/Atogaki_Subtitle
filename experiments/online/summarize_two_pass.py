#!/usr/bin/env python3
"""Summarize causal worker traces; provider boundaries are diagnostic, not semantic gold."""
import argparse
import json
import re
from pathlib import Path
from harness import percentile


def stats(values):
    return dict(n=len(values), p50_s=percentile(values, .5),
                p95_s=percentile(values, .95), max_s=max(values, default=None))


def summarize(events, summary):
    offset = summary.get('audio_clock_offset_s', 0.0)
    closed = {e['segment_id']: e for e in events if e['kind'] == 'segment_closed'}
    finals = [e for e in events if e['kind'] == 'source_final']
    translations = [e for e in events if e['kind'] in {'translation_draft', 'translation_final'}]
    structural_echo = [e for e in translations if re.search(
        r'〖约束要求〗|〖背景信息|Translation \(wording reference only\):|\[Previous source and translation pairs\]|Group \d+\s+Source:',
        e.get('text', ''))]
    expansion = [e for e in translations if len(e.get('text', '')) > max(300, 6 * len(e.get('source_text', '')))]
    mt_intervals = [(e['emission_s'] - e['compute_s'], e['emission_s']) for e in translations]
    queue_rows = []
    for e in finals:
        inference_start = e['emission_s'] - e['compute_s']
        queued = inference_start - e['queue_wait_s']
        mt_overlap = sum(max(0, min(inference_start, end) - max(queued, start))
                         for start, end in mt_intervals)
        queue_rows.append(dict(segment_id=e['segment_id'], queue_wait_s=e['queue_wait_s'],
                               returned_mt_overlap_s=mt_overlap))
    groups = {}
    for e in translations:
        groups.setdefault(e['group_id'], []).append(e)
    group_closes = {e['group_id']: e for e in events if e['kind'] == 'translation_group_closed'}
    rows = []
    for key, versions in groups.items():
        final = next((e for e in versions if e['kind'] == 'translation_final'), None)
        close = group_closes.get(key)
        last = final or versions[-1]
        reason = last.get('completion_reason') or (close['completion_reason'] if close else None)
        source_end = offset + last.get('first_source_end_s', last['end_s'])
        latest_source = [e for e in finals if e['segment_id'] in last.get('source_revisions', {})]
        last_source_time = max((e['emission_s'] for e in latest_source), default=None)
        # Retrospective matching to the last group's first acoustic segment;
        # the first draft may already show text but is not verified useful MT.
        first = versions[0]
        previous_text = None
        last_text_change = None
        for version in versions:
            if version['text'] != previous_text:
                last_text_change = version['emission_s']
            previous_text = version['text']
        rows.append(dict(group_id=key, has_final=final is not None,
                         eof=reason == 'input_eof',
                         close_reason=reason,
                         first_return_s=first['emission_s'], last_returned_text_change_s=last_text_change,
                         first_return_after_first_source_end_s=first['emission_s'] - source_end,
                         final_after_first_source_end_s=final['emission_s'] - source_end if final else None,
                         final_after_last_source_end_s=final['emission_s'] - offset - final['end_s'] if final else None,
                         final_compute_s=final['compute_s'] if final else None,
                         last_source_final_s=last_source_time,
                         final_after_last_source_final_s=final['emission_s'] - last_source_time if final and last_source_time is not None else None,
                         final_request_start_estimate_s=final['emission_s'] - final['compute_s'] if final else None,
                         revisions=len(versions), stale_returns=sum(bool(e.get('stale_source')) for e in versions)))
    normal = [r for r in rows if r['has_final'] and not r['eof']]
    all_group_ids = set(groups) | set(group_closes)
    finalized_ids = {r['group_id'] for r in rows if r['has_final']}
    return dict(status=summary['status'], audio_seconds=summary['audio_seconds'],
                errors=summary.get('errors', []), peak_final_queue_depth=summary.get('peak_final_queue_depth'),
                source_final_count=len(finals),
                closed_without_source_final=len(set(closed) - {e['segment_id'] for e in finals}),
                source_after_audio_end=stats([e['after_audio_end_s'] for e in finals]),
                source_queue_wait=stats([e['queue_wait_s'] for e in finals]),
                source_compute=stats([e['compute_s'] for e in finals]),
                source_queue_returned_mt_overlap=stats([r['returned_mt_overlap_s'] for r in queue_rows]),
                source_queue_details=queue_rows,
                vad_close_wait=stats([e['emission_s'] - offset - e['end_s'] for e in closed.values() if e.get('reason') != 'input_eof']),
                mt_compute=stats([e['compute_s'] for e in translations]),
                mt_return_count=len(translations),
                prompt_structure_echo_returns=len(structural_echo),
                prompt_structure_echo_finals=sum(e['kind'] == 'translation_final' for e in structural_echo),
                first_prompt_structure_echo_s=min((e['emission_s'] for e in structural_echo), default=None),
                max_target_characters=max((len(e.get('text', '')) for e in translations), default=0),
                large_expansion_returns=len(expansion),
                non_eof_final_after_first_end=stats([r['final_after_first_source_end_s'] for r in normal]),
                non_eof_final_after_last_end=stats([r['final_after_last_source_end_s'] for r in normal]),
                non_eof_final_after_last_qwen=stats([r['final_after_last_source_final_s'] for r in normal if r['final_after_last_source_final_s'] is not None]),
                translated_groups=len(rows), closed_groups_without_any_translation=sorted(set(group_closes)-set(groups)),
                unfinished_groups=len(all_group_ids - finalized_ids),
                groups_without_final=sorted(all_group_ids - finalized_ids),
                eof_groups=sum(r['eof'] for r in rows) + sum(
                    group_closes[key]['completion_reason'] == 'input_eof' for key in set(group_closes)-set(groups)),
                groups=rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--batch', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    cases = []
    for directory in sorted(args.batch.glob('radio-*')):
        if not directory.is_dir() or not (directory / 'summary.json').exists():
            continue
        events = [json.loads(line) for line in (directory / 'events.jsonl').read_text().splitlines()]
        cases.append(dict(case_id=directory.name, **summarize(events, json.loads((directory / 'summary.json').read_text()))))
    result = dict(version=1, cases=cases, notes=[
        'Failed cases retain partial metrics and are not complete 180s runs.',
        'Latencies are relative to worker/provider segment ends, not verified semantic anchors.',
        'EOF groups are excluded from ordinary final latency distributions.',
        'MT return minus compute is an estimated request start, not an observed queue event.',
        'Post-Qwen wait combines semantic commit policy, scheduler and MT; it is not pure model compute.',
        'Prompt-structure echo uses exact template markers; expansion is only a length diagnostic, neither is a semantic quality score.',
        'Returned-MT/ASR-queue overlap uses inferred intervals and omits failed requests; not causal proof or all queue time.',
        'No GUI IPC or paint time included. No verified MT correctness score is produced.'])
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps([{k: r[k] for k in ('case_id', 'status', 'audio_seconds', 'source_after_audio_end', 'source_queue_wait', 'mt_compute', 'non_eof_final_after_first_end', 'non_eof_final_after_last_end', 'unfinished_groups')} for r in cases], indent=2))


if __name__ == '__main__':
    main()
