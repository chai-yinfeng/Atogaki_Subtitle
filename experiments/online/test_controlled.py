import time
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from controlled_clock import FeedClock, next_decode
from controlled_metrics import reference_score, resource_growth
from summarize_controlled import quality_reasons


class ProducerTests(unittest.TestCase):
    def test_causal_feed_continues_while_consumer_is_busy(self):
        clock = FeedClock([0] * 2048, plan=dict(frames=[dict(start_s=i*.032, end_s=(i+1)*.032,
                                  probability=.9, compute_s=0) for i in range(4)]))
        start = time.monotonic()
        clock.start(start)
        try:
            cursor, actions, done = clock.snapshot()
            self.assertLess(cursor, 512)
            time.sleep(.15)  # Represents a blocked consumer; producer is independent.
            cursor, actions, done = clock.snapshot()
            self.assertEqual(cursor, 2048)
            self.assertTrue(done)
            self.assertTrue(all(a['decision_s'] <= cursor / 16000 for a in actions))
        finally:
            clock.close()

    def test_duplicate_decode_triggers_coalesced(self):
        self.assertEqual(next_decode(2.8, .5), 3)
        self.assertEqual(next_decode(3, .5), 3.5)

    def test_cancel_stops_producer_before_future_audio(self):
        clock = FeedClock([0] * 160000)
        clock.start(time.monotonic())
        clock.close()
        self.assertEqual(clock.cursor, 0)
        self.assertFalse(clock.thread.is_alive())


class ReferenceTests(unittest.TestCase):
    def events(self, texts):
        return [dict(kind='display', slot=0, emission_s=i+1, text=t) for i,t in enumerate(texts)]

    def anchor(self, **updates):
        return dict(dict(id='a', text='否定', audio_end_s=.5, text_status='model_reference', timing_status='provider_estimate'), **updates)

    def test_model_reference_cannot_become_verified_latency(self):
        r = reference_score(self.events(['否定']), [self.anchor()])
        self.assertEqual(r['verified_text']['eligible'], 0)
        self.assertEqual(r['model_reference_diagnostic']['matched'], 1)

    def test_missing_anchor_kept_in_coverage(self):
        r = reference_score(self.events(['别的话']), [self.anchor(text_status='verified_verbatim', timing_status='verified')])
        self.assertEqual(r['verified_text']['coverage'], 0)
        self.assertEqual(r['verified_text']['missing_or_ambiguous'], 1)

    def test_last_correction_is_stability_time(self):
        r = reference_score(self.events(['否定', '肯定', '否定']), [self.anchor(text_status='verified_verbatim', timing_status='verified')])
        self.assertEqual(r['anchors'][0]['first_s'], 1)
        self.assertEqual(r['anchors'][0]['stable_s'], 3)

    def test_repeated_match_is_ambiguous(self):
        r = reference_score(self.events(['否定否定']), [self.anchor(text_status='verified_verbatim', timing_status='verified')])
        self.assertTrue(r['anchors'][0]['ambiguous'])
        self.assertEqual(r['verified_text']['coverage'], 0)

    def test_semantic_acceptance_requires_explicit_variants_and_time(self):
        r = reference_score(self.events(['不赞同']), [self.anchor(semantic_status='verified', timing_status='verified', acceptable_texts=['不赞同'])])
        self.assertEqual(r['verified_semantic']['matched'], 1)
        self.assertEqual(r['verified_text']['eligible'], 0)

    def test_semantic_review_does_not_approve_original_model_error(self):
        r = reference_score(self.events(['否定']), [self.anchor(semantic_status='verified', timing_status='verified', acceptable_texts=['不赞同'])])
        self.assertEqual(r['verified_semantic']['coverage'], 0)
        self.assertEqual(r['model_reference_diagnostic']['matched'], 1)

    def test_paraphrase_does_not_pass_verbatim_gate(self):
        r = reference_score(self.events(['不赞同']), [self.anchor(text_status='verified_verbatim', semantic_status='verified', timing_status='verified', acceptable_texts=['不赞同'])])
        self.assertEqual(r['verified_text']['coverage'], 0)
        self.assertEqual(r['verified_semantic']['coverage'], 1)
        self.assertEqual(r['effective']['coverage'], 1)

    def test_uncertain_anchor_excluded_but_missing_verified_anchor_retained(self):
        r = reference_score(self.events(['别的话']), [self.anchor(text_status='verified_verbatim', timing_status='verified'), self.anchor(id='b', timing_status='uncertain')])
        self.assertEqual(r['effective']['eligible'], 1)
        self.assertEqual(r['effective']['missing_or_ambiguous'], 1)
        self.assertEqual(r['effective']['excluded_uncertain'], 1)
        self.assertEqual(r['effective']['pending'], 0)


class ResourceTests(unittest.TestCase):
    def test_parent_rss_growth_does_not_sum_children(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            folder = root/'long'
            rows = [dict(run='long',run_age_s=t,processes=[dict(role='runner',rss_bytes=rss),dict(role='child',rss_bytes=100)])
                    for t,rss in [(90,10),(150,12),(510,20),(570,22)]]
            (root/'resource-samples.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in rows))
            r = resource_growth(folder,dict(audio_duration_s=600))
            self.assertEqual(r['runner_rss_delta_bytes'],10)
            self.assertEqual(r['runner_peak_sampled_bytes'],22)

    def test_partial_series_does_not_invent_memory_growth(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root/'resource-samples.jsonl').write_text(json.dumps(dict(run='long',run_age_s=570,processes=[dict(role='runner',rss_bytes=22)]))+'\n')
            r = resource_growth(root/'long',dict(audio_duration_s=600))
            self.assertIsNone(r['runner_rss_delta_bytes'])


class BaselineReviewTests(unittest.TestCase):
    def test_gemini_correctness_cannot_replace_large_v3_review(self):
        ref = dict(anchors=[dict(id='a',critical_error=False,semantic_observations={'run':'correct'})])
        self.assertTrue(quality_reasons(ref,[dict(run='run')]))

    def test_additional_critical_error_relative_to_reviewed_baseline(self):
        ref = dict(anchors=[dict(id='a',semantic_observations={'run':'critical_error'})],
                   large_v3_baseline=dict(source_sha256='bound',review_status='reviewed',observations={'a':'correct'}))
        self.assertTrue(quality_reasons(ref,[dict(run='run')]))
        ref['large_v3_baseline']['observations']['a'] = 'critical_error'
        self.assertEqual(quality_reasons(ref,[dict(run='run')]),[])


if __name__ == '__main__':
    unittest.main()
