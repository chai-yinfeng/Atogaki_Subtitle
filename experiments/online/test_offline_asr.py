import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'src/infrastructure'))
from online_worker.offline_asr import timed_units

class OfflineAlignmentTests(unittest.TestCase):
    def result(self, text, spans, truncated=False):
        return SimpleNamespace(text=text, segments=[dict(text=t,start=a,end=b) for t,a,b in spans],truncated=truncated)
    def test_punctuation_is_preserved_and_zero_bins_merge_without_inventing_time(self):
        units=timed_units(self.result('「猫に、会う。」',[('猫',0,.5),('に',.5,.5),('会う',.5,1)]))
        self.assertEqual(''.join(u['text'] for u in units),'「猫に、会う。」')
        self.assertEqual([(u['start_ms'],u['end_ms']) for u in units],[(0,500),(500,1000)])
        self.assertTrue(all(u['timing_source']=='forced_alignment' for u in units))
    def test_leading_zero_and_overlapping_words_keep_all_source(self):
        units=timed_units(self.result('Hello, world!',[('Hello',0,0),('world',0,1)]))
        self.assertEqual(units[0]['text'],'Hello, world!')
        units=timed_units(self.result('猫を見る。',[('猫',0,1),('を',.8,1.2),('見る',1.2,2)]))
        self.assertEqual(units[0]['end_ms'],1200)
        self.assertEqual(''.join(u['text'] for u in units),'猫を見る。')
    def test_truncated_missing_mismatched_and_invalid_evidence_is_rejected(self):
        for result in [self.result('猫', [('猫',0,1)],True),self.result('猫',[]),self.result('猫',[('犬',0,1)]),self.result('猫',[('猫',1,.5)]),self.result('猫',[('猫',0,0)])]:
            with self.assertRaises(ValueError): timed_units(result)

if __name__=='__main__':unittest.main()
