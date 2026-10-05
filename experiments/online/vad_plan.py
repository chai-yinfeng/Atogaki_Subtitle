#!/usr/bin/env python3
"""Record causal Silero probabilities once; replays consume only available frames."""
import argparse
import importlib.metadata
import json
from pathlib import Path
import time
import wave

from run_asr import file_sha256


def decisions(frames, silence_s=.5, pre_roll_s=.2, threshold=.5, negative=.35):
    active, last_voice = False, 0.0
    for frame in frames:
        end, probability = frame['end_s'], frame['probability']
        if not active and probability >= threshold:
            active = True
            last_voice = end
            yield dict(kind='start', decision_s=end,
                       audio_start_s=max(0, frame['start_s'] - pre_roll_s))
        elif active:
            if probability >= negative:
                last_voice = end
            elif end - last_voice >= silence_s - 1e-9:
                yield dict(kind='end', decision_s=end, speech_end_s=last_voice)
                active = False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--audio', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    import numpy as np
    import torch
    import silero_vad
    torch.set_num_threads(1)
    model = silero_vad.load_silero_vad()
    model.reset_states()
    with wave.open(str(args.audio)) as wav:
        if (wav.getframerate(), wav.getnchannels(), wav.getsampwidth()) != (16000, 1, 2):
            parser.error('requires 16kHz mono PCM16')
        audio = np.frombuffer(wav.readframes(wav.getnframes()), dtype='<i2').astype('float32') / 32768
    frames = []
    # Cold first invocation excluded from per-frame compute observations.
    model(torch.zeros(512), 16000)
    model.reset_states()
    for start in range(0, len(audio), 512):
        data = torch.from_numpy(np.pad(audio[start:start + 512], (0, max(0, start + 512 - len(audio)))))
        before = time.monotonic()
        probability = float(model(data, 16000).item())
        frames.append(dict(start_s=start / 16000, end_s=min(start + 512, len(audio)) / 16000,
                           probability=probability, compute_s=time.monotonic() - before))
    model_file = Path(silero_vad.__file__).parent / 'data/silero_vad.jit'
    result = dict(version=1, audio_sha256=file_sha256(args.audio),
                  frame_samples=512, causal=True, mode='cached_causal_probabilities',
                  package_version=importlib.metadata.version('silero-vad'),
                  model_sha256=file_sha256(model_file),
                  compute_total_s=sum(f['compute_s'] for f in frames), frames=frames)
    with args.output.open('x') as out:
        json.dump(result, out, indent=2)
    print(json.dumps({k: v for k, v in result.items() if k != 'frames'}))


if __name__ == '__main__':
    main()
