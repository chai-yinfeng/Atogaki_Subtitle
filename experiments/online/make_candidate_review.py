#!/usr/bin/env python3
"""Prepare bound human-review excerpts from raw displays without repairing text."""
import argparse
import json
from pathlib import Path
import subprocess
from asr_metrics import normalized
from controlled_metrics import reference_score
from run_asr import file_sha256


def seconds(value):
    return float(value[:-1]) if isinstance(value, str) and value.endswith('s') else float(value)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--runs', type=Path, required=True)
    p.add_argument('--reference', type=Path, required=True)
    p.add_argument('--cases', type=Path, required=True)
    p.add_argument('--gemini', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    ref = json.loads(a.reference.read_text())
    words = []
    for i in range(4):
        source = json.loads((a.gemini/f'dev-{i}'/'reference.json').read_text())
        words.extend(dict(text=w['text'], start=seconds(w['start_offset'])+i*60, end=seconds(w['end_offset'])+i*60) for w in source['words'])
    runs = []
    for path in sorted(a.runs.glob('full-r*/run.json')):
        meta = json.loads(path.read_text())
        events = [json.loads(line) for line in (path.parent/'events.jsonl').read_text().splitlines()] if (path.parent/'events.jsonl').exists() else []
        snapshots, history = [], {}
        for e in events:
            if e['kind'] == 'display':
                history[e['slot']] = e['text']
                snapshots.append((e['emission_s'], ''.join(history[k] for k in sorted(history))))
        runs.append((path.parent.name, file_sha256(path), meta, events, snapshots))
    legacy = a.cases/'gemini-reference-review.md'
    lines = ['# Development 候选语义核验', '',
             '所有文字、原始时间和片段上下文仍是 model_reference。这里不拼接修复 CPP，不把出现速度判为质量通过。',
             f'参考摘要：`{file_sha256(a.reference)}`。不同 repeat 单独核验，未完成会话保留实际暴露输出。', '',
             '请按 anchor / run 标记 correct、critical_error 或 uncertain；否定、数字、人物关系和主要意思优先。',
             '日语逐字原文与声学末尾可保持 pending；不确定时间不进入正式延迟验收。',
             '下方音频为连续节目上下文，包含短语前后文；音频听到的全部文字不等于标记短语的文字范围。Gemini 仍是四段60s参考，边界处需单独核准。',
             f'保留此前人工备注：[{legacy.name}]({legacy.resolve()})，摘要 `{file_sha256(legacy)}`。', '']
    for anchor in ref['anchors']:
        window = int(anchor['case'].split('-')[-1])
        global_end = window*60 + anchor['audio_end_s']
        lo, hi = max(0, window*60+anchor['estimated_start_s']-5), min(268.534, global_end+4)
        context = ''.join(w['text'] for w in words if w['start'] < hi and w['end'] > lo)
        clip = a.output.parent/'context-audio'/(anchor['id']+'.wav')
        clip.parent.mkdir(exist_ok=True)
        if not clip.exists():
            subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-nostdin','-ss',str(lo),
                '-i',str(a.cases/'dev-full/audio.wav'),'-t',str(hi-lo),str(clip)],check=True)
        lines += [f"## {anchor['id']}：全节目估计末尾 {global_end:.2f}s", '',
                  f"Gemini 短语：{anchor['text']}", '',f'上下文（模型）：{context}', '',f'![上下文音频]({clip.resolve()})','']
        for name, digest, meta, events, snapshots in runs:
            if meta['case']['media_sha256'] != ref['media_sha256']:
                raise ValueError('review run is from different recording')
            scored = reference_score(events, [dict(anchor, audio_end_s=global_end)])['anchors'][0]['model_match']
            before = [text for t,text in snapshots if t <= global_end+2]
            visible = before[-1][-180:] if before else '（无可见文本）'
            final = snapshots[-1][1] if snapshots else ''
            at = normalized(final).find(normalized(anchor['text']))
            excerpt = normalized(final)[max(0,at-60):at+len(normalized(anchor['text']))+60] if at>=0 else '（最终全文未精确匹配此模型短语；不能据此判定语义错误）'
            lines += [f'### {name}（{meta["status"]}）','',f'run 摘要：`{digest}`', '',
                      f'估计末尾 +2s 时的实际显示尾部：{visible}', '',f'最终模型短语附近：{excerpt}', '',
                      f'模型短语精确匹配诊断：{json.dumps(scored,ensure_ascii=False)}', '']
    lines += ['## Holdout（冻结参数后一次运行）','']
    for path in sorted(a.runs.glob('holdout-*/run.json')):
        meta = json.loads(path.read_text())
        events = [json.loads(l) for l in (path.parent/'events.jsonl').read_text().splitlines()] if (path.parent/'events.jsonl').exists() else []
        history = {}
        for e in events:
            if e['kind']=='display':
                history[e['slot']] = e['text']
        lines += [f'### {path.parent.name}：{meta["status"]}', '',f'run 摘要：`{file_sha256(path)}`','',
                  ''.join(history[k] for k in sorted(history)) or '（无输出）','']
    with a.output.open('x') as f:
        f.write('\n'.join(lines)+'\n')


if __name__ == '__main__':
    main()
