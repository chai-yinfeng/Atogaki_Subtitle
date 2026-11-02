import unittest
from unittest.mock import patch, MagicMock
from types import SimpleNamespace
from pathlib import Path

from replay_asr import CppEngine
from run_asr_suite import reusable

from asr_metrics import summarize, withdrawn
from vad_plan import decisions


class CausalVadTests(unittest.TestCase):
    def test_endpoint_never_precedes_required_silence(self):
        frames = [dict(start_s=i / 10, end_s=(i + 1) / 10, probability=.9 if i in (2, 3) else .1)
                  for i in range(10)]
        actions = list(decisions(frames, silence_s=.5, pre_roll_s=.2))
        self.assertEqual(actions[0]['kind'], 'start')
        self.assertAlmostEqual(actions[0]['decision_s'], .3)
        self.assertAlmostEqual(actions[0]['audio_start_s'], 0)
        self.assertAlmostEqual(actions[1]['decision_s'], .9)
        self.assertAlmostEqual(actions[1]['speech_end_s'], .4)
        # The prefix available at .8 cannot expose the future endpoint.
        self.assertEqual(list(decisions(frames[:8], silence_s=.5)), actions[:1])

    def test_hysteresis_does_not_split_low_probability_speech(self):
        frames = [dict(start_s=i * .1, end_s=(i + 1) * .1, probability=p)
                  for i, p in enumerate([.8, .4, .4, .1, .8])]
        actions = list(decisions(frames, silence_s=.2))
        self.assertEqual(len(actions), 1)
        self.assertEqual(actions[0]['kind'], 'start')

    def test_all_silence_has_no_fabricated_segment(self):
        self.assertEqual(list(decisions([dict(start_s=0, end_s=.032, probability=.01)])), [])


class MeasurementTests(unittest.TestCase):
    def test_eof_flush_is_not_algorithm_commit(self):
        result = summarize([
            dict(kind='inference', emission_s=2, audio_available_s=1, compute_s=1, model_decode=True),
            dict(kind='display', emission_s=2, text='あ', slot=0),
            dict(kind='flush', emission_s=3, text='あ', end_s=1, slot=0),
            dict(kind='finish', emission_s=3, flush_s=1)])
        self.assertIsNone(result['first_commit_s'])
        self.assertIsNone(result['provider_commit_delay_p50_s'])
        self.assertEqual(result['input_processing_lag_p50_s'], 1)

    def test_revision_counts_retraction_but_not_growth_or_new_slot(self):
        events = [dict(kind='display', emission_s=i, text=t, slot=s)
                  for i, (s, t) in enumerate([(0, '私は'), (0, '私は学生'), (0, '私が学生'),
                                              (1, 'です'), (1, '')])]
        result = summarize(events)
        self.assertEqual(result['replacement_events'], 2)
        self.assertEqual(result['withdrawn_characters'], 5)
        self.assertEqual(result['final_text'], '私が学生')

    def test_cpp_freeze_cannot_create_commit_latency(self):
        result = summarize([dict(kind='display', emission_s=1, text='你好', slot=0, stage='frozen')])
        self.assertIsNone(result['first_commit_s'])
        self.assertEqual(result['first_visible_s'], 1)

    def test_flush_without_decoder_excluded_from_decode_cost(self):
        result = summarize([dict(kind='inference', emission_s=1, audio_available_s=1,
                                 compute_s=.2, model_decode=False)])
        self.assertEqual(result['inference_calls'], 0)
        self.assertEqual(result['decode_total_s'], 0)

    def test_endpoint_wait_includes_detection_and_service(self):
        result = summarize([
            dict(kind='vad_decision', action='end', emission_s=1.8,
                 decision_s=1.5, speech_end_s=1.0),
            dict(kind='segment_end', emission_s=2.2, slot=0)])
        self.assertAlmostEqual(result['endpoint_from_vad_speech_end_p50_s'], 1.2)
        self.assertAlmostEqual(result['endpoint_after_detection_p50_s'], .7)
        self.assertIsNone(result['first_commit_s'])

    def test_spacing_normalization_is_only_for_display_churn(self):
        self.assertEqual(withdrawn('作 っ た', '作ったもの'), 0)
        self.assertEqual(withdrawn('はい', ''), 2)


class LifecycleTests(unittest.TestCase):
    def test_worker_closed_if_readiness_fails(self):
        worker = MagicMock()
        worker.stdout.readline.return_value = b''
        worker.poll.return_value = None
        args = SimpleNamespace(cpp_worker=Path('/tmp/worker'), model=Path('/tmp/model'), language='ja')
        with patch('replay_asr.subprocess.Popen', return_value=worker):
            with self.assertRaises(ValueError):
                CppEngine(args)
        worker.stdin.close.assert_called_once()
        worker.wait.assert_called_once_with(timeout=5)

    def test_reuse_rejects_changed_settings(self):
        command = ['python', 'replay_asr.py', '--model', '/tmp/model', '--step', '1']
        previous = dict(command=command[1:])
        self.assertTrue(reusable(previous, command))
        self.assertFalse(reusable(previous, command[:-1] + ['0.5']))
        self.assertFalse(reusable({}, command))


if __name__ == '__main__':
    unittest.main()
