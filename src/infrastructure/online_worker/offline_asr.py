"""File ASR + forced alignment bridge. One bounded acoustic chunk per process."""
from dataclasses import asdict
import contextlib
import json
import math
from pathlib import Path
import sys
import unicodedata
import wave

LANGUAGES = {'ja': 'Japanese', 'zh-Hans': 'Chinese', 'en': 'English', 'ko': 'Korean'}


def timed_units(result):
    if result.truncated:
        raise ValueError('Qwen transcription was truncated; refusing incomplete subtitles')
    if result.text.strip() and not result.segments:
        raise ValueError('Forced alignment returned no timestamps for nonempty text')
    segments = result.segments or []
    kept = lambda char: char == "'" or unicodedata.category(char)[0] in 'LN'
    source_positions = [index for index, char in enumerate(result.text) if kept(char)]
    lexical = lambda text: ''.join(char for char in text if kept(char))
    if ''.join(lexical(str(item['text'])) for item in segments) != lexical(result.text):
        raise ValueError('Alignment text differs from ASR; refusing to drop or rewrite text')
    units, position, boundary, pending = [], 0, 0, ''
    for index, segment in enumerate(segments):
        start, end = float(segment['start']), float(segment['end'])
        if not math.isfinite(start) or not math.isfinite(end) or start < 0 or end < start:
            raise ValueError('Forced alignment returned invalid timestamps')
        position += len(lexical(str(segment['text'])))
        next_boundary = source_positions[position] if position < len(source_positions) else len(result.text)
        text = result.text[boundary:next_boundary]
        boundary = next_boundary
        start_ms, end_ms = round(start * 1000), round(end * 1000)
        # Aligner timestamp bins can give short words zero duration. Preserve
        # their text by merging at real neighboring boundaries, never interpolate.
        if end_ms == start_ms:
            if units:
                units[-1]['text'] += text
                units[-1]['end_ms'] = max(units[-1]['end_ms'], end_ms)
            else:
                pending += text
            continue
        text = pending + text
        pending = ''
        if units and start_ms < units[-1]['end_ms']:
            units[-1]['text'] += text
            units[-1]['end_ms'] = max(units[-1]['end_ms'], end_ms)
        else:
            units.append(dict(id=f'word-{index}', text=text, kind='word', start_ms=start_ms,
                              end_ms=end_ms, timing_source='forced_alignment'))
    if pending or (result.text.strip() and not units):
        raise ValueError('Forced alignment returned no usable duration')
    if ''.join(item['text'] for item in units) != result.text:
        raise ValueError('Alignment projection lost source text')
    return units


def main():
    request = json.load(sys.stdin)
    model, aligner = Path(request['model']), Path(request['alignment_model'])
    if not model.is_dir() or not aligner.is_dir():
        raise ValueError('Local Qwen and ForcedAligner directories are required')
    # Libraries must never put progress/model-loading messages on the IPC stream.
    with contextlib.redirect_stdout(sys.stderr):
        import mlx.core as mx
        from mlx_qwen3_asr import Session
        mx.set_cache_limit(512000000)
        import numpy as np
        with wave.open(request['audio'], 'rb') as audio:
            if (audio.getframerate(),audio.getnchannels(),audio.getsampwidth()) != (16000,1,2):
                raise ValueError('The durable executor must provide mono PCM16 / 16kHz WAV')
            samples=np.frombuffer(audio.readframes(audio.getnframes()),dtype='<i2').astype(np.float32)/32768.0
        session = Session(model=str(model), dtype=mx.float16)
        result = session.transcribe((samples,16000), language=LANGUAGES[request['language']],
                                    context=request.get('context', ''), return_timestamps=True,
                                    forced_aligner=str(aligner), return_chunks=True)
        mx.synchronize()
    raw = Path(request['output_prefix']).with_suffix('.json')
    raw.write_text(json.dumps(asdict(result), ensure_ascii=False, indent=2)+'\n')
    print(json.dumps(dict(raw_output_path=str(raw), timed_units=timed_units(result)), ensure_ascii=False))


if __name__ == '__main__':
    main()
