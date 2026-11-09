import unittest
from semantic_anchor_review import anchor_row


class AnchorReviewTests(unittest.TestCase):
    def anchor(self):
        return dict(id='a',case_id='c',core_range_s=[100,120],prior_media_range_s=[104,106],
                    source_text='原文',target_semantic_reference='语义',reference_status='posterior_semantic_reference',
                    estimated_end_s=106,end_sensitivity_range_s=[105.5,106.5],timing_status='estimate',estimated_latency_eligible=True)

    def test_whole_unit_lock_is_distinct_from_semantic_correctness(self):
        source=[dict(kind='source_final',segment_id='s',start_s=2,end_s=8,text='源文')]
        result=dict(duration_s=20,policy=dict(id='p'),events=[dict(kind='translation_final',
                    source_revisions={'s':1},replay_emission_s=12,text='可能错译')])
        row=anchor_row(self.anchor(),result,source)
        self.assertTrue(row['all_related_source_translated'])
        self.assertEqual(row['lock_wait_sensitivity_s'],[5.5,6.5])
        self.assertIsNone(row['semantic_judgment'])

    def test_failure_and_partial_coverage_do_not_get_a_lock_latency(self):
        source=[dict(kind='source_final',segment_id='s',start_s=2,end_s=8,text='源文')]
        result=dict(duration_s=20,policy=dict(id='p'),events=[dict(kind='translation_failed',
                    source_revisions={'s':1},replay_emission_s=12)])
        row=anchor_row(self.anchor(),result,source)
        self.assertFalse(row['all_related_source_translated'])
        self.assertIsNone(row['lock_wait_sensitivity_s'])
        result['duration_s']=5
        self.assertIsNone(anchor_row(self.anchor(),result,source))

    def test_eof_lock_does_not_count_as_natural_semantic_completion(self):
        source=[dict(kind='source_final',segment_id='s',start_s=2,end_s=8,text='源文')]
        result=dict(duration_s=20,policy=dict(id='p'),events=[dict(kind='translation_final',
                    source_revisions={'s':1},replay_emission_s=21,completion_reason='input_eof')])
        row=anchor_row(self.anchor(),result,source)
        self.assertTrue(row['all_related_source_translated'])
        self.assertTrue(row['eof_forced'])
        self.assertFalse(row['ordinary_lock_latency_eligible'])


if __name__=='__main__':unittest.main()
