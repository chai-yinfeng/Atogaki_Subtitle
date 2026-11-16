"""Desktop single-owner contracts without loading the large model."""
import queue
import unittest

import numpy as np
import _worker_path
from online_worker.speculative_asr import SpeculativeASR, take_asr_job


class FakeCached:
    def __init__(self):
        self.resets = self.calls = 0
        self.audio = np.empty(0, dtype=np.float32)

    def reset(self):
        self.resets += 1
        self.audio = np.empty(0, dtype=np.float32)

    def transcribe(self, samples):
        self.calls += 1
        self.audio = samples.copy()
        return dict(text='source', finish_reason='eos')


class SingleASRTests(unittest.TestCase):
    def job(self, sid='a', start=0, count=16):
        return dict(segment_id=sid, start_s=start, end_s=start+count/16000,
                    samples=np.arange(count, dtype=np.float32))

    def test_final_fifo_and_stop_precede_latest_preview(self):
        finals, previews = queue.Queue(), queue.Queue(maxsize=1)
        previews.put('latest')
        finals.put('final')
        finals.put(None)
        self.assertEqual(take_asr_job(finals, previews), 'final')
        self.assertIsNone(take_asr_job(finals, previews))
        self.assertEqual(take_asr_job(finals, previews), 'latest')
        with self.assertRaises(queue.Empty):
            take_asr_job(finals, previews)

    def test_same_complete_audio_reuses_preview_without_a_second_decode(self):
        cached = FakeCached()
        asr = SpeculativeASR(cached)
        self.assertFalse(asr.transcribe(self.job())['reused'])
        self.assertTrue(asr.transcribe(self.job())['reused'])
        self.assertEqual(cached.calls, 1)
        changed = self.job()
        changed['samples'][0] = -1
        with self.assertRaises(ValueError):
            asr.transcribe(changed)

    def test_shorter_vad_final_and_new_origin_reset_audio_cache(self):
        cached = FakeCached()
        asr = SpeculativeASR(cached)
        asr.transcribe(self.job(count=32))
        asr.transcribe(self.job(count=48))
        self.assertEqual(cached.resets, 1)
        asr.transcribe(self.job(count=40))
        self.assertEqual(cached.resets, 2)
        asr.transcribe(self.job('b', start=1, count=16))
        self.assertEqual(cached.resets, 3)
        self.assertEqual(cached.calls, 4)


if __name__ == '__main__':
    unittest.main()
