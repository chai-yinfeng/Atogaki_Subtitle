"""MT finality, source snapshots and audio boundaries are independent."""
import unittest

from retranslation import Retranslation


class RetranslationTests(unittest.TestCase):
    def setUp(self):
        self.events = []
        self.mt = Retranslation(lambda kind, **data: self.events.append(dict(kind=kind, **data)), language='zh', pause=1.2)

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

    def test_default_finalizes_short_mt_group_independently_of_display_retention(self):
        self.mt = Retranslation(lambda kind, **data: self.events.append(dict(kind=kind, **data)), language='zh')
        self.source('a', 'ここで話したい。', final=True, end=1)
        self.mt.advance(2.3)
        request = self.mt.take(0)
        self.assertTrue(request['final'])
        self.assertEqual(set(request['source_revisions']), {'a'})
        self.assertEqual(self.mt.max_seconds, 20)
        self.assertEqual(self.mt.max_chars, 400)

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
                                language='zh', drafts=False, max_seconds=2)
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

    def test_sentence_commits_only_final_prefix_and_uses_tail_as_reference(self):
        self.source('a', '昨日は駅に行きました。', final=True, end=2)
        self.source('b', 'そこで友達に会いました', start=2, end=4)
        request = self.mt.take(0)
        self.assertTrue(request['final'])
        self.assertEqual(request['completion_reason'], 'sentence_with_lookahead')
        self.assertEqual(request['source_text'], '昨日は駅に行きました。')
        self.assertEqual(request['lookahead_source'], 'そこで友達に会いました')
        self.assertEqual(request['source_revisions'], {'a': 1})
        # New tail revisions do not invalidate the immutable Qwen prefix.
        self.source('b', 'そこで先生に会いました。', revision=2, final=True, start=2, end=4)
        self.mt.complete(request, '昨天去了车站。')
        self.mt.advance(4, eof=True)
        tail = self.mt.take(1)
        self.assertNotEqual(tail['group_id'], request['group_id'])
        self.assertEqual(tail['source_text'], 'そこで先生に会いました。')
        self.assertEqual(tail['context'], (('昨日は駅に行きました。', '昨天去了车站。'),))
        self.assertEqual(tail['source_revisions'], {'b': 2})
        self.mt.complete(tail, '在那里遇到了老师。')
        self.assertEqual(self.mt.pending(), 0)
        self.assertEqual(self.mt.segment_groups, {})
        self.assertEqual(self.mt.context[0][0], '昨日は駅に行きました。そこで先生に会いました。')

    def test_timeout_commits_qwen_prefix_without_freezing_the_following_draft(self):
        self.source('a', 'まだ話の途中ですが', final=True, end=1)
        self.source('b', 'これから', start=1, end=5)
        self.mt.advance(3.9, speech=True)
        draft = self.mt.take(0)
        self.assertFalse(draft['final'])
        self.mt.complete(draft, '话还没说完，接下来')
        self.mt.advance(4.1, speech=True)
        request = self.mt.take(1)
        self.assertEqual(request['completion_reason'], 'commit_timeout')
        self.assertTrue(request['final'])
        self.assertEqual(request['source_revisions'], {'a': 1})
        self.assertEqual(request['source_text'], 'まだ話の途中ですが')
        self.mt.complete(request, '话还没说完，')
        self.assertEqual(self.mt.take(2)['source_text'], 'これから')

    def test_timeout_cannot_commit_unfinalized_asr(self):
        self.source('a', 'ずっと草稿', end=10)
        self.mt.advance(10, speech=True)
        request = self.mt.take(0)
        self.assertFalse(request['final'])
        self.mt.complete(request, '仍是草稿')
        self.source('a', '確定した原文', revision=2, final=True, end=10)
        updated = self.mt.take(1)
        self.assertFalse(updated['final'])
        self.mt.complete(updated, '定稿原文的活动译文')
        self.mt.advance(14.1, speech=True)
        request = self.mt.take(2)
        self.assertTrue(request['final'])
        self.assertEqual(request['completion_reason'], 'commit_timeout')

    def test_sentence_split_preserves_eof_on_tail_and_commits_each_source_once(self):
        for i in range(3):
            self.source(str(i), f'これは文章の番号{i}です。', final=True, start=i, end=i+1)
        self.mt.advance(3, eof=True)
        committed = []
        reasons = []
        for i in range(3):
            request = self.mt.take(i)
            self.assertTrue(request['final'])
            committed.extend(request['source_revisions'])
            reasons.append(request['completion_reason'])
            self.mt.complete(request, f'句子{i}')
        self.assertEqual(committed, ['0', '1', '2'])
        self.assertEqual(reasons, ['sentence_with_lookahead', 'sentence_with_lookahead', 'input_eof'])
        self.assertEqual(self.mt.pending(), 0)


