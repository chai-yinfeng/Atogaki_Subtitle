#!/usr/bin/env python3
"""Extract local screen-text candidates; display intervals are not speech timing."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import unicodedata

from run_asr import file_sha256


def layout(source_id):
    vol = int(source_id.split('vol')[1])
    roi = ([.14, .17, .83, .63] if vol in (1, 4) else
           [.10, .25, .88, .56] if vol in (2, 3) else
           [.31, .31, .68, .42] if vol == 33 else [.11, .18, .88, .75])
    return dict(roi=roi, languages=['ja-JP', 'zh-Hans'] if vol <= 4 else ['zh-Hans', 'en-US'])


def key(text):
    # Retain punctuation and every character, including negation. No fuzzy merge.
    return ''.join(unicodedata.normalize('NFKC', text).split())


def body_lines(frame):
    result = []
    for line in frame.get('lines', []):
        text = key(line['text'])
        x, y, width, height = line['box']
        if len(text) < 2 or text.lower().strip(':：') in {'suis', 'sues', 'n-buna'}:
            continue
        if x > .90 and width < .10 and y < .48:  # Side speaker badge, not subtitles.
            continue
        if not re.search(r'[\u3040-\u30ff\u3400-\u9fff]|[A-Za-z]{2}', text):
            continue
        result.append(line)
    return result


def classify(frame):
    lines = body_lines(frame)
    vol = int(frame['source_id'].split('vol')[1])
    darkest = min((line.get('ink_luma', 255) for line in lines), default=255)
    # Later page layout also keeps dark-ish earlier lines. Only the darkest band
    # is current; fixed thresholds there would include history (seen in pilot).
    cutoff = min(110, darkest + 20) if vol >= 34 else 120
    return [(line, 'current_candidate' if line.get('ink_luma', 255) <= cutoff
             else 'context_only') for line in lines]


def merge_frames(frames, sample_s=.5):
    """Merge only continuously visible identical lines; never global-dedupe speech."""
    units, active = [], []
    previous_case = None
    previous_time = None
    for frame in frames:
        case = frame['case_id']
        time = frame['media_time_s']
        if case != previous_case or frame['status'] != 'completed':
            active = []
            previous_time = None
        previous_case = case
        if frame['status'] != 'completed':
            continue
        current = []
        for line, state in classify(frame):
            if state != 'current_candidate':
                continue
            text = key(line['text'])
            x, y, width, height = line['box']
            matches = [u for u in active if u not in current and u['normalized_text'] == text
                       and abs(u['box'][0] - x) < .04
                       and abs(u['box'][1] - y) < .025
                       and time - u['last_seen_s'] <= sample_s * 1.6]
            match = min(matches, key=lambda u: abs(u['box'][0] - x) + abs(u['box'][1] - y),
                        default=None)
            if match is None:
                match = dict(id=f'{case}-screen-{len(units):05}', case_id=case,
                             text=line['text'], normalized_text=text, box=line['box'],
                             first_seen_s=time, last_seen_s=time, observations=0,
                             best_confidence=line['confidence'],
                             evidence_image=frame['image_path'],
                             appearance_bracket_s=[previous_time, time],
                             disappearance_bracket_s=[time, None],
                             timing_status='sampled_display_interval_not_acoustic',
                             reference_status='ocr_candidate_unreviewed',
                             language_hint=('ja_with_kana' if re.search(r'[\u3040-\u30ff]', text)
                                            else 'zh_or_kanji_ambiguous'))
                units.append(match)
            match['last_seen_s'] = time
            match['disappearance_bracket_s'] = [time, None]
            match['observations'] += 1
            if line['confidence'] > match['best_confidence']:
                match['best_confidence'] = line['confidence']
                match['evidence_image'] = frame['image_path']
            current.append(match)
        for unit in active:
            if unit not in current:
                unit['disappearance_bracket_s'] = [unit['last_seen_s'], time]
        active = current
        previous_time = time
    return units


def extract(selection, output, sample_s):
    jobs, commands = [], []
    for case in selection['cases']:
        media = Path(case['media_path'])
        if file_sha256(media) != case['media_sha256']:
            raise ValueError(f"source media changed: {case['id']}")
        frames = output / 'frames' / case['id']
        frames.mkdir(parents=True)
        command = ['ffmpeg', '-hide_banner', '-loglevel', 'info', '-nostdin',
                   '-ss', str(case['start_ms'] / 1000), '-i', str(media),
                   '-map', '0:v:0', '-an', '-sn', '-vf',
                   f"trim=start=0:end={(case['end_ms'] - case['start_ms']) / 1000},"
                   f"select='isnan(prev_selected_t)+gte(t-prev_selected_t,{sample_s})',showinfo",
                   '-fps_mode', 'vfr', '-compression_level', '3', str(frames / '%06d.png')]
        commands.append(command)
        run = subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        (frames / 'ffmpeg.log').write_text(run.stderr)
        run.check_returncode()
        times = [float(t) for t in re.findall(r'\bn:\s*\d+.*?\bpts_time:([-\d.e+]+)', run.stderr)]
        images = sorted(frames.glob('*.png'))
        if len(times) != len(images) or not times:
            raise ValueError(f"frame/timestamp count mismatch: {case['id']}")
        for i, (time, image) in enumerate(zip(times, images)):
            jobs.append(dict(case_id=case['id'], source_id=case['source_id'],
                             frame_id=f"{case['id']}-{i:06}",
                             media_time_s=case['start_ms'] / 1000 + time,
                             image_path=str(image.resolve()), **layout(case['source_id'])))
        print(f"{case['id']}: {len(images)} frames", flush=True)
    (output / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
    (output / 'jobs.json').write_text(json.dumps(jobs, indent=2) + '\n')
    return jobs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--selection', type=Path, required=True)
    parser.add_argument('--helper', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--sample-seconds', type=float, default=.5)
    args = parser.parse_args()
    if not 0 < args.sample_seconds <= 1:
        parser.error('sample-seconds must be in (0, 1]')
    args.output.mkdir(parents=True, exist_ok=False)
    selection = json.loads(args.selection.read_text())
    (args.output / 'selection.snapshot.json').write_text(json.dumps(selection, indent=2) + '\n')
    jobs = extract(selection, args.output, args.sample_seconds)
    raw = args.output / 'raw.jsonl'
    with (args.output / 'vision.log').open('x') as log:
        run = subprocess.run([str(args.helper.resolve()), str(args.output / 'jobs.json'), str(raw)],
                             stdout=log, stderr=subprocess.STDOUT)
    frames = [json.loads(line) for line in raw.read_text().splitlines()]
    if len(frames) != len(jobs):
        raise ValueError('incomplete OCR output; preserve evidence, do not score')
    units = merge_frames(frames, args.sample_seconds)
    (args.output / 'screen-lines.json').write_text(json.dumps(units, ensure_ascii=False, indent=2) + '\n')
    summary = dict(frames=len(frames), failed_frames=sum(f['status'] != 'completed' for f in frames),
                   ocr_requests=sum('reused_from_frame' not in f for f in frames),
                   display_line_units=len(units), sample_seconds=args.sample_seconds,
                   timing_status='display_only_not_acoustic', reference_status='ocr_candidate_unreviewed',
                   helper_sha256=file_sha256(args.helper), script_sha256=file_sha256(Path(__file__)))
    (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary), flush=True)
    run.check_returncode()


if __name__ == '__main__':
    main()
