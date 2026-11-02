#!/usr/bin/env python3
"""Deterministic derived PCM fixtures, with source/transform provenance."""
import argparse
import json
from pathlib import Path
import wave
import numpy as np
from run_asr import file_sha256


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cases', type=Path, required=True)
    a = p.parse_args()
    source = a.cases / 'dev-0/audio.wav'
    source_sha = file_sha256(source)
    if source_sha != json.loads((a.cases/'dev-0/case.json').read_text())['audio_sha256']:
        raise ValueError('development PCM changed')
    with wave.open(str(source)) as w:
        audio = np.frombuffer(w.readframes(w.getnframes()), '<i2').copy()
    speech = audio[10*16000:18*16000]
    fixtures = {
        'silence': (np.zeros(30*16000, '<i2'), '30s digital silence'),
        'long-gap': (np.concatenate([speech, np.zeros(15*16000, '<i2'), speech]), 'speech10-18s +15s silence +same speech'),
        'quiet-onset': (np.concatenate([np.zeros(16000, '<i2'), (speech*.15).astype('<i2'), speech]), '1s silence +8s speech gain0.15 +8s normal speech'),
        'cut-eof': (audio[10*16000:12500*16], 'original10-12.5s, EOF near introduction; exact clause ending not reviewed'),
        'mid-sentence-eof': (audio[26*16000:30400*16], 'original26-30.4s, truncate ongoing place-description; exact phonetic cut remains unreviewed'),
        'music-intro': (audio[:9*16000], 'original0-9s music intro before Gemini first speech annotation; human review pending'),
    }
    for name,(data,transform) in fixtures.items():
        root = a.cases / name
        if root.exists():
            meta = json.loads((root/'case.json').read_text())
            if meta['media_sha256'] != source_sha or file_sha256(root/'audio.wav') != meta['audio_sha256']:
                raise ValueError('existing fixture provenance differs: '+name)
            with wave.open(str(root/'audio.wav')) as w:
                if w.readframes(w.getnframes()) != data.tobytes():
                    raise ValueError('existing fixture PCM differs: '+name)
            continue
        root.mkdir()
        path = root / 'audio.wav'
        with wave.open(str(path), 'wb') as w:
            w.setnchannels(1); w.setsampwidth(2); w.setframerate(16000); w.writeframes(data.tobytes())
        (root / 'case.json').write_text(json.dumps(dict(version=2, case_id=name, role='smoke',
            media_sha256=file_sha256(source), audio_sha256=file_sha256(path), start_ms=0,
            end_ms=len(data)//16, reference_status='synthetic_unreviewed', transform=transform), indent=2)+'\n')


if __name__ == '__main__':
    main()
