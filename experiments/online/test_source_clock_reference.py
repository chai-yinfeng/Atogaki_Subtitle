import copy
from functools import partial
import json
import unittest
from unittest.mock import patch

from compare_japanese_mt import replay
from retranslation import Retranslation
from source_clock_reference import compile_plan, consume, signature


def race(second=6.1):
    return [dict(kind='segment_closed', emission_s=1, segment_id='a', start_s=0, end_s=1),
            dict(kind='source_final', emission_s=2, segment_id='a', start_s=0, end_s=1,
                 source_revision=1, text='まだ話の途中で。'),
            dict(kind='source_draft', emission_s=3, segment_id='b', start_s=1.5, end_s=3,
                 source_revision=1, text='つなぎ'),
            dict(kind='source_draft', emission_s=5, segment_id='b', start_s=1.5, end_s=5,
                 source_revision=2, text='つなぎ込む'),
            dict(kind='segment_closed', emission_s=5.8, segment_id='b', start_s=1.5, end_s=5.8),
            dict(kind='source_final', emission_s=second, segment_id='b', start_s=1.5, end_s=5.8,
                 source_revision=3, text='つなぎ込む未来の話です。')]


def finals(plan):
    return [e for e in plan['events'] if e['kind']=='snapshot' and e['final']]


def membership(rows):
    return [tuple(row['source_revisions']) for row in rows]


