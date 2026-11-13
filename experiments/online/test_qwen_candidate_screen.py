"""CPU-only checks for evidence integrity and blind-review construction."""
from pathlib import Path
import tempfile
import unittest

from screen_qwen_candidates import anonymize, choose_review, normalize, sha, verify


class CandidateScreenTests(unittest.TestCase):
    def test_changed_evidence_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "input.wav"
            path.write_bytes(b"frozen input")
            digest = sha(path)
            verify(path, digest)
            path.write_bytes(b"modified input")
            with self.assertRaises(ValueError):
                verify(path, digest)

    def test_anonymous_candidates_keep_all_results_and_separate_identity(self):
        candidates = {name: {"attempts": [{"status": "completed", "text": text}]}
                      for name, text in zip(["small", "large", "quantized", "reference"],
                                            ["一", "二", "三", "四"])}
        public, key = anonymize("segment-1", candidates)
        self.assertEqual((public, key), anonymize("segment-1", candidates))
        self.assertEqual([row["label"] for row in public], list("ABCD"))
        self.assertEqual(set(key.values()), set(candidates))
        for row in public:
            self.assertEqual(set(row), {"label", "attempts"})
            self.assertEqual(row["attempts"], candidates[key[row["label"]]]["attempts"])

    def test_review_keeps_agreement_controls_and_preapproved_overlaps(self):
        rows = [dict(id=f"{case}-{index}", case_id=case, disagreement=index/20 if index > 2 else 0)
                for case in ["radio-01", "radio-11"] for index in range(15)]
        selected = choose_review(rows, {"radio-11-0"})
        self.assertIn("radio-11-0", selected)
        for case in ["radio-01", "radio-11"]:
            self.assertGreaterEqual(sum(row["id"] in selected and row["case_id"] == case
                                        and row["disagreement"] == 0 for row in rows), 2)
            self.assertIn(f"{case}-14", selected)
        self.assertEqual(normalize(" Ａ、 B。"), "AB")
        self.assertNotEqual(normalize("行った"), normalize("行かなかった"))


if __name__ == "__main__":
    unittest.main()
