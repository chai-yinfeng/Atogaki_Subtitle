import unittest
from draft_anchor_review import collect, digest, summarize


class DraftReviewTests(unittest.TestCase):
    def packet(self, versions):
        anchor=dict(id='A',case_id='c',core_range_s=[100,120],prior_media_range_s=[102,104],
                    estimated_end_s=104,end_sensitivity_range_s=[103.8,104.2],
                    estimated_latency_eligible=True,target_semantic_reference='fact')
        source=dict(kind='source_final',segment_id='s',start_s=2,end_s=5,text='source')
        group=lambda t,text,kind='translation_draft':dict(kind=kind,group_id='g',emission_s=t,
                    source_revisions={'s':1},text=text)
        snapshots=[dict(time_s=t,translations=[] if text is None else [group(t,text,kind)])
                   for t,text,kind in versions]
        finals=[g for s in snapshots for g in s['translations'] if g['kind'] in ('translation_final','translation_failed')]
        return collect(anchor,[source,*finals],dict(snapshots=snapshots))

    def observe(self,p,correct):
        return dict(packet_sha256=digest(p),judgments={k:dict(judgment='correct' if v['texts']==[correct] else 'incorrect',
                    reviewer='test_fixture') for k,v in p['variants'].items()})

    def test_correct_draft_can_regress_into_wrong_final(self):
        p=self.packet([(5,'right','translation_draft'),(7,'wrong','translation_final')])
        r=summarize(p,self.observe(p,'right'))
        self.assertEqual(r['first_correct_wait_s'],1)
        self.assertEqual(len(r['regressions']),1)
        self.assertFalse(r['final_correct'])

    def test_repeated_text_keeps_distinct_draft_and_lock_time(self):
        p=self.packet([(5,'right','translation_draft'),(7,'right','translation_final')])
        self.assertEqual(len(p['variants']),1)
        r=summarize(p,self.observe(p,'right'))
        self.assertEqual(r['first_correct_visible_s'],5)
        self.assertEqual(r['final_lock_s'],7)
        self.assertTrue(r['final_correct'])

    def test_unshown_return_is_not_first_visible(self):
        p=self.packet([(7,'wrong','translation_final')])
        self.assertIsNone(summarize(p,self.observe(p,'right'))['first_correct_visible_s'])

    def test_gap_is_retained_and_never_counts_as_correct(self):
        p=self.packet([(5,'right','translation_draft'),(6,None,''),(8,'wrong','translation_final')])
        self.assertIsNone(p['timeline'][1]['variant'])
        self.assertEqual(len(summarize(p,self.observe(p,'right'))['regressions']),1)

    def test_review_cannot_be_reused_after_packet_changes(self):
        p=self.packet([(5,'right','translation_final')]);o=self.observe(p,'right')
        p['timeline'][0]['time_s']=6
        with self.assertRaises(ValueError):summarize(p,o)

    def test_missing_judgment_is_not_assumed_wrong_or_correct(self):
        p=self.packet([(5,'right','translation_final')]);o=self.observe(p,'right');o['judgments']={}
        with self.assertRaises(ValueError):summarize(p,o)

    def test_failed_or_missing_coverage_never_certifies_final(self):
        for versions in [[(5,'right','translation_draft')],[(5,'right','translation_failed')]]:
            p=self.packet(versions);self.assertFalse(summarize(p,self.observe(p,'right'))['final_correct'])
        p=self.packet([(5,'right','translation_failed')])
        self.assertIsNone(summarize(p,self.observe(p,'right'))['first_correct_visible_s'])

    def test_contract_issue_is_not_usable_even_with_semantic_label(self):
        p=self.packet([(5,'right','translation_final')]);o=self.observe(p,'right')
        next(iter(o['judgments'].values()))['output_contract_issue']=True
        self.assertIsNone(summarize(p,o)['first_correct_visible_s'])

    def test_invalid_display_clock_fails(self):
        with self.assertRaises(ValueError):self.packet([(7,'a','translation_draft'),(5,'b','translation_final')])

    def test_early_prediction_keeps_signed_delay_and_sensitivity(self):
        p=self.packet([(3,'right','translation_final')]);r=summarize(p,self.observe(p,'right'))
        self.assertEqual(r['first_correct_wait_s'],-1)
        self.assertAlmostEqual(r['first_correct_wait_sensitivity_s'][0],-1.2)

    def test_multiple_related_groups_are_reviewed_together(self):
        anchor=dict(id='A',case_id='c',core_range_s=[100,120],prior_media_range_s=[102,104],
                    estimated_end_s=104,end_sensitivity_range_s=[103.8,104.2],
                    estimated_latency_eligible=True,target_semantic_reference='fact')
        sources=[dict(kind='source_final',segment_id=sid,start_s=2,end_s=5) for sid in ['a','b']]
        groups=[dict(kind='translation_final',group_id=sid,emission_s=6,source_revisions={sid:1},text=sid)
                for sid in ['a','b']]
        p=collect(anchor,[*sources,*groups],dict(snapshots=[dict(time_s=6,translations=groups)]))
        self.assertEqual(next(iter(p['variants'].values()))['texts'],['a','b'])
        self.assertTrue(p['final_source_coverage_complete'])
        p=collect(anchor,[*sources,groups[0]],dict(snapshots=[dict(time_s=6,translations=groups[:1])]))
        self.assertFalse(p['final_source_coverage_complete'])


if __name__=='__main__':unittest.main()
