import threading
import unittest

from live_asr_schedule import ASRMailbox, replay_metrics


def job(sid='a', end=16000):
    return dict(sid=sid, end=end, samples=None, queued_at=0)


class LiveASRScheduleTests(unittest.TestCase):
    def test_latest_coalesces_and_final_takes_priority(self):
        mailbox = ASRMailbox()
        mailbox.preview(job(end=1))
        mailbox.preview(job(end=2))
        mailbox.close(job(end=3))
        mailbox.preview(job('b', 1))
        first = mailbox.take()
        self.assertEqual((first['sid'], first['end'], first['final']), ('a', 3, True))
        mailbox.complete(first, {'text': 'locked'})
        self.assertEqual(mailbox.take()['sid'], 'b')
        self.assertEqual(mailbox.replaced, 1)
        with self.assertRaises(ValueError):
            mailbox.close(job(end=3))

    def test_end_of_audio_upgrades_inflight_without_second_inference(self):
        mailbox = ASRMailbox()
        mailbox.preview(job())
        active = mailbox.take()
        mailbox.close(job())
        self.assertTrue(mailbox.complete(active, {'text': 'same audio'}))
        self.assertIsNone(mailbox.take())
        self.assertEqual(mailbox.final_upgrades, 1)

    def test_completed_full_audio_result_is_reused(self):
        mailbox = ASRMailbox()
        mailbox.preview(job())
        active = mailbox.take()
        result = {'text': 'already available'}
        self.assertFalse(mailbox.complete(active, result))
        mailbox.preview(job())
        self.assertIs(mailbox.close(job()), result)
        self.assertIsNone(mailbox.take())
        mailbox.preview(job())
        self.assertIsNone(mailbox.take())
        self.assertEqual(mailbox.final_reuses, 1)

    def test_abort_wakes_waiter_and_discards_work(self):
        mailbox = ASRMailbox()
        done = threading.Event()
        result = []
        def wait():
            result.append(mailbox.wait())
            done.set()
        thread = threading.Thread(target=wait)
        thread.start()
        mailbox.finish(abort=True)
        self.assertTrue(done.wait(1))
        thread.join()
        self.assertEqual(result, [False])
        mailbox.preview(job())
        self.assertIsNone(mailbox.take())

    def test_metric_counts_are_not_semantic_accuracy(self):
        events = [dict(kind='crop_start', segment_id='a', start_s=0, duration_s=4),
                  dict(kind='source_draft', segment_id='a', text='abc', emission_s=1.2),
                  dict(kind='source_draft', segment_id='a', text='abcd', emission_s=2.2),
                  dict(kind='source_final', segment_id='a', text='abX', emission_s=4.4),
                  dict(kind='translation_final', segment_id='a', emission_s=5, queue_wait_s=.1)]
        row = replay_metrics(events)['crops']['a']
        self.assertEqual(row['rewritten_suffix_characters'], 2)
        self.assertAlmostEqual(row['final_source_after_end_s'], .4)
        self.assertEqual(row['translation_after_end_s'], 1)
        self.assertEqual(row['post_final_source_events'], 0)


if __name__ == '__main__':
    unittest.main()
