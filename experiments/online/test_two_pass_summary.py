import unittest
from summarize_two_pass import partition_interval, summarize


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

    def test_inherited_eof_on_split_tail_needs_no_new_close_event(self):
        event = dict(kind='translation_final', group_id='tail', emission_s=4, start_s=0, end_s=2,
                     compute_s=.5, text='译文', completion_reason='input_eof')
        result = summarize([event], dict(status='completed', audio_seconds=2))
        self.assertEqual(result['eof_groups'], 1)
        self.assertEqual(result['non_eof_final_after_last_end']['n'], 0)

    def test_template_echo_is_separate_from_length_expansion(self):
        event = dict(kind='translation_final', group_id='g', emission_s=3, start_s=0, end_s=1,
                     compute_s=1, source_text='短句', text='译文\n〖约束要求〗\n提示')
        result = summarize([event], dict(status='completed', audio_seconds=3))
        self.assertEqual(result['prompt_structure_echo_finals'], 1)
        self.assertEqual(result['large_expansion_returns'], 0)

    def test_queue_overlap_uses_only_intersecting_returned_mt_time(self):
        events = [dict(kind='translation_draft', group_id='g', emission_s=5, start_s=0, end_s=1, compute_s=3, text='译文'),
                  dict(kind='source_final', segment_id='s', emission_s=7, end_s=1,
                       compute_s=1, queue_wait_s=3, after_audio_end_s=6)]
        result = summarize(events, dict(status='completed', audio_seconds=7))
        self.assertEqual(result['source_queue_returned_mt_overlap']['p50_s'], 2)

    def test_queue_partition_retains_rejected_draft_and_prior_source_work(self):
        events = [dict(kind='translation_error', emission_s=5, compute_s=4, final=False),
                  dict(kind='source_final', segment_id='first', emission_s=6, end_s=1,
                       compute_s=1, queue_wait_s=2, after_audio_end_s=5),
                  dict(kind='source_final', segment_id='second', emission_s=8, end_s=2,
                       compute_s=1, queue_wait_s=3, after_audio_end_s=6)]
        result = summarize(events, dict(status='completed', audio_seconds=8))
        first, second = result['source_queue_details']
        self.assertEqual(first['observed_work_partition_s'], {'rejected_draft': 2})
        self.assertEqual(second['observed_work_partition_s'],
                         {'rejected_draft': 1, 'other_source_compute': 1, 'unattributed': 1})
        self.assertEqual(result['source_queue_partition_totals_s']['rejected_draft'], 3)
        self.assertEqual(result['source_queue_returned_mt_overlap']['max_s'], 0)
        for row in result['source_queue_details']:
            self.assertAlmostEqual(sum(row['observed_work_partition_s'].values()), row['queue_wait_s'])

    def test_overlapping_intervals_are_not_double_counted_or_arbitrarily_assigned(self):
        self.assertEqual(partition_interval(0, 5, [(1, 4, 'a'), (2, 3, 'b'), (1, 4, 'a')]),
                         {'unattributed': 2, 'a': 2, 'ambiguous': 1})
        self.assertEqual(partition_interval(2, 2, [(1, 4, 'a')]), {})

    def test_rejection_missing_timestamp_is_not_given_invented_interval(self):
        result = summarize([dict(kind='translation_error', compute_s=9)],
                           dict(status='failed', audio_seconds=10))
        self.assertEqual(result['mt_rejections_without_timing'], 1)
        self.assertEqual(result['source_queue_partition_totals_s'], {})

    def test_closed_group_without_any_return_counts_as_unfinished(self):
        events = [dict(kind='translation_group_closed', group_id='g', completion_reason='pause')]
        result = summarize(events, dict(status='failed', audio_seconds=2))
        self.assertEqual(result['unfinished_groups'], 1)
        self.assertEqual(result['closed_groups_without_any_translation'], ['g'])

    def test_audio_clock_offset_does_not_become_latency(self):
        events = [dict(kind='segment_closed', segment_id='s', emission_s=7, end_s=1, reason='pause'),
                  dict(kind='translation_final', group_id='g', emission_s=10, start_s=0, end_s=2,
                       compute_s=1, text='译文')]
        result = summarize(events, dict(status='completed', audio_seconds=2, audio_clock_offset_s=5))
        self.assertEqual(result['vad_close_wait']['p50_s'], 1)
        self.assertEqual(result['non_eof_final_after_last_end']['p50_s'], 3)

    def test_retired_failure_remains_untranslated_and_cost_is_not_hidden(self):
        events = [dict(kind='translation_failed', group_id='g'),
                  dict(kind='translation_error', compute_s=9)]
        result = summarize(events, dict(status='degraded', audio_seconds=10))
        self.assertEqual(result['unfinished_groups'], 0)
        self.assertEqual(result['untranslated_groups'], 1)
        self.assertEqual(result['failed_translation_groups'], ['g'])
        self.assertEqual(result['mt_all_observed_attempt_compute']['p95_s'], 9)

    def test_first_end_wait_and_last_end_wait_remain_distinct(self):
        event = dict(kind='translation_final', group_id='g', emission_s=12, start_s=0, end_s=10,
                     first_source_end_s=2, compute_s=.5, text='译文')
        result = summarize([event], dict(status='completed', audio_seconds=10))
        self.assertEqual(result['non_eof_final_after_first_end']['p50_s'], 10)
        self.assertEqual(result['non_eof_final_after_last_end']['p50_s'], 2)
        self.assertIsNone(result['groups'][0]['last_source_final_s'])


if __name__ == '__main__':
    unittest.main()
