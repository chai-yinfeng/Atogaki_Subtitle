import unittest
from reference_ledger import build_case


class ReferenceLedgerTests(unittest.TestCase):
    def case(self, segments, lines=(), decisions=()):
        return build_case(dict(id='c', start_ms=10000, end_ms=20000, context_start_ms=5000),
                          dict(result=dict(segments=segments)), lines, decisions)

    def test_approval_is_exact_and_does_not_approve_timing(self):
        s = dict(start=6, end=7, text='確認')
        d = dict(case_id='c', source_text_status='user_approved', source_segments=[s])
        rows = self.case([s, dict(start=8, end=9, text='確認')], decisions=[d])['cues']
        self.assertEqual([r['text_status'] for r in rows], ['user_approved', 'model_reference_unreviewed'])
        self.assertTrue(all(not r['precise_latency_eligible'] for r in rows))

    def test_uncertain_range_overrides_text_approval(self):
        s = dict(start=6, end=7, text='確認')
        decisions = [dict(case_id='c', source_text_status='user_approved', source_segments=[s]),
                     dict(case_id='c', conservative_exclusion_media_range_s=[11.5, 12.5])]
        row = self.case([s], decisions=decisions)['cues'][0]
        self.assertEqual(row['text_status'], 'uncertain')
        self.assertTrue(row['excluded'])

    def test_boundary_is_not_interpolated_or_outside_counted(self):
        rows = self.case([dict(start=3, end=6, text='跨界'), dict(start=15, end=16, text='窗外')])['cues']
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['media_range_s'], [8, 11])
        self.assertEqual(rows[0]['core_overlap_s'], [10, 11])
        self.assertTrue(rows[0]['crosses_core_boundary'])

    def test_display_links_remain_many_to_many_and_unmapped(self):
        def line(id, start, end):
            return dict(id=id, case_id='c', first_seen_s=start, last_seen_s=end,
                        language_hint='zh_or_kanji_ambiguous', text='字幕',
                        timing_status='sampled_display_interval_not_acoustic',
                        reference_status='ocr_candidate_unreviewed')
        rows = self.case([dict(start=6, end=8, text='甲'), dict(start=8, end=10, text='乙')],
                         [line('one', 11, 15), line('two', 12, 14), line('none', 18, 19)])
        self.assertEqual([len(r['source_cue_ids']) for r in rows['display_evidence']], [2, 2, 0])
        self.assertEqual(rows['unmapped_display_count'], 1)
        self.assertTrue(all('candidate' in r['relation_status'] for r in rows['display_evidence']))


if __name__ == '__main__':
    unittest.main()
