"""32ms causal producer independent of a single, possibly busy ASR consumer."""
import math
import threading
import time


class FeedClock:
    def __init__(self, audio, plan=None, live=None, silence_s=.5):
        self.audio, self.plan, self.live = audio, plan, live
        self.silence_s = silence_s
        self.lock, self.stop = threading.Lock(), threading.Event()
        self.cursor, self.actions, self.active, self.error = 0, [], False, None
        self.frames, self.compute_s, self.finished = [], 0.0, False

    def start(self, start):
        self.start_time = start
        self.thread = threading.Thread(target=self._produce, name='causal-audio-feed', daemon=True)
        self.thread.start()

    def _produce(self):
        try:
            for i, lo in enumerate(range(0, len(self.audio), 512)):
                hi = min(lo + 512, len(self.audio))
                if self.stop.wait(max(0, self.start_time + hi / 16000 - time.monotonic())):
                    return
                frame = None
                action = None
                if self.live:
                    frame = self.live(self.audio[lo:hi], lo / 16000, hi / 16000)
                elif self.plan:
                    frame = self.plan['frames'][i]
                if frame:
                    self.frames.append(frame)
                    self.compute_s += frame['compute_s'] if self.live else 0
                    # Small state machine equivalent to cached decisions, without rescanning history.
                    p = frame['probability']
                    action = None
                    if not self.active and p >= .5:
                        self.active, self.last_voice = True, frame['end_s']
                        action = dict(kind='start', decision_s=frame['end_s'],
                                      audio_start_s=max(0, frame['start_s'] - .2))
                    elif self.active:
                        if p >= .35:
                            self.last_voice = frame['end_s']
                        elif frame['end_s'] - self.last_voice >= self.silence_s - 1e-9:
                            self.active = False
                            action = dict(kind='end', decision_s=frame['end_s'], speech_end_s=self.last_voice)
                with self.lock:
                    if action:
                        self.actions.append(action)
                    self.cursor = hi
            with self.lock:
                self.finished = True
        except BaseException as e:
            with self.lock:
                self.error = e
                self.finished = True

    def snapshot(self):
        with self.lock:
            if self.error:
                raise self.error
            return self.cursor, list(self.actions), self.finished

    def close(self):
        self.stop.set()
        if hasattr(self, 'thread'):
            self.thread.join(timeout=5)
            if self.thread.is_alive():
                raise RuntimeError('audio producer did not stop')


def next_decode(now, interval):
    # Skip obsolete trigger slots instead of queuing/bursting duplicate inference calls.
    return (math.floor(now / interval + 1e-9) + 1) * interval
