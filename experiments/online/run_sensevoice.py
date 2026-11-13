#!/usr/bin/env python3
"""Run whole-segment SenseVoiceSmall ONNX ASR and optional local Hy-MT2."""
import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import platform
import resource
import time
import wave

from harness import HyMT2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', type=Path, required=True)
    parser.add_argument('--audio', type=Path, action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=2)
    parser.add_argument('--translation-url', help='Optional loopback llama-server /v1 URL')
    args = parser.parse_args()
    if args.repeat < 1:
        parser.error('--repeat must be positive')
    args.output.mkdir(parents=True, exist_ok=False)
    inputs = []
    for path in args.audio:
        with wave.open(str(path)) as audio:
            if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate()) != (1, 2, 16000):
                parser.error('input must be mono PCM16 16kHz WAV')
            duration = audio.getnframes() / audio.getframerate()
        inputs.append(dict(path=str(path.resolve()), seconds=duration,
                           sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    started = time.monotonic()
    import numpy as np
    import sherpa_onnx
    recognizer = sherpa_onnx.OfflineRecognizer.from_sense_voice(
        model=str(args.model_dir / 'model.onnx'), tokens=str(args.model_dir / 'tokens.txt'),
        num_threads=1, provider='cpu', language='ja', use_itn=True)
    metadata = dict(model_dir=str(args.model_dir.resolve()), precision='float32',
                    provider='cpu', num_threads=1, language='ja', use_itn=True,
                    model_load_and_import_s=time.monotonic() - started,
                    python=platform.python_version(), platform=platform.platform(),
                    packages={p: version(p) for p in ['sherpa-onnx', 'sherpa-onnx-core', 'numpy']},
                    inputs=inputs, streaming=False, segmentation='one complete input per stream',
                    timing='complete WAV ready -> complete ASR -> optional complete local translation',
                    exclusions=['audio acquisition', 'VAD endpoint wait', 'first-pass ASR',
                                'model downloads', 'translation server startup'])
    (args.output / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    translator = (HyMT2(args.translation_url, 'Hy-MT2-1.8B-Q4_K_M', 'Simplified Chinese')
                  if args.translation_url else None)
    rows = []
    for repeat in range(args.repeat):
        for index, item in enumerate(inputs):
            before = time.monotonic()
            with wave.open(item['path']) as audio:
                samples = np.frombuffer(audio.readframes(audio.getnframes()), dtype='<i2')
            samples = samples.astype(np.float32) / 32768.0
            stream = recognizer.create_stream()
            stream.accept_waveform(16000, samples)
            recognizer.decode_stream(stream)
            text = stream.result.text
            asr_done = time.monotonic()
            row = dict(repeat=repeat, input_index=index, audio_seconds=item['seconds'],
                       asr_seconds=asr_done - before, asr_result={'text': text}, status='completed')
            (args.output / f'{repeat}-{index}-asr.json').write_text(
                json.dumps(row, ensure_ascii=False, indent=2) + '\n')
            if not text.strip():
                raise RuntimeError('empty ASR output; inspect saved result')
            if translator:
                row['translation'] = translator.translate(text)
                finished = time.monotonic()
                row.update(translation_seconds=finished - asr_done,
                           audio_ready_to_translation_seconds=finished - before)
            row.update(asr_rtf=row['asr_seconds'] / item['seconds'],
                       process_max_rss_bytes=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
            rows.append(row)
            (args.output / 'results.json').write_text(
                json.dumps(rows, ensure_ascii=False, indent=2) + '\n')
            print(json.dumps({k: v for k, v in row.items()
                              if k not in ('asr_result', 'translation')}), flush=True)
    print('Saved transcripts and timings:', args.output, flush=True)


if __name__ == '__main__':
    main()
