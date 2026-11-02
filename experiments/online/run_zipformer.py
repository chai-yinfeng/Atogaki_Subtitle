#!/usr/bin/env python3
"""Pace a WAV into sherpa-onnx Zipformer and record replaceable draft updates."""
import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import time
import wave


def zipformer(model_dir):
    import sherpa_onnx
    def pick(prefix, quantized=True):
        files = sorted(model_dir.glob(prefix + '*.int8.onnx')) if quantized else []
        if not files:
            files = sorted(p for p in model_dir.glob(prefix + '*.onnx') if '.int8.' not in p.name)
        if len(files) != 1:
            raise ValueError(f'expected one {prefix} model, found {files}')
        return str(files[0])
    return sherpa_onnx.OnlineRecognizer.from_transducer(
        tokens=str(model_dir / 'tokens.txt'), encoder=pick('encoder'),
        decoder=pick('decoder', False), joiner=pick('joiner'),
        num_threads=1, provider='cpu', enable_endpoint_detection=False)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--model-dir', type=Path, required=True)
    p.add_argument('--audio', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    import numpy as np
    with wave.open(str(a.audio)) as f:
        if f.getnchannels() != 1 or f.getsampwidth() != 2:
            p.error('input must be mono PCM16 WAV')
        rate = f.getframerate()
        pcm = np.frombuffer(f.readframes(f.getnframes()), dtype='<i2').astype(np.float32) / 32768
    before = time.monotonic()
    recognizer = zipformer(a.model_dir)
    load_s = time.monotonic() - before
    stream = recognizer.create_stream()
    started, previous, updates = time.monotonic(), '', []
    block = int(rate * .032)
    for offset in range(0, len(pcm), block):
        end = min(offset + block, len(pcm))
        time.sleep(max(0, started + end / rate - time.monotonic()))
        stream.accept_waveform(rate, pcm[offset:end])
        while recognizer.is_ready(stream):
            recognizer.decode_stream(stream)
        text = recognizer.get_result(stream)
        if text and text != previous:
            event = dict(kind='draft_replace', emission_s=time.monotonic() - started,
                         received_audio_end_s=end / rate, text=text)
            updates.append(event)
            print(json.dumps(event, ensure_ascii=False), flush=True)
            previous = text
    stream.accept_waveform(rate, np.zeros(int(rate * .66), dtype=np.float32))
    stream.input_finished()
    while recognizer.is_ready(stream):
        recognizer.decode_stream(stream)
    result = dict(audio_sha256=hashlib.sha256(a.audio.read_bytes()).hexdigest(),
                  audio_seconds=len(pcm) / rate, model_dir=str(a.model_dir.resolve()),
                  sherpa_onnx=version('sherpa-onnx'), cpu_threads=1, load_s=load_s,
                  first_nonempty_s=updates[0]['emission_s'] if updates else None,
                  updates=updates, final_text=recognizer.get_result(stream),
                  wall_s=time.monotonic() - started, eof_padding_s=.66,
                  note='first nonempty arrival is not reference-aligned correct-word latency')
    (a.output / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    if not result['final_text'] or not updates:
        raise RuntimeError('no streaming text; inspect result.json')


if __name__ == '__main__':
    main()
