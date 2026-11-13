"""Sample-clock input shared by WAV replay and a desktop PCM pipe."""
import sys
import wave


class PCMInput:
    def __init__(self, path=None):
        self.path = path
        self.frames = 0
        self.wav = None
        if path is not None:
            self.wav = wave.open(str(path))
            if (self.wav.getnchannels(), self.wav.getsampwidth(), self.wav.getframerate()) != (1, 2, 16000):
                self.wav.close()
                raise ValueError('input must be mono PCM16 16kHz WAV')
            self.frames = self.wav.getnframes()

    def __iter__(self):
        import numpy as np
        stream = self.wav if self.wav else sys.stdin.buffer
        try:
            while True:
                data = stream.readframes(512) if self.wav else stream.read(1024)
                if not data:
                    break
                if len(data) % 2:
                    raise ValueError('truncated PCM16 sample on input pipe')
                yield np.frombuffer(data, dtype='<i2').astype(np.float32) / 32768
        finally:
            if self.wav:
                self.wav.close()


class AudioRing:
    """Absolute sample coordinates; snapshots own their samples before eviction."""
    def __init__(self, seconds=30):
        import numpy as np
        self.samples = np.zeros(seconds*16000, dtype=np.float32)
        self.end = 0

    def append(self, samples):
        count = len(samples)
        if count > len(self.samples):
            raise ValueError('input chunk exceeds audio ring capacity')
        offset = self.end % len(self.samples)
        first = min(count, len(self.samples)-offset)
        self.samples[offset:offset+first] = samples[:first]
        self.samples[:count-first] = samples[first:]
        self.end += count

    def snapshot(self, start, end):
        import numpy as np
        if not max(0, self.end-len(self.samples)) <= start <= end <= self.end:
            raise ValueError('requested audio is outside the live ring')
        offset = start % len(self.samples)
        count = end-start
        first = min(count, len(self.samples)-offset)
        return np.concatenate((self.samples[offset:offset+first], self.samples[:count-first]))
