import threading
import queue
import unittest

import _worker_path
from online_worker.retranslation import Retranslation
from online_worker.stage_admission import StageAdmission
from online_worker.worker import enqueue_final


class StageAdmissionTests(unittest.TestCase):
    def translator(self):
        mt = Retranslation(lambda *_a, **_k: None, max_seconds=1)
        mt.observe('a', 0, 1, closed=True)
        mt.update(dict(kind='source_final', segment_id='a', text='confirmed', source_revision=1))
        return mt

    def test_ready_oldest_translation_runs_before_waiting_later_asr(self):
        mt = self.translator()
        gate = StageAdmission(mt)
        self.addCleanup(gate.abort)
        ran = threading.Event()
        def asr():
            with gate.asr():
                ran.set()
        thread = threading.Thread(target=asr, daemon=True)
        thread.start()
        with gate.condition:
            self.assertTrue(gate.condition.wait_for(lambda: gate.asr_waiting == 1, timeout=1))
        self.assertFalse(ran.is_set())
        request = gate.take_mt()
        self.assertTrue(request['final'])
        mt.complete(request, 'target')
        self.assertFalse(ran.is_set())
        gate.complete_mt()
        self.assertTrue(ran.wait(1))
        thread.join(1)
        self.assertFalse(thread.is_alive())

    def test_shared_device_prevents_mt_while_asr_is_running(self):
        gate = StageAdmission(self.translator())
        # Let ASR start first, before there is any MT work.
        gate.translation = None
        with gate.asr():
            gate.translation = self.translator()
            self.assertIsNone(gate.take_mt())
        self.assertIsNotNone(gate.take_mt())
        gate.complete_mt()

    def test_capacity_candidate_allows_only_one_call_per_stage(self):
        gate = StageAdmission(self.translator(), overlap=True)
        with gate.asr():
            self.assertIsNotNone(gate.take_mt())
            self.assertIsNone(gate.take_mt())
            self.assertTrue(gate.asr_active)
            gate.complete_mt()
            self.assertTrue(gate.asr_active)

    def test_abort_wakes_admission_waiter(self):
        gate = StageAdmission(self.translator())
        errors = []
        def asr():
            try:
                with gate.asr():
                    self.fail('aborted ASR must not start')
            except RuntimeError as error:
                errors.append(str(error))
        thread = threading.Thread(target=asr, daemon=True)
        thread.start()
        with gate.condition:
            self.assertTrue(gate.condition.wait_for(lambda: gate.asr_waiting == 1, timeout=1))
        gate.abort()
        thread.join(1)
        self.assertFalse(thread.is_alive())
        self.assertEqual(len(errors), 1)

    def test_asr_failure_releases_resource_and_no_mt_mode_never_waits(self):
        gate = StageAdmission(None)
        with self.assertRaisesRegex(ValueError, 'model failed'):
            with gate.asr():
                raise ValueError('model failed')
        self.assertFalse(gate.asr_active)
        with gate.asr():
            self.assertTrue(gate.asr_active)

    def test_failed_consumer_releases_capture_backpressure(self):
        jobs = queue.Queue(maxsize=1)
        jobs.put('queued audio')
        errors, results = [], []
        producer = threading.Thread(target=lambda: results.append(enqueue_final(jobs, 'later audio', errors)))
        producer.start()
        errors.append('ASR failed')
        producer.join(1)
        self.assertFalse(producer.is_alive())
        self.assertEqual(results, [False])
        self.assertEqual(jobs.get_nowait(), 'queued audio')


if __name__ == '__main__':
    unittest.main()