class JapaneseBlockTests(unittest.TestCase):
    def setUp(self):
        self.events = []
        self.mt = Retranslation(lambda kind, **data: self.events.append(dict(kind=kind, **data)))

    def source(self, sid, text, start, end, final=True):
        self.mt.observe(sid, start, end, closed=final)
        self.mt.update(dict(segment_id=sid, text=text, source_revision=2 if final else 1,
                            kind='source_final' if final else 'source_draft'))

    def test_short_pause_and_particle_fragments_stay_in_one_source_block(self):
        self.source('a', '今回は。', 0, 1)
        self.mt.advance(2.3)
        draft = self.mt.take(0)
        self.assertFalse(draft['final'])
        self.mt.complete(draft, '这次。')
        self.source('b', '一人で。', 2.4, 3)
        self.source('c', 'のんびりする場所を作ってもらえまして。', 3.2, 5)
        self.source('d', 'まさに憩いの場的な。', 5.2, 6)
        self.mt.advance(6, speech=True)
        draft = self.mt.take(1)
        self.assertFalse(draft['final'])
        self.assertEqual(set(draft['source_revisions']), {'a', 'b', 'c', 'd'})
        self.mt.complete(draft, '这次为我安排了一个可以独自悠闲待着的地方。')
        self.mt.advance(9.1)
        final = self.mt.take(2)
        self.assertTrue(final['final'])
        self.assertEqual(final['completion_reason'], 'long_pause')
        self.assertEqual(final['source_text'], draft['source_text'])

    def test_only_final_right_context_can_commit_a_complete_predicate(self):
        self.source('a', '昨日は駅に行きました。', 0, 2)
        self.source('b', 'そこで友達に会いました。', 2, 4, final=False)
        draft = self.mt.take(0)
        self.assertFalse(draft['final'])
        self.mt.complete(draft, '昨天去了车站，在那里遇到了朋友。')
        self.source('b', 'そこで先生に会いました。', 2, 4)
        final = self.mt.take(1)
        self.assertTrue(final['final'])
        self.assertEqual(final['source_revisions'], {'a': 2})
        self.assertEqual(final['lookahead_source'], 'そこで先生に会いました。')
        self.assertEqual(final['completion_reason'], 'japanese_sentence_with_final_lookahead')
        self.mt.complete(final, '昨天去了车站。')
        self.mt.advance(4, eof=True)
        tail = self.mt.take(2)
        self.assertEqual(tail['source_revisions'], {'b': 2})
        self.mt.complete(tail, '在那里遇到了老师。')
        self.assertEqual(self.mt.pending(), 0)

    def test_late_negation_is_jointly_translated_not_reference_only(self):
        self.source('a', '彼が来ると思っていた。', 0, 2)
        self.source('b', 'わけではないんです。', 2, 4)
        draft = self.mt.take(0)
        self.assertFalse(draft['final'])
        self.assertEqual(draft['source_text'], '彼が来ると思っていた。わけではないんです。')

    def test_closed_block_gets_final_context_across_group_boundary(self):
        self.source('a', '最初の話です。', 0, 1)
        self.mt.advance(4.1)
        self.source('b', '後の原文はまだ草稿。', 4.2, 5, final=False)
        self.source('c', '後の後だけ定稿。', 5.2, 6)
        final = self.mt.take(0)
        self.assertTrue(final['final'])
        self.assertEqual(final['lookahead_source'], '')
        self.mt.complete(final, '第一个话题。')
        self.mt.advance(6, eof=True)
        self.source('b', '後の原文が確定しました。', 4.2, 5)
        tail = self.mt.take(1)
        covered = list(tail['source_revisions'])
        self.mt.complete(tail, '后续原文。')
        while self.mt.pending():
            tail = self.mt.take(2)
            covered.extend(tail['source_revisions'])
            self.mt.complete(tail, '其余原文。')
        self.assertEqual(covered, ['b', 'c'])

    def test_cross_group_reference_is_not_translated_twice(self):
        self.source('a', '最初の話です。', 0, 1)
        self.mt.advance(4.1)
        self.source('b', '別の話を始めます。', 4.2, 5)
        final = self.mt.take(0)
        self.assertEqual(final['lookahead_source'], '別の話を始めます。')
        self.assertEqual(final['source_revisions'], {'a': 2})
        self.mt.complete(final, '第一个话题。')
        self.mt.advance(5, eof=True)
        tail = self.mt.take(1)
        self.assertEqual(tail['source_revisions'], {'b': 2})

    def test_timeout_is_bounded_and_does_not_lock_a_draft(self):
        self.source('a', '話の途中ですが。', 0, 2)
        self.source('b', 'まだ変わる後文。', 2, 10, final=False)
        self.mt.advance(8.1, speech=True)
        final = self.mt.take(0)
        self.assertTrue(final['final'])
        self.assertEqual(final['completion_reason'], 'japanese_timeout')
        self.assertEqual(final['source_revisions'], {'a': 2})
        self.assertEqual(final['lookahead_source'], '')


if __name__ == '__main__':
    unittest.main()
