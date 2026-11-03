"""MT finality, source snapshots and audio boundaries are independent."""
import unittest

from retranslation import Retranslation


class RetranslationTests(unittest.TestCase):
    def setUp(self):
        self.events = []
        self.mt = Retranslation(lambda kind, **data: self.events.append(dict(kind=kind, **data)), pause=1.2)

    def source(self, sid, text, revision=1, final=False, start=0, end=1):
        self.mt.observe(sid, start, end, closed=final)
        event = dict(kind='source_final' if final else 'source_draft',
                     segment_id=sid, text=text, source_revision=revision)
        self.mt.update(event)
        return event

    def test_vad_and_asr_final_do_not_commit_translation(self):
        self.source('a', '彼が来ると', final=True)
        self.mt.advance(1.5)
        self.source('b', '思っていたわけではない。', final=True, start=1.5, end=2.5)
        request = self.mt.take(10)
        self.assertFalse(request['final'])
        self.assertEqual(request['source_text'], '彼が来ると思っていたわけではない。')
        self.mt.complete(request, '我并不是以为他会来。')
        self.mt.advance(3.8)
        request = self.mt.take(11)
        self.assertTrue(request['final'])
        self.assertEqual(len(request['source_revisions']), 2)
        self.mt.complete(request, self.mt.cached(request))
        self.assertEqual(self.events[-1]['kind'], 'translation_final')
        self.assertFalse(self.events[-1]['stale_source'])

    def test_asr_period_does_not_freeze_a_fragment_before_continuation(self):
        self.source('a', '一人で。', final=True)
        self.mt.advance(1.7)
        request = self.mt.take(0)
        self.assertFalse(request['final'])
        self.mt.complete(request, '独自一人。')
        self.source('b', 'のんびり過ごす。', final=True, start=1.8, end=3)
        self.mt.advance(4.3)
        request = self.mt.take(1)
        self.assertTrue(request['final'])
        self.assertEqual(request['source_text'], '一人で。のんびり過ごす。')
        self.assertEqual(set(request['source_revisions']), {'a', 'b'})
        self.assertEqual(request['completion_reason'], 'long_pause')

    def test_live_default_keeps_short_asr_segments_in_one_larger_translation_group(self):
        self.mt = Retranslation(lambda kind, **data: self.events.append(dict(kind=kind, **data)))
        self.source('a', 'ここで。', final=True, end=1)
        self.mt.advance(3.5)
        self.source('b', '一人で。', final=True, start=3.6, end=4.5)
        self.mt.advance(7.0)
        self.source('c', '話したい。', final=True, start=7.1, end=8)
        self.mt.advance(11.1)
        request = self.mt.take(0)
        self.assertTrue(request['final'])
        self.assertEqual(set(request['source_revisions']), {'a', 'b', 'c'})
        self.assertEqual(self.mt.max_seconds, 45)
        self.assertEqual(self.mt.max_chars, 900)

    def test_late_translation_retains_snapshot_then_latest_replaces_it(self):
        self.source('a', '来る', revision=1)
        old = self.mt.take(0)
        self.source('a', '来ない', revision=2)
        self.mt.complete(old, '会来')
        self.assertTrue(self.events[-1]['stale_source'])
        self.assertEqual(self.events[-1]['source_revisions'], {'a': 1})
        self.assertIsNone(self.mt.take(.2))
        latest = self.mt.take(1)
        self.assertEqual(latest['source_text'], '来ない')
        self.mt.complete(latest, '不来')
        self.assertFalse(self.events[-1]['stale_source'])

    def test_closed_group_waits_for_all_finals_and_context_is_bounded(self):
        self.source('a', 'draft')
        self.mt.observe('a', 0, 1, closed=True)
        self.mt.advance(3)
        self.assertIsNone(self.mt.take(0))
        self.source('a', 'final', revision=2, final=True)
        self.mt.complete(self.mt.take(1), '定稿')
        self.source('b', 'next', start=3, end=4)
        request = self.mt.take(2)
        self.assertEqual(request['context'], (('final', '定稿'),))
        self.assertEqual(request['context_revision'], 1)
        self.assertNotEqual(request['group_id'], self.events[-1]['group_id'])

    def test_status_only_change_reuses_identical_translation(self):
        draft = self.source('a', 'same')
        request = self.mt.take(0)
        self.mt.complete(request, '相同')
        self.source('a', 'same', revision=2, final=True)
        self.mt.advance(1, eof=True)
        request = self.mt.take(1)
        self.assertTrue(request['final'])
        self.assertEqual(self.mt.cached(request), '相同')
        self.mt.complete(request, '相同')
        self.mt.update(draft)  # Late callback after commit cannot revive the group.
        self.assertEqual(self.mt.pending(), 0)
        with self.assertRaisesRegex(ValueError, 'not in flight'):
            self.mt.complete(request, 'late')

    def test_duration_bound_at_segment_boundary_and_final_only_mode(self):
        self.mt = Retranslation(lambda kind, **data: self.events.append(dict(kind=kind, **data)),
                                drafts=False, max_seconds=2)
        self.source('a', 'unfinished', final=True, end=3)
        request = self.mt.take(0)
        self.assertTrue(request['final'])
        self.assertEqual(request['completion_reason'], 'max_duration')
        self.mt.complete(request, '未完')
        self.source('b', 'next', start=3, end=4)
        self.assertIsNone(self.mt.take(1))
        self.source('b', 'next', revision=2, final=True, start=3, end=4)
        self.mt.advance(4, eof=True)
        self.assertTrue(self.mt.take(2)['final'])


if __name__ == '__main__':
    unittest.main()
