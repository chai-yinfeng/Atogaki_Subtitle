#!/usr/bin/env python3
"""CPU-only Silero worker; fixed frames, one thread, no audio files or network."""
import json
import sys
import time
from pathlib import Path
import numpy as np
import torch
import silero_vad
from run_asr import file_sha256

torch.set_num_threads(1)
model = silero_vad.load_silero_vad()
model(torch.zeros(512), 16000)
model.reset_states()
print(json.dumps(dict(ready=True, model_sha256=file_sha256(Path(silero_vad.__file__).parent / 'data/silero_vad.jit'))), flush=True)
while True:
    data = sys.stdin.buffer.read(2048)
    if not data:
        break
    if len(data) != 2048:
        raise ValueError('partial frame')
    before = time.monotonic()
    probability = float(model(torch.from_numpy(np.frombuffer(data, dtype='<f4').copy()), 16000))
    print(json.dumps(dict(probability=probability, compute_s=time.monotonic()-before)), flush=True)
