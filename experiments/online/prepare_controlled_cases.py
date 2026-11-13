#!/usr/bin/env python3
"""Freeze authorized local ranges; holdout extraction requires frozen candidate config."""
import argparse
import json
from pathlib import Path
import subprocess
from run_asr import file_sha256


def freeze(case, lo, hi, target, frozen=None):
    if target.exists():
        m = json.loads((target / 'case.json').read_text())
        if (m['media_sha256'], m['start_ms'], m['end_ms']) != (case['media_sha256'], lo, hi):
            raise ValueError('case cannot be overwritten')
        if file_sha256(target / 'audio.wav') != m['audio_sha256']:
            raise ValueError('case PCM changed')
        return
    target.mkdir(parents=True)
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin', '-ss', str(lo / 1000),
                    '-i', case['media_path'], '-t', str((hi - lo) / 1000), '-ar', '16000', '-ac', '1',
                    '-c:a', 'pcm_s16le', str(target / 'audio.wav')], check=True)
    m = dict(version=2, case_id=case['id'], role=case['role'], media_sha256=case['media_sha256'],
             start_ms=lo, end_ms=hi, audio_sha256=file_sha256(target / 'audio.wav'), reference_status='unreviewed',
             frozen_config_sha256=file_sha256(frozen) if frozen else None)
    (target / 'case.json').write_text(json.dumps(m, indent=2) + '\n')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--manifest', type=Path, required=True)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--frozen-config', type=Path)
    a = p.parse_args()
    cases = {c['id']: c for c in json.loads(a.manifest.read_text())['cases']}
    ranges = [('ja-kokichi-vol1-development', i * 60000, (i + 1) * 60000, 'dev-' + str(i)) for i in range(4)]
    ranges += [('ja-kokichi-vol1-development', 0, 268534, 'dev-full'),
               ('ja-eupho-live-long', 0, 600000, 'long-10m'),
               ('ja-eupho-loop-regression', 2951870, 2996870, 'repeat-regression')]
    if a.frozen_config:
        ranges.append(('ja-atogaki-holdout', 120000, 300000, 'holdout'))
    verified = set()
    for key, lo, hi, name in ranges:
        c = cases[key]
        if key not in verified:
            if file_sha256(Path(c['media_path'])) != c['media_sha256']:
                raise ValueError('source changed: ' + key)
            verified.add(key)
        if not c['start_ms'] <= lo < hi <= c['end_ms']:
            raise ValueError('range outside manifest')
        freeze(c, lo, hi, a.root / name, a.frozen_config if c['role'] == 'holdout' else None)
        print(name, flush=True)
    historical = Path(cases['ja-kokichi-vol1-development']['previous_output_path'])
    segments = json.loads(historical.read_text())
    anchors = []
    review = ['# 20 个候选锚点（尚未核验）', '', '文字与时间来自历史 large-v3 ASR，供核验，不是人工 gold。',
              '请分别核对意思、原文及声学结束时间；不确定的项目保持 pending。', '']
    for window in range(4):
        lo, hi = window * 60000, (window + 1) * 60000
        pool = [s for s in segments if lo <= s['start_ms'] and s['end_ms'] <= hi]
        for j in range(5):
            s = pool[round(j * (len(pool) - 1) / 4)]
            text = s['source_text']
            # Bounded suffix for text matching; full utterance retained for semantic review.
            anchor = dict(id=f'dev-{window}-{j}', case=f'dev-{window}', text=text[-12:], full_text=text,
                          audio_end_s=(s['end_ms'] - lo) / 1000, estimated_start_s=(s['start_ms'] - lo) / 1000,
                          text_status='model_reference', timing_status='provider_estimate', semantic_status='pending',
                          critical_error=None, acceptable_texts=[], semantic_observations={})
            anchors.append(anchor)
            clip = a.root / 'review-audio' / (anchor['id'] + '.wav')
            clip.parent.mkdir(exist_ok=True)
            if not clip.exists():
                subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin',
                                '-ss', str(max(0, anchor['estimated_start_s'] - .5)), '-i', str(a.root / anchor['case'] / 'audio.wav'),
                                '-t', str(anchor['audio_end_s'] - max(0, anchor['estimated_start_s'] - .5) + .5),
                                str(clip)], check=True)
            review += [f"## {anchor['id']}（估计末尾 {anchor['audio_end_s']:.2f}s）", '', text, '',
                       f'![音频]({clip.resolve()})', '']
    ref = a.root / 'reference.json'
    if not ref.exists():
        ref.write_text(json.dumps(dict(version=1, source='historical-large-v3-model-reference', media_sha256=cases['ja-kokichi-vol1-development']['media_sha256'],
                       source_sha256=file_sha256(historical),
                       audio_sha256={f'dev-{i}': json.loads((a.root / f'dev-{i}/case.json').read_text())['audio_sha256'] for i in range(4)},
                       anchors=anchors), ensure_ascii=False, indent=2) + '\n')
        (a.root / 'reference-review.md').write_text('\n'.join(review) + '\n')


if __name__ == '__main__':
    main()
