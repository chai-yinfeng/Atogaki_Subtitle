"""Committed text and lookahead stay reference-only in the MT request."""
import io
import json
import unittest
from harness import HyMT2


class PromptTests(unittest.TestCase):
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
        self.assertIn('Source: 以前の原文。\nTranslation: 锁定译文。', prompt)
        self.assertIn('reference only; do not translate', prompt)
        self.assertIn('後続の参考文。', prompt.split('[Source Text]\n')[0])
        self.assertFalse(requests[0]['stream'])
