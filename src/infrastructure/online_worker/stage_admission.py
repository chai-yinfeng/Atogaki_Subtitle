"""Bounded local stage admission; model calls remain on their owning threads."""
from contextlib import contextmanager
import threading
import time


class StageAdmission:
    def __init__(self, translation, *, overlap=False):
        self.translation = translation
        self.overlap = overlap
        self.condition = threading.Condition()
        self.asr_active = False
        self.mt_active = False
        self.asr_waiting = 0
        self.aborted = False

    def _priority(self):
        return self.translation.priority(time.monotonic()) if self.translation is not None else None

    def abort(self):
        with self.condition:
            self.aborted = True
            self.condition.notify_all()

    @contextmanager
    def asr(self):
        with self.condition:
            self.asr_waiting += 1
            self.condition.notify_all()
            try:
                while not self.aborted and (self.asr_active or (not self.overlap and (
                        self.mt_active or self._priority() in ('final', 'confirmed')))):
                    self.condition.wait(.02)
                if self.aborted:
                    raise RuntimeError('online stage admission aborted')
                self.asr_active = True
            finally:
                self.asr_waiting -= 1
        try:
            yield
        finally:
            with self.condition:
                self.asr_active = False
                self.condition.notify_all()

    def take_mt(self):
        with self.condition:
            if self.aborted or self.mt_active or (self.asr_active and not self.overlap):
                return None
            priority = self._priority()
            if priority is None or (not self.overlap and self.asr_waiting and priority == 'draft'):
                return None
            request = self.translation.take(time.monotonic())
            if request is not None:
                self.mt_active = True
            return request

    def complete_mt(self):
        with self.condition:
            self.mt_active = False
            self.condition.notify_all()
