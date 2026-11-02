"""Temporal contracts for provisional source display and finalized translations."""
from contextlib import redirect_stdout
import io
import unittest

from run_two_pass import Events


class RevisionContractTests(unittest.TestCase):
    def test_late_draft_cannot_replace_final_source(self):
        stream = io.StringIO()
        with redirect_stdout(io.StringIO()):
            events = Events(stream)
            events.emit('source_draft', segment_id='s', text='old')
            final = events.emit('source_final', segment_id='s', text='corrected')
            events.emit('translation_final', segment_id='s',
                        source_revision=final['source_revision'], text='translation')
            self.assertIsNone(events.emit('source_draft', segment_id='s', text='late'))
        self.assertNotIn('late', stream.getvalue())
        self.assertEqual(events.counts, {'source_draft': 1, 'source_final': 1,
                                       'translation_final': 1})

    def test_translation_cannot_bind_to_provisional_revision(self):
        with redirect_stdout(io.StringIO()):
            events = Events(io.StringIO())
            draft = events.emit('source_draft', segment_id='s', text='draft')
            events.emit('source_final', segment_id='s', text='final')
            with self.assertRaisesRegex(ValueError, 'finalized source revision'):
                events.emit('translation_final', segment_id='s',
                            source_revision=draft['source_revision'], text='stale')


if __name__ == '__main__':
    unittest.main()
