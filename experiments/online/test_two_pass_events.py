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

    def test_group_final_requires_every_source_to_be_final(self):
        with redirect_stdout(io.StringIO()):
            events = Events(io.StringIO())
            a = events.emit('source_final', segment_id='a', text='a')
            b = events.emit('source_draft', segment_id='b', text='b')
            revisions = {'a': a['source_revision'], 'b': b['source_revision']}
            with self.assertRaisesRegex(ValueError, 'finalized source revision'):
                events.emit('translation_final', group_id='group-1',
                            source_revisions=revisions, text='premature')
            b = events.emit('source_final', segment_id='b', text='final b')
            revisions['b'] = b['source_revision']
            final = events.emit('translation_final', group_id='group-1',
                                source_revisions=revisions, text='valid', end_s=0)
            self.assertEqual(final['source_revisions'], revisions)


if __name__ == '__main__':
    unittest.main()
