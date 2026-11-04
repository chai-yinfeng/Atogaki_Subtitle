#!/usr/bin/env python3
"""Fixed-ASR policy replay with real local MT; no audio/ASR/cloud calls.

Recorded recognition arrivals stay fixed. MT compute advances a virtual replay
clock, so later arrivals coalesce during a request. This measures conditional MT
delay, NOT a new end-to-end run (there is no ASR/GPU resource contention).
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics
import sys
import time

from harness import HyMT2
from retranslation import Retranslation


def replay(events, policy, translator, duration):
    output, requests, clock = [], [], [0.0]

    def emit(kind, **data):
        event = dict(kind=kind, replay_emission_s=clock[0], **data)
        if kind.startswith('translation_') and kind != 'translation_group_closed':
            event['after_audio_end_s'] = clock[0]-data['end_s']
            first_end = data.get('first_source_end_s')
            if first_end is None:
                first_end = next(e['end_s'] for e in events if e['kind'] == 'source_final'
                                 and e.get('segment_id') in data['source_revisions'])
            event['after_first_audio_end_s'] = clock[0]-first_end
        output.append(event)
        return event

    mt = policy(emit, language='ja')
    arrivals = [e for e in events if e['kind'] in ('source_draft', 'source_final', 'segment_closed')]
    ranges = [(e['start_s'], e['end_s']) for e in events if e['kind'] == 'segment_closed']
    final_ranges = {e['segment_id']: (e['start_s'], e['end_s'])
                    for e in events if e['kind'] == 'segment_closed'}

    def observe(event):
        # Rolling ASR can include VAD's trailing silence. In production the
        # capture thread owns segment ranges; late recognizer snapshots do not
        # extend an already closed range. Reconstruct that contract here.
        start, end = final_ranges[event['segment_id']]
        mt.observe(event['segment_id'], start, min(event['end_s'], end),
                   closed=event['kind'] in ('source_final', 'segment_closed'))
        if event['kind'] != 'segment_closed':
            mt.update(event)

    index = 0
    while clock[0] < duration or index < len(arrivals) or mt.pending():
        def advance(at):
            audio = min(at, duration)
            speech = any(start <= audio < end for start, end in ranges)
            mt.advance(audio, speech=speech, eof=at >= duration)

        while index < len(arrivals) and arrivals[index]['emission_s'] <= clock[0]:
            e = arrivals[index]
            advance(e['emission_s'])
            observe(e)
            index += 1
        advance(clock[0])
        request = mt.take(clock[0])
        if request:
            started = time.monotonic()
            text = mt.cached(request)
            cached = text is not None
            if text is None:
                text = translator.translate(request['source_text'], request['context'],
                                            request['lookahead_source'])
            compute = time.monotonic()-started
            clock[0] += compute
            # Consume arrivals before returning the request, just as capture
            # and local-draft threads can update source while MT is in flight.
            while index < len(arrivals) and arrivals[index]['emission_s'] <= clock[0]:
                e = arrivals[index]
                advance(e['emission_s'])
                observe(e)
                index += 1
            advance(clock[0])
            result = mt.complete(request, text, compute)
            requests.append(dict(**request, compute_s=compute, cached=cached,
                                 accepted=result is not None, text=text))
        else:
            clock[0] += .1
        if clock[0] > duration+120:
            raise RuntimeError('replay failed to drain')
    finals = [e for e in output if e['kind'] == 'translation_final']
    coverage = [sid for e in finals for sid in e['source_revisions']]
    expected = [e['segment_id'] for e in arrivals if e['kind'] == 'source_final']
    if len(coverage) != len(set(coverage)) or set(coverage) != set(expected):
        raise ValueError('final translations must cover each final source exactly once')
    summary = dict(final_blocks=len(finals), draft_updates=sum(e['kind'] == 'translation_draft' for e in output),
                   requests=len(requests), compute_total_s=sum(r['compute_s'] for r in requests),
                   final_after_block_end_p50_s=statistics.median(e['after_audio_end_s'] for e in finals),
                   final_after_first_segment_end_p50_s=statistics.median(e['after_first_audio_end_s'] for e in finals),
                   final_after_first_segment_end_max_s=max(e['after_first_audio_end_s'] for e in finals),
                   source_segments=len(expected), coverage_exactly_once=True)
    return dict(summary=summary, events=output, requests=requests)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--events', type=Path, required=True, help='Private development trace, never a new holdout')
    p.add_argument('--baseline-module', type=Path, required=True)
    p.add_argument('--duration', type=float, required=True)
    p.add_argument('--url', default='http://127.0.0.1:18767/v1')
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    spec = importlib.util.spec_from_file_location('baseline_retranslation', a.baseline_module)
    baseline = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = baseline
    spec.loader.exec_module(baseline)
    events = [json.loads(line) for line in a.events.read_text().splitlines()]
    translator = HyMT2(a.url, 'Hy-MT2-1.8B-Q4_K_M', 'Simplified Chinese')
    # Warm the shared service before either measured policy.
    translator.translate('今日は良い天気です。')
    result = dict(trace_sha256=hashlib.sha256(a.events.read_bytes()).hexdigest(),
                  baseline_sha256=hashlib.sha256(a.baseline_module.read_bytes()).hexdigest(),
                  candidate_sha256=hashlib.sha256(Path('src/infrastructure/online_worker/retranslation.py').read_bytes()).hexdigest(),
                  duration_s=a.duration, note=__doc__, policies={})
    for name, policy in [('baseline', baseline.Retranslation), ('candidate', Retranslation)]:
        result['policies'][name] = replay(events, policy, translator, a.duration)
        (a.output/'comparison.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
        print(name, json.dumps(result['policies'][name]['summary']), flush=True)
    source = ''.join(e['text'] for e in events if e['kind'] == 'source_final')
    started = time.monotonic()
    result['whole_clip_reference'] = dict(source=source, translation=translator.translate(source),
                                         note='Same-model context reference, not gold or a streaming result.')
    result['whole_clip_reference']['compute_s'] = time.monotonic()-started
    (a.output/'comparison.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    lines = ['# 固定 Qwen 原文的日语 MT 对照', '', __doc__, '']
    for name, data in result['policies'].items():
        lines.extend([f'## {name}', '', '```json', json.dumps(data['summary'], indent=2), '```', ''])
        for e in data['events']:
            if e['kind'] == 'translation_final':
                lines.extend([f"### {e['start_s']:.1f}–{e['end_s']:.1f}s · {e['completion_reason']}", '',
                              e['source_text'], '', e['text'], '',
                              f"末段结束后 {e['after_audio_end_s']:.2f}s；首段结束后 {e['after_first_audio_end_s']:.2f}s", ''])
    lines.extend(['## 整段上下文参考（不是标准答案）', '', source, '',
                  result['whole_clip_reference']['translation'], ''])
    (a.output/'review.md').write_text('\n'.join(lines))


if __name__ == '__main__':
    main()
