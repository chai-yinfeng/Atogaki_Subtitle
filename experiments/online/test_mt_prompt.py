"""Committed text and lookahead stay reference-only in the MT request."""
import io
import json
import unittest
from harness import HyMT2


class PromptTests(unittest.TestCase):
    def test_prompt_budget_removes_whole_old_pairs(self):
        adapter = HyMT2('http://127.0.0.1:9999/v1', 'local', 'Chinese')
        prompt = adapter._prompt('当前原文。', (('古'*700, '旧'*700), ('新'*200, '译'*200)))
        self.assertLessEqual(len(prompt.encode('utf-8')), 6000)
        self.assertNotIn('古', prompt)
        self.assertNotIn('旧', prompt)
        self.assertIn('Source: '+'新'*200, prompt)
        self.assertIn('Translation (wording reference only): '+'译'*200, prompt)

    def test_scene_names_and_old_translations_have_distinct_roles(self):
        adapter = HyMT2('http://127.0.0.1:9999/v1', 'local', 'Chinese',
                       protected_terms=['n-buna', 'unused'], scene='个人电台，讲述过去经历。')
        requests = []

        class Client:
            def open(self, request, timeout):
                requests.append(json.loads(request.data))
                return io.StringIO(json.dumps({'choices': [{
                    'finish_reason': 'stop', 'message': {'content': '译文'}}]}))

        adapter.client = Client()
        adapter.translate('n-bunaさんです。', (('先前原文。', '可能错误的译法。'),))
        prompt = requests[0]['messages'][0]['content']
        self.assertIn('[User-provided scene]\n个人电台，讲述过去经历。', prompt)
        self.assertIn('preserve verbatim]\nn-buna', prompt)
        self.assertNotIn('unused', prompt)
        self.assertIn('source text takes precedence', prompt)
        self.assertIn('wording reference only', prompt)
        self.assertEqual(prompt.split('[Source Text]\n')[-1], 'n-bunaさんです。')

    def test_index_uses_its_template_and_cannot_publish_thinking_as_subtitles(self):
        adapter = HyMT2('http://127.0.0.1:9999/v1', 'Index-Translate-2B', 'Simplified Chinese',
                       backend='index-translate')
        requests = []

        class Client:
            content = '<think>internal</think>译文'
            def open(self, request, timeout):
                requests.append(json.loads(request.data))
                return io.StringIO(json.dumps({'choices': [{
                    'finish_reason': 'stop', 'message': {'content': self.content}}]}))

        adapter.client = Client()
        self.assertEqual(adapter.translate('原文。'), '译文')
        self.assertFalse(requests[0]['chat_template_kwargs']['enable_thinking'])
        self.assertEqual([m['role'] for m in requests[0]['messages']], ['user'])
        self.assertTrue(requests[0]['messages'][0]['content'].endswith('\n\n原文。'))
        adapter.client.content = '<think>unfinished'
        with self.assertRaisesRegex(ValueError, 'unfinished thinking'):
            adapter.translate('原文。')

    def test_only_uncommitted_source_is_in_the_translation_field(self):
        adapter = HyMT2('http://127.0.0.1:9999/v1', 'local-model', 'Chinese')
        requests = []

        class Client:
            def open(self, request, timeout):
                requests.append(json.loads(request.data))
                return io.StringIO(json.dumps({'choices': [{
                    'finish_reason': 'stop', 'message': {'content': '新译文'}}]}))

        adapter.client = Client()
        self.assertEqual(adapter.translate('今回の原文。', (('以前の原文。', '锁定译文。'),),
                                           '後続の参考文。'), '新译文')
        prompt = requests[0]['messages'][0]['content']
        self.assertEqual(prompt.split('[Source Text]\n')[-1], '今回の原文。')
        self.assertIn('Source: 以前の原文。\nTranslation (wording reference only): 锁定译文。', prompt)
        self.assertIn('reference only; do not translate', prompt)
        self.assertIn('後続の参考文。', prompt.split('[Source Text]\n')[0])
        self.assertFalse(requests[0]['stream'])
