"""Single Qwen owner for revisable DFlash previews and VAD-finalized source."""
from importlib.metadata import version
import queue

import numpy as np


def take_asr_job(finals, previews):
    """Select only after GPU admission; final FIFO precedes the latest preview."""
    try:
        return finals.get_nowait()
    except queue.Empty:
        return previews.get_nowait()


class SpeculativeASR:
    def __init__(self, cached):
        self.cached = cached
        self.origin = None
        self.last_key = None
        self.last_result = None

    @classmethod
    def load(cls, model_dir, draft_dir):
        if version('mlx-qwen3-asr') != '0.4.4':
            raise RuntimeError('DFlash profile requires mlx-qwen3-asr 0.4.4')
        import mlx.core as mx
        from mlx_qwen3_asr import Session
        from .qwen_speculative import Target, Draft
        from .qwen_prefix_cache import CachedTranscriber
        mx.set_cache_limit(512000000)
        session = Session(model=str(model_dir.resolve()), dtype=mx.float16)
        draft = Draft.load(draft_dir, session.model.config.text_config, dtype=mx.bfloat16)
        if draft.algorithm != 'dflash':
            raise ValueError('The local trial profile requires the DFlash checkpoint')
        mx.synchronize()
        return cls(CachedTranscriber(session, Target(session.model), draft))

    def transcribe(self, job):
        origin = (job['segment_id'], job['start_s'])
        key = (*origin, job['end_s'])
        samples = job['samples']
        if key == self.last_key:
            if not np.array_equal(samples, self.cached.audio):
                raise ValueError('The same audio coordinates cannot contain different samples')
            return dict(self.last_result, reused=True)
        # VAD may close behind the last preview or alter pre-roll. Its shorter
        # final input must not inherit features computed from later audio.
        if origin != self.origin or len(samples) < len(self.cached.audio):
            self.cached.reset()
        result = self.cached.transcribe(samples)
        self.origin, self.last_key, self.last_result = origin, key, result
        return dict(result, reused=False)
