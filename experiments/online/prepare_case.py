#!/usr/bin/env python3
"""Freeze a development-set audio range without touching source media or holdout."""
import argparse
import json
from pathlib import Path
import subprocess

from run_asr import file_sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--case-id', required=True)
    parser.add_argument('--start-ms', type=int, required=True)
    parser.add_argument('--end-ms', type=int, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--ffmpeg', default='ffmpeg')
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    cases = [case for case in manifest['cases'] if case['id'] == args.case_id]
    if len(cases) != 1:
        parser.error('case ID must match exactly one manifest entry')
    case = cases[0]
    if case['role'] != 'development':
        parser.error('initial online experiments only use development cases; holdout remains untouched')
    if not case['start_ms'] <= args.start_ms < args.end_ms <= case['end_ms']:
        parser.error('requested range is outside the frozen case')
    media = Path(case['media_path'])
    digest = file_sha256(media)
    if digest != case['media_sha256']:
        parser.error('media digest differs from evaluation manifest')
    args.output_dir.mkdir(parents=True, exist_ok=False)
    audio = args.output_dir / 'audio.wav'
    subprocess.run([args.ffmpeg, '-hide_banner', '-loglevel', 'error', '-nostdin',
                    '-ss', str(args.start_ms / 1000), '-i', str(media),
                    '-t', str((args.end_ms - args.start_ms) / 1000),
                    '-ar', '16000', '-ac', '1', '-c:a', 'pcm_s16le', str(audio)], check=True)
    metadata = dict(version=1, case_id=args.case_id, role=case['role'],
                    media_sha256=digest, start_ms=args.start_ms, end_ms=args.end_ms,
                    audio_sha256=file_sha256(audio), reference_status='unreviewed',
                    ffmpeg_version=subprocess.check_output([args.ffmpeg, '-version'], text=True).splitlines()[0])
    (args.output_dir / 'case.json').write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
