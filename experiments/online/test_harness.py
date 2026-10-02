import unittest
from harness import HyMT2, SCHEMA, normalize, replay, report


class Translator:
    def __init__(self):
        self.inputs = []

    def translate(self, source):
        self.inputs.append(source)
        return '译文:' + source


class OnlineTests(unittest.TestCase):
    def test_upstream_formats_have_same_audio_clock(self):
        whisper = list(normalize(['2000 100 1000 hello\n'], 'whisper-streaming'))[0]
        simul = list(normalize(['{"emission_time":2,"start":0.1,"end":1,"text":" hello"}',
                                '{"emission_time":3,"is_final":true}'], 'simulstreaming'))
        self.assertEqual(whisper['end_s'], simul[0]['end_s'])
        self.assertEqual(whisper['emission_s'], simul[0]['emission_s'])
        self.assertEqual(simul[1]['kind'], 'source_boundary')

    def test_revisions_replace_whole_group_and_eof_flushes(self):
        events = list(normalize(['1000 0 500 私は', '7000 500 6000 思わない'], 'whisper-streaming'))
        translator = Translator()
        updates = list(replay(events, translator, 'revisable', 5, 240))
        self.assertEqual(translator.inputs, ['私は', '私は思わない', '私は思わない'])
        self.assertEqual([e['source_revision'] for e in updates], [1, 2, 2])
        self.assertEqual([e['final'] for e in updates], [False, False, True])
        self.assertEqual(report(updates)['replacement_updates'], 2)
        self.assertEqual(updates[-1]['completion_reason'], 'input_eof')

    def test_boundary_policy_and_empty_boundary(self):
        events = list(normalize(['{"emission_time":0,"is_final":true}',
                                 '{"emission_time":1,"start":0,"end":0.5,"text":"はい。"}',
                                 '{"emission_time":2,"start":1,"end":1.5,"text":"次"}'], 'simulstreaming'))
        updates = list(replay(events, Translator(), 'boundary', 5, 240))
        self.assertEqual([e['group_id'] for e in updates], [1, 2])
        self.assertTrue(all(e['final'] for e in updates))
        self.assertEqual([e['completion_reason'] for e in updates], ['sentence_end', 'input_eof'])

    def test_english_word_separator_is_preserved(self):
        events = list(normalize(['1000 0 500  hello', '2000 500 1000  world'], 'whisper-streaming'))
        updates = list(replay(events, Translator(), 'boundary', 5, 240))
        self.assertEqual(updates[0]['source_text'], ' hello world')

    def test_bad_timestamps_rejected(self):
        for lines in (['nan 0 0 x'], ['100 200 100 x'], ['200 0 100 x', '100 0 100 y']):
            with self.assertRaises(ValueError):
                list(normalize(lines, 'whisper-streaming'))

    def test_remote_translation_rejected(self):
        for url in ('https://example.com/v1', 'http://user:secret@localhost/v1'):
            with self.assertRaises(ValueError):
                HyMT2(url, 'hy-mt2', 'Chinese')

    def test_unknown_schema_rejected(self):
        with self.assertRaises(ValueError):
            list(replay([dict(schema='offline', emission_s=0)], Translator(), 'boundary', 5, 240))


class ComparisonTests(unittest.TestCase):
    def metadata(self, digest='same'):
        return dict(audio_sha256=digest, audio_duration_s=60,
                    timing_mode='computationally-aware', backend='test',
                    model_sha256={}, upstream_commit='test', wall_time_s=65)

    def test_comparison_rejects_different_inputs(self):
        from compare_runs import compare
        with self.assertRaises(ValueError):
            compare([(self.metadata(), []), (self.metadata('different'), [])])

    def test_comparison_never_claims_absolute_accuracy(self):
        from compare_runs import compare
        result = compare([(self.metadata(), [])])
        self.assertEqual(result['reference_status'], 'no_verified_reference')
        self.assertNotIn('cer', result['runs'][0]['observed_metrics'])

    def test_comparison_rejects_simulation_without_compute_time(self):
        from compare_runs import compare
        metadata = self.metadata()
        metadata['timing_mode'] = 'computationally-unaware'
        with self.assertRaises(ValueError):
            compare([(metadata, [])])


if __name__ == '__main__':
    unittest.main()
