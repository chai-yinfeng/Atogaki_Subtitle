import io
from contextlib import redirect_stdout
import json
from pathlib import Path
import tempfile
import unittest

import _worker_path
from online_worker.mt import HyMT2, TranslationError
from online_worker.retranslation import Retranslation
from online_worker.worker import Events, complete_translation
from online_worker.drain_budget import DrainBudget


class MTFailureTests(unittest.TestCase):
    def test_request_uses_remaining_budget_and_late_response_cannot_commit(self):
        now = [0]
        budget = DrainBudget(monotonic=lambda: now[0])
        budget.begin(3)
        mt = Retranslation(lambda *_a, **_k: None)
        self.source(mt, 'a', '原文', final=True)
        mt.advance(1, eof=True)
        class Late:
            def translate(self, *args):
                now[0] = 4
                return '迟到译文'
        translator = Late()
        with redirect_stdout(io.StringIO()):
            events = Events()
            complete_translation(translator, mt, mt.take(0), events, budget=budget)
        self.assertEqual(translator.timeout, 3)
        self.assertNotIn('translation_final', events.counts)
        self.assertEqual(events.counts['translation_error'], 1)
        self.assertIsNone(mt.cache)

    def test_rejected_response_preserves_finish_reason_and_body(self):
        adapter = HyMT2('http://127.0.0.1:9/v1', 'local', 'Chinese')
        class Client:
            content, reason = '正文', 'length'
            def open(self, request, timeout):
                return io.StringIO(json.dumps(dict(choices=[dict(finish_reason=self.reason,
                    message=dict(content=self.content))])))
        client = Client(); adapter.client = client
        with self.assertRaises(TranslationError) as raised:
            adapter.translate('原文')
        self.assertEqual(raised.exception.finish_reason, 'length')
        self.assertEqual(raised.exception.output, '正文')
        client.reason = 'stop'; client.content = '译文\n〖约束要求〗\n规则'
        with self.assertRaisesRegex(TranslationError, 'prompt structure'):
            adapter.translate('原文')
        client.content = '[Previous source context]\n泄漏的背景'
        with self.assertRaisesRegex(TranslationError, 'prompt structure'):
            adapter.translate('原文')
        client.content = '译文\n{"上文": [{"译法参考": "污染"}]}'
        with self.assertRaisesRegex(TranslationError, 'prompt structure'):
            adapter.translate('原文')
        client.content = '长' * 301
        with self.assertRaisesRegex(TranslationError, 'expansion'):
            adapter.translate('原文')
        client.content = '〖约束要求〗'
        self.assertEqual(adapter.translate('〖约束要求〗'), client.content)

    def source(self, mt, sid, text, revision=1, final=False, start=0, end=1):
        mt.observe(sid, start, end, closed=final)
        mt.update(dict(kind='source_final' if final else 'source_draft', segment_id=sid,
                       text=text, source_revision=revision))

    def test_draft_failure_waits_for_revision_and_stale_final_does_not_retire(self):
        events=[]; mt=Retranslation(lambda kind, **data: events.append(dict(kind=kind, **data)))
        self.source(mt, 's', '草稿')
        request=mt.take(10); self.assertFalse(mt.fail(request))
        self.assertIsNone(mt.take(11))
        self.source(mt, 's', '新草稿', revision=2)
        request=mt.take(12); self.assertIsNotNone(request); mt.fail(request)
        self.source(mt, 's', '定稿', revision=3, final=True)
        mt.advance(1, eof=True); request=mt.take(13)
        # A newer finalized revision can arrive before an older final response;
        # retain the group and don't spend its retry budget.
        mt.groups[0].revision += 1
        self.assertFalse(mt.fail(request)); self.assertEqual(mt.pending(), 1)
        self.assertEqual(mt.groups[0].final_failures, 0)

    def test_two_final_failures_keep_asr_and_later_groups_without_bad_history(self):
        self.addCleanup(lambda: quiet.__exit__(None, None, None))
        quiet=redirect_stdout(io.StringIO()); quiet.__enter__()
        events=Events(io.StringIO()); mt=Retranslation(events.emit)
        self.source(mt, 'a', '失败原文', final=True); mt.advance(1, eof=True)
        class Broken:
            def translate(self, *args):
                raise TranslationError('bad response', finish_reason='length', output='不可信正文')
        with tempfile.TemporaryDirectory() as folder:
            for now in (10, 11):
                complete_translation(Broken(), mt, mt.take(now), events, Path(folder))
            rows=[json.loads(line) for line in (Path(folder)/'translation-rejections.jsonl').read_text().splitlines()]
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]['finish_reason'], 'length')
        self.assertEqual(rows[0]['rejected_output'], '不可信正文')
        self.assertEqual(mt.pending(), 0); self.assertEqual(mt.context, (('失败原文', ''),))
        self.assertIsNone(mt.cache)
        self.assertEqual(events.counts['translation_failed'], 1)
        self.assertNotIn('translation_final', events.counts)
        self.source(mt, 'b', '后续原文', final=True, start=2, end=3)
        mt.advance(3, eof=True)
        class Good:
            def translate(self, *args): return '后续译文'
        # Register source finality just as the worker does before MT complete.
        events.final['b']=1
        complete_translation(Good(), mt, mt.take(12), events)
        self.assertEqual(mt.context, (('失败原文', ''), ('后续原文', '')))
        self.assertEqual(mt.pending(), 0)


if __name__ == '__main__':
    unittest.main()
