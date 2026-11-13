#!/usr/bin/env python3
"""Five short reference phrases per authorized window; all review statuses remain pending."""
import argparse
import json
from pathlib import Path
import subprocess
from run_asr import file_sha256


def seconds(value):
    return float(value[:-1]) if isinstance(value,str) and value.endswith('s') else float(value)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cases', type=Path, required=True)
    p.add_argument('--gemini', type=Path, required=True)
    a = p.parse_args()
    anchors, sources, lines = [], {}, ['# Gemini 参考：20 个候选锚点', '',
        '模型原文和声学时间均待核验。请区分：意思是否正确、日语是否逐字准确、最后一词的声学末尾是否正确。',
        '任何不确定项保持 pending；不会因此制造准确率或有效字幕延迟结论。', '']
    for i in range(4):
        name = f'dev-{i}'
        root = a.gemini/name
        response = json.loads((root/'reference.json').read_text())
        case = json.loads((a.cases/name/'case.json').read_text())
        if response['audio_sha256'] != case['audio_sha256']:
            raise ValueError('reference PCM differs')
        sources[name] = dict(audio_sha256=case['audio_sha256'], response_sha256=file_sha256(root/'response.json'))
        words = response['words']
        groups = []
        for end in range(len(words)):
            hi = seconds(words[end]['end_offset'])
            start = end
            while start>0 and hi-seconds(words[start-1]['start_offset'])<=2.5:
                start -= 1
            lo = seconds(words[start]['start_offset'])
            if .4 <= hi-lo <= 3 and 0<=lo<hi<=60:
                groups.append((lo,hi,''.join(w['text'] for w in words[start:end+1])))
        if len(groups)<5:
            raise ValueError('not enough bounded reference phrases')
        for j in range(5):
            lo,hi,text = groups[round((j+.5)*len(groups)/5)-1]
            aid = f'{name}-{j}'
            anchors.append(dict(id=aid,case=name,text=text,full_text=text,audio_end_s=hi,estimated_start_s=lo,
                text_status='model_reference',timing_status='provider_estimate',semantic_status='pending',
                acceptable_texts=[],critical_error=None,semantic_observations={}))
            clip = a.cases/'gemini-review-audio'/(aid+'.wav')
            clip.parent.mkdir(exist_ok=True)
            if not clip.exists():
                subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-nostdin','-ss',str(max(0,lo-.5)),
                    '-i',str(a.cases/name/'audio.wav'),'-t',str(hi-max(0,lo-.5)+.5),str(clip)],check=True)
            lines += [f'## {aid}：估计 {lo:.2f}–{hi:.2f}s', '',text,'',f'![核验音频]({clip.resolve()})','']
    reference = dict(version=1,source='gemini-3.5-transcribe-model-reference',sources=sources,
        media_sha256=json.loads((a.cases/'dev-0/case.json').read_text())['media_sha256'],anchors=anchors,
        large_v3_baseline=dict(source_sha256=json.loads((a.cases/'reference.json').read_text())['source_sha256'],
                               review_status='pending',observations={}))
    with (a.cases/'gemini-reference.json').open('x') as f:
        json.dump(reference,f,ensure_ascii=False,indent=2); f.write('\n')
    with (a.cases/'gemini-reference-review.md').open('x') as f:
        f.write('\n'.join(lines)+'\n')


if __name__ == '__main__':
    main()
