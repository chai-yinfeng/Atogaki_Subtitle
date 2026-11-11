from pathlib import Path
import tempfile
import unittest

import _worker_path
from online_worker.drain_budget import DrainBudget


class DrainBudgetTests(unittest.TestCase):
    def test_owner_deadline_and_eof_share_remaining_time_without_reset(self):
        with tempfile.TemporaryDirectory() as root:
            control = Path(root)/'stop'
            now = [10, 1000]
            budget = DrainBudget(control, monotonic=lambda: now[0], wall=lambda: now[1])
            self.assertFalse(budget.draining)
            control.write_text('1030')
            self.assertEqual(budget.remaining(), 30)
            now[:] = [20, 1010]
            budget.begin()
            self.assertEqual(budget.remaining(), 20)
            # Wall-clock jumps after receipt cannot extend a monotonic deadline.
            now[:] = [39, 1]
            self.assertEqual(budget.remaining(), 1)
            now[0] = 40
            with self.assertRaises(TimeoutError):
                budget.remaining()

    def test_replay_eof_has_finite_budget(self):
        now = [0]
        budget = DrainBudget(monotonic=lambda: now[0])
        budget.begin()
        now[0] = 29
        self.assertEqual(budget.remaining(), 1)
        budget.begin()
        self.assertEqual(budget.remaining(), 1)

    def test_invalid_owner_deadline_is_not_silently_unbounded(self):
        with tempfile.TemporaryDirectory() as root:
            control = Path(root)/'stop'
            for value in ['nan', 'inf', 'broken']:
                control.write_text(value)
                with self.assertRaises(ValueError):
                    DrainBudget(control).remaining()
