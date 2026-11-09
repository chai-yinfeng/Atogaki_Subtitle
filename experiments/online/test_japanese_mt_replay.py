"""A fixed-ASR replay must preserve capture ranges rather than draft padding."""
import unittest

from compare_japanese_mt import replay
from retranslation import Retranslation


class FixedReplayTests(unittest.TestCase):
    def test_trailing_draft_silence_cannot_extend_closed_source_ranges(self):
        events = [
            dict(kind='source_draft', emission_s=1.1, segment_id='a', start_s=0,
                 end_s=1.5, text='途中', source_revision=1),
            dict(kind='segment_closed', emission_s=1.6, segment_id='a', start_s=0, end_s=1),
            dict(kind='source_final', emission_s=2, segment_id='a', start_s=0,
                 end_s=1, text='完了しました。', source_revision=2),
        ]

        class Translator:
            def translate(self, source, context=(), lookahead_source=''):
                return '翻译：'+source

        result = replay(events, Retranslation, Translator(), duration=3)
        final = next(e for e in result['events'] if e['kind'] == 'translation_final')
        self.assertEqual(final['end_s'], 1)
        self.assertEqual(final['first_source_end_s'], 1)
        self.assertEqual(final['after_audio_end_s'], final['after_first_audio_end_s'])
        self.assertTrue(result['summary']['coverage_exactly_once'])

    def test_failed_finals_remain_missing_translation_and_are_not_a_false_success(self):
        events = [dict(kind='segment_closed', emission_s=1.1, segment_id='a', start_s=0, end_s=1),
                  dict(kind='source_final', emission_s=1.2, segment_id='a', start_s=0,
                       end_s=1, text='元の文です。', source_revision=1)]
        class Translator:
            def translate(self, *args): raise ValueError('rejected')
        result = replay(events, Retranslation, Translator(), duration=2)
        self.assertEqual(result['summary']['status'], 'degraded')
        self.assertFalse(result['summary']['coverage_exactly_once'])
        self.assertTrue(result['summary']['accounted_exactly_once'])
        self.assertEqual(result['summary']['failed_groups'], 1)
        self.assertIsNone(result['summary']['final_after_block_end_p50_s'])
        self.assertEqual(sum(e['kind']=='translation_failed' for e in result['events']), 1)


if __name__ == '__main__':
    unittest.main()
