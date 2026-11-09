#!/usr/bin/env python3
"""Compare fixed causal batches without turning availability into semantic accuracy."""
import argparse
import hashlib
import json
from pathlib import Path
from summarize_two_pass import summarize


def load_batch(path):
    metadata = json.loads((path/'metadata.json').read_text())
    cases = {}
    for case in metadata['cases']:
        directory = path/case['id']
        summary = json.loads((directory/'summary.json').read_text())
        events = [json.loads(line) for line in (directory/'events.jsonl').read_text().splitlines()]
        if summary['audio_sha256'] != case['audio_sha256']:
            raise ValueError('summary audio differs from frozen case')
        cases[case['id']] = dict(summary=summary, metrics=summarize(events, summary),
                                source_finals={e['segment_id']:e for e in events if e['kind']=='source_final'})
    return metadata, cases


def comparable(first, second):
    if first['cases'] != second['cases']:
        raise ValueError('ordered audio cases differ')
    if first['translation_context'] != second['translation_context']:
        raise ValueError('frozen translation context differs')
    # Ignore the ephemeral loopback port, while preserving all model/runtime
    # and generation-service arguments and their order.
    def server(args):
        result=args[:]; result[result.index('--port')+1]='PORT'; return result
    def worker(args):
        result=args[:]; result[result.index('--translation-url')+1]='LOOPBACK'; return result
    if server(first['server']) != server(second['server']) or worker(first['worker_base']) != worker(second['worker_base']):
        raise ValueError('runtime/model command differs')
    for path, digest in first['source_hashes'].items():
        if path.endswith(('.gguf', 'llama-server')) and second['source_hashes'].get(path) != digest:
            raise ValueError('MT model or runtime binary differs')


def compare(baseline, candidate):
    first, old = baseline; second, new = candidate
    comparable(first, second)
    rows=[]
    for case in first['cases']:
        key=case['id']; a=old[key]; b=new[key]
        for field in ('vad_model_sha256', 'vad_config', 'pre_roll_s', 'max_segment_s',
                      'draft_model', 'final_model', 'translation_group_policy'):
            if a['summary'][field] != b['summary'][field]:
                raise ValueError('ASR/group policy differs: '+field)
        common=set(a['source_finals']) & set(b['source_finals'])
        source_comparison=dict(common_segments=len(common),
            baseline_only=len(set(a['source_finals'])-common), candidate_only=len(set(b['source_finals'])-common),
            identical_text=sum(a['source_finals'][sid]['text']==b['source_finals'][sid]['text'] for sid in common),
            identical_bounds=sum((a['source_finals'][sid]['start_s'],a['source_finals'][sid]['end_s'])==
                                 (b['source_finals'][sid]['start_s'],b['source_finals'][sid]['end_s']) for sid in common))
        rows.append(dict(case_id=key, baseline=a['metrics'], candidate=b['metrics'], source_comparison=source_comparison))
    return dict(version=1, cases=rows, baseline_commit=first['code_commit'], candidate_commit=second['code_commit'],
                winner=None, notes=[
        'One pass per configuration. Combined prompt/validation/recovery change, not independent ablations.',
        'Failed baseline cases have partial audio and survivor metrics; do not rank their quantiles against complete candidates.',
        'Degraded/retired groups are untranslated, not successful finals. Missing/failed groups remain reported.',
        'Accepted MT compute, rejected compute and all observed attempts are separate; baseline fatal response compute is unavailable.',
        'Group composition can differ with timing despite identical policy. Final delay uses worker/provider boundaries.',
        'ASR weight paths/configs match; weight fingerprints require the private finish provenance audit.',
        'No verified semantic gold or native IPC/paint latency; no accuracy, CER or 80% quality claim.'])


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args=parser.parse_args()
    result=compare(load_batch(args.baseline),load_batch(args.candidate))
    result['analyzer_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    with args.output.open('x') as stream:
        json.dump(result,stream,ensure_ascii=False,indent=2);stream.write('\n')


if __name__=='__main__':
    main()
