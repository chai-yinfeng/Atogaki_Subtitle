"""One stop deadline, shared by queued stages; owner enforces blocking calls."""
import math
import threading
import time


class DrainBudget:
    def __init__(self, control=None, *, monotonic=time.monotonic, wall=time.time):
        self.control = control
        self.monotonic, self.wall = monotonic, wall
        self.deadline = None
        self.received = False
        self.lock = threading.Lock()

    def _poll(self):
        if self.control is not None and not self.received:
            try:
                value = float(self.control.read_text())
            except FileNotFoundError:
                return
            if not math.isfinite(value):
                raise ValueError('invalid online drain deadline')
            deadline = self.monotonic() + max(0, value-self.wall())
            self.deadline = deadline if self.deadline is None else min(self.deadline, deadline)
            self.received = True

    def begin(self, seconds=30):
        with self.lock:
            self._poll()
            deadline = self.monotonic()+seconds
            self.deadline = deadline if self.deadline is None else min(self.deadline, deadline)

    def remaining(self):
        with self.lock:
            self._poll()
            remaining = self.deadline-self.monotonic() if self.deadline is not None else float('inf')
        if remaining <= 0:
            raise TimeoutError('online session drain budget exhausted')
        return remaining

    @property
    def draining(self):
        with self.lock:
            self._poll()
            return self.deadline is not None
