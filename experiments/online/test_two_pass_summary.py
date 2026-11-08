import unittest
from summarize_two_pass import summarize


class SummaryTests(unittest.TestCase):
    def test_eof_and_failed_partial_are_not_hidden(self):
        events = [dict(kind='segment_closed', segment_id='s', emission_s=2.5, end_s=2, reason='vad_or_max_duration'),
                  dict(kind='translation_group_closed', group_id='g', completion_reason='input_eof'),
                  dict(kind='translation_final', group_id='g', emission_s=8, start_s=0, end_s=2,
                       first_source_end_s=1, compute_s=.5, text='译文')]
        result = summarize(events, dict(status='failed', audio_seconds=2, errors=['failure']))
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['closed_without_source_final'], 1)
        self.assertEqual(result['eof_groups'], 1)
        self.assertEqual(result['non_eof_final_after_last_end']['n'], 0)

    def test_queue_overlap_uses_only_intersecting_returned_mt_time(self):
        events = [dict(kind='translation_draft', group_id='g', emission_s=5, start_s=0, end_s=1, compute_s=3, text='译文'),
                  dict(kind='source_final', segment_id='s', emission_s=7, end_s=1,
                       compute_s=1, queue_wait_s=3, after_audio_end_s=6)]
        result = summarize(events, dict(status='completed', audio_seconds=7))
        self.assertEqual(result['source_queue_returned_mt_overlap']['p50_s'], 2)

    def test_closed_group_without_any_return_counts_as_unfinished(self):
        events = [dict(kind='translation_group_closed', group_id='g', completion_reason='pause')]
        result = summarize(events, dict(status='failed', audio_seconds=2))
        self.assertEqual(result['unfinished_groups'], 1)
        self.assertEqual(result['closed_groups_without_any_translation'], ['g'])

    def test_first_end_wait_and_last_end_wait_remain_distinct(self):
        event = dict(kind='translation_final', group_id='g', emission_s=12, start_s=0, end_s=10,
                     first_source_end_s=2, compute_s=.5, text='译文')
        result = summarize([event], dict(status='completed', audio_seconds=10))
        self.assertEqual(result['non_eof_final_after_first_end']['p50_s'], 10)
        self.assertEqual(result['non_eof_final_after_last_end']['p50_s'], 2)
        self.assertIsNone(result['groups'][0]['last_source_final_s'])


if __name__ == '__main__':
    unittest.main()
