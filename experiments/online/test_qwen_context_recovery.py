from dataclasses import dataclass
import unittest

from qwen_context_recovery import (ASRContextRecoveryError, looks_like_context_copy,
                                   transcribe_with_recovery)


@dataclass
class Result:
    text: str
    truncated: bool = False
    finish_reason: str = 'eos'


class RecoveryTests(unittest.TestCase):
    def run_case(self, outputs, context='候補Ａ 候補Ｂ'):
        calls = []
        audio = object()
        seconds = [0.0]
        def transcribe(received, **kwargs):
            self.assertIs(received, audio)
            self.assertEqual(kwargs['language'], 'Japanese')
            calls.append(kwargs['context'])
            seconds[0] += 1.5
            value = outputs[len(calls)-1]
            if isinstance(value, Exception):
                raise value
            return value
        try:
            result = transcribe_with_recovery(transcribe, audio, language='Japanese',
                       context=context, clock=lambda: seconds[0])
        except ASRContextRecoveryError as error:
            return error, calls
        return result, calls

    def test_normal_biased_result_is_not_redecoded(self):
        result, calls = self.run_case([Result('候補Ａについて話しました。')])
        self.assertEqual(calls, ['候補Ａ 候補Ｂ'])
        self.assertFalse(result['used_fallback'])

    def test_copy_retry_keeps_raw_rejection_and_all_compute(self):
        result, calls = self.run_case([Result('候補A、候補B。'), Result('元の音声です。')])
        self.assertEqual(calls, ['候補Ａ 候補Ｂ',''])
        self.assertEqual(result['attempts'][0]['rejection_reason'], 'suspected_context_copy')
        self.assertEqual(result['attempts'][0]['result']['text'], '候補A、候補B。')
        self.assertEqual(result['result'].text, '元の音声です。')
        self.assertEqual(result['total_compute_s'], 3)

    def test_actual_spoken_vocabulary_may_survive_unbiased_retry(self):
        result, calls = self.run_case([Result('候補Ａ 候補Ｂ'),Result('候補Ａ 候補Ｂ')])
        self.assertEqual(len(calls),2)
        self.assertEqual(result['result'].text,'候補Ａ 候補Ｂ')

    def test_empty_and_truncated_biased_outputs_get_one_retry(self):
        for bad in [Result(''), Result('途中',True)]:
            with self.subTest(bad=bad):
                result,calls=self.run_case([bad,Result('完整结果')])
                self.assertEqual(len(calls),2)
                self.assertEqual(result['result'].text,'完整结果')

    def test_invalid_fallback_is_explicit_failure_and_never_spliced(self):
        for second in [Result(''),Result('途中',True)]:
            with self.subTest(second=second):
                error,calls=self.run_case([Result('候補Ａ候補Ｂ'),second])
                self.assertIsInstance(error,ASRContextRecoveryError)
                self.assertEqual(len(calls),2)
                self.assertEqual(len(error.attempts),2)
                self.assertFalse(any(a['accepted'] for a in error.attempts))

    def test_no_context_has_no_extra_attempt_budget(self):
        error,calls=self.run_case([Result('',True)],context='')
        self.assertIsInstance(error,ASRContextRecoveryError)
        self.assertEqual(calls,[''])

    def test_runtime_exception_is_not_retried(self):
        error,calls=self.run_case([RuntimeError('device failure')])
        self.assertIsInstance(error,ASRContextRecoveryError)
        self.assertEqual(len(calls),1)
        self.assertEqual(error.attempts[0]['rejection_reason'],'runtime_error')

    def test_silence_hallucination_is_not_misrepresented_as_fixed(self):
        result,calls=self.run_case([Result('候補Ａ候補Ｂ'),Result('はい。')])
        self.assertEqual(result['result'].text,'はい。')
        self.assertEqual(len(calls),2)

    def test_copy_matching_is_exact_normalized_not_a_semantic_detector(self):
        self.assertTrue(looks_like_context_copy('候補A、候補B。','候補Ａ 候補Ｂ'))
        self.assertFalse(looks_like_context_copy('候補Aを実際に読みました','候補Ａ 候補Ｂ'))
        self.assertFalse(looks_like_context_copy('説明です','説明'))
        self.assertFalse(looks_like_context_copy('','。 '))


if __name__=='__main__':unittest.main()