class SourceClockTests(unittest.TestCase):
    def test_original_take_depends_on_compute_even_with_fixed_arrivals(self):
        results=[]
        for seconds in (0, .5, 2, 6):
            elapsed=[0.0]
            class Translator:
                def translate(self, source, context=(), lookahead_source=''):
                    elapsed[0]+=seconds
                    return '译文'
            with patch('compare_japanese_mt.time.monotonic',side_effect=lambda: elapsed[0]):
                result=replay(race(),partial(Retranslation,commit_wait=4),Translator(),10)
            results.append(tuple(tuple(e['source_revisions']) for e in result['events'] if e['kind']=='translation_final'))
        self.assertGreater(len(set(results)),1)

    def test_reference_membership_does_not_follow_the_model_slot(self):
        plan=compile_plan(race(),10,commit_wait=4)
        before=json.dumps(plan,ensure_ascii=False)
        self.assertEqual(membership(finals(plan)),[('a',),('b',)])
        inputs=[]
        for seconds in (0,.5,2,6,12):
            result=consume(plan,lambda _s,_a: (seconds,False))
            accepted=[r for r in result['requests'] if r['snapshot']['final'] and r['accepted']]
            self.assertEqual(membership([r['snapshot'] for r in accepted]),[('a',),('b',)])
            inputs.append([(signature(r['snapshot']),r['context']) for r in accepted])
            for r in accepted:
                self.assertGreaterEqual(r['start_s'],r['snapshot']['emission_s'])
        self.assertTrue(all(value==inputs[0] for value in inputs))
        self.assertEqual(json.dumps(plan,ensure_ascii=False),before)

    def test_source_at_deadline_precedes_timeout(self):
        plan=compile_plan(race(6),10,commit_wait=4)
        self.assertEqual(membership(finals(plan)),[('a','b')])

    def test_late_final_after_eof_is_not_read_early(self):
        plan=compile_plan(race(12),10,commit_wait=4)
        b=next(e for e in finals(plan) if 'b' in e['source_revisions'])
        self.assertEqual(b['emission_s'],12)
        result=consume(plan,lambda _s,_a: (2,False))
        self.assertTrue(all(r['start_s']>=12 for r in result['requests'] if r['snapshot']['final'] and 'b' in r['snapshot']['source_revisions']))

    def test_final_right_context_is_not_replaced_by_future_arrivals(self):
        plan=compile_plan(race(),10,commit_wait=4)
        a=finals(plan)[0]
        self.assertEqual(a['lookahead_source'],'')
        result=consume(plan,lambda _s,_a: (12,False))
        job=next(r for r in result['requests'] if r['snapshot']['final'] and 'a' in r['snapshot']['source_revisions'])
        self.assertGreater(job['start_s'],6.1)
        self.assertEqual(job['snapshot']['lookahead_source'],'')

    def test_stale_inflight_draft_cannot_resurface_after_split(self):
        result=consume(compile_plan(race(),10,commit_wait=4),lambda _s,_a: (6,False))
        stale=[r for r in result['requests'] if not r['snapshot']['final'] and not r['accepted']]
        self.assertTrue(stale)
        self.assertEqual(sum(e['kind']=='translation_final' for e in result['events']),2)

    def test_final_failure_is_bounded_and_excluded_from_history(self):
        def cost(snap,attempt):
            return .2, bool(snap['final'] and 'a' in snap['source_revisions'])
        result=consume(compile_plan(race(),10,commit_wait=4),cost)
        attempts=[r for r in result['requests'] if r['snapshot']['final'] and 'a' in r['snapshot']['source_revisions']]
        self.assertEqual(len(attempts),2)
        final_events=[e for e in result['events'] if e['kind']!='translation_draft']
        self.assertEqual([e['kind'] for e in final_events],['translation_failed','translation_final'])
        b=next(r for r in result['requests'] if r['snapshot']['final'] and 'b' in r['snapshot']['source_revisions'])
        self.assertEqual(b['context'],())

    def test_draft_failure_waits_for_new_signature(self):
        result=consume(compile_plan(race(),10,commit_wait=4),lambda snap,_a: (.1,not snap['final']))
        drafts=[r for r in result['requests'] if not r['snapshot']['final']]
        keys=[(r['snapshot']['group_id'],signature(r['snapshot'])) for r in drafts]
        self.assertEqual(len(keys),len(set(keys)))

    def test_pending_limit_fails_explicitly_without_mutating_plan(self):
        plan=compile_plan(race(),10,commit_wait=4)
        before=copy.deepcopy(plan)
        with self.assertRaisesRegex(ValueError,'pending limit'):
            consume(plan,lambda _s,_a: (20,False),pending_limit=1)
        self.assertEqual(plan,before)

    def test_closed_cap_membership_cannot_be_split_by_late_source(self):
        events=[]
        for sid,start,end,arrive in [('a',0,6,8),('b',6.5,12,14),('c',12.5,15,16)]:
            events.extend([dict(kind='segment_closed',emission_s=end,segment_id=sid,start_s=start,end_s=end),
                           dict(kind='source_final',emission_s=arrive,segment_id=sid,start_s=start,end_s=end,
                                source_revision=1,text='これは完全な文章です。')])
        events.sort(key=lambda e:e['emission_s'])
        plan=compile_plan(events,20,max_seconds=10)
        first=finals(plan)[0]
        self.assertEqual(tuple(first['source_revisions']),('a','b'))
        self.assertEqual(first['completion_reason'],'max_duration')

    def test_stale_source_draft_after_final_does_not_change_ready_job(self):
        events=race()+[dict(kind='source_draft',emission_s=9,segment_id='a',start_s=0,end_s=1,
                            source_revision=0,text='過去の草稿')]
        self.assertEqual(finals(compile_plan(events,10,commit_wait=4)),finals(compile_plan(race(),10,commit_wait=4)))

    def test_history_keeps_only_complete_bounded_pairs(self):
        events=[]
        for i in range(12):
            events.extend([dict(kind='segment_closed',emission_s=i*5+1,segment_id=str(i),start_s=i*5,end_s=i*5+1),
                           dict(kind='source_final',emission_s=i*5+1.2,segment_id=str(i),start_s=i*5,end_s=i*5+1,
                                source_revision=1,text='長い原文'*90)])
        result=consume(compile_plan(events,60),lambda _s,_a: (0,False))
        self.assertLessEqual(len(result['final_history']),8)
        self.assertLessEqual(sum(len(a)+len(b) for a,b in result['final_history']),2000)
        self.assertEqual(sum(e['kind']=='translation_final' for e in result['events']),12)

    def test_empty_earlier_group_blocks_out_of_order_later_commit(self):
        events=[dict(kind='segment_closed',emission_s=1,segment_id='a',start_s=0,end_s=1),
                dict(kind='segment_closed',emission_s=6,segment_id='b',start_s=5,end_s=6),
                dict(kind='source_final',emission_s=6.2,segment_id='b',start_s=5,end_s=6,
                     source_revision=1,text='後の文章です。'),
                dict(kind='source_final',emission_s=9,segment_id='a',start_s=0,end_s=1,
                     source_revision=1,text='先の文章です。')]
        result=consume(compile_plan(events,10),lambda _s,_a: (0,False))
        commits=[e['source_ids'] for e in result['events'] if e['kind']=='translation_final']
        self.assertEqual(commits,[['a'],['b']])
        self.assertTrue(all(r['start_s']>=9 for r in result['requests']))

    def test_fractional_interval_boundary_does_not_stall(self):
        plan=compile_plan(race(),10,commit_wait=4)
        result=consume(plan,lambda _s,_a: (0,False))
        self.assertLess(result['finished_s'],12)
        starts=[r['start_s'] for r in result['requests'] if not r['snapshot']['final']]
        self.assertTrue(all(b-a>=.8-1e-9 for a,b in zip(starts,starts[1:])))

    def test_bad_source_and_cost_are_rejected(self):
        with self.assertRaises(ValueError):compile_plan(list(reversed(race())),10)
        with self.assertRaises(ValueError):compile_plan(race(),10,commit_wait=-1)
        with self.assertRaises(ValueError):compile_plan(race(),10,max_seconds=float('nan'))
        with self.assertRaises(ValueError):consume(compile_plan(race(),10),lambda _s,_a: (float('nan'),False))


if __name__=='__main__':
    unittest.main()
