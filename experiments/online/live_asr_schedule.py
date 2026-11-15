"""Bounded rolling ASR demand for controlled, independent-crop live replays."""
from collections import deque
import queue
import threading
import time


class ASRMailbox:
    def __init__(self):
        self.condition = threading.Condition()
        self.latest = None
        self.finals = deque()
        self.inflight = None
        self.completed = {}
        self.closed = set()
        self.finished = False
        self.aborted = False
        self.replaced = 0
        self.final_upgrades = 0
        self.final_reuses = 0

    def preview(self, job):
        with self.condition:
            if job['sid'] in self.closed or self.finished or self.aborted:
                return
            self.replaced += int(self.latest is not None)
            self.latest = dict(job, final=False)
            self.condition.notify_all()

    def close(self, job):
        with self.condition:
            sid = job['sid']
            if sid in self.closed:
                raise ValueError('crop already closed')
            self.closed.add(sid)
            if self.latest and self.latest['sid'] == sid:
                self.latest = None
            previous = self.completed.get(sid)
            if previous is not None and previous[0] == job['end']:
                self.final_reuses += 1
                return previous[1]
            if self.inflight and (self.inflight['sid'], self.inflight['end']) == (sid, job['end']):
                self.inflight['final'] = True
                self.final_upgrades += 1
            else:
                if self.latest and self.latest['sid'] == sid:
                    self.latest = None
                self.finals.append(dict(job, final=True))
            self.condition.notify_all()
            return None

    def discard(self, sid):
        with self.condition:
            self.closed.add(sid)
            if self.latest and self.latest['sid'] == sid:
                self.latest = None

    def wait(self):
        with self.condition:
            self.condition.wait_for(lambda: self.aborted or self.latest is not None or self.finals or self.finished)
            return not self.aborted and bool(self.latest is not None or self.finals)

    def take(self):
        with self.condition:
            if self.aborted:
                return None
            if self.inflight is not None:
                raise RuntimeError('ASR mailbox already has an owner')
            self.inflight = self.finals.popleft() if self.finals else self.latest
            if self.inflight is self.latest:
                self.latest = None
            return self.inflight

    def complete(self, job, result):
        with self.condition:
            if self.inflight is not job:
                raise RuntimeError('ASR completion does not own its job')
            self.completed[job['sid']] = (job['end'], result)
            self.inflight = None
            return job['final']

    def finish(self, *, abort=False):
        with self.condition:
            self.finished = True
            self.aborted = abort
            self.condition.notify_all()


class FinalMTQueue:
    """Minimal finalized-crop work list consumed by the existing StageAdmission."""
    def __init__(self):
        self.jobs = queue.Queue()

    def put(self, job):
        self.jobs.put(dict(job, queued_at=time.monotonic()))

    def priority(self, now):
        return 'final' if not self.jobs.empty() else None

    def take(self, now):
        try:
            return self.jobs.get_nowait()
        except queue.Empty:
            return None


def replay_metrics(events):
    crops = {e['segment_id']: dict(start_s=e['start_s'], duration_s=e['duration_s'])
             for e in events if e['kind'] == 'crop_start'}
    for sid, row in crops.items():
        source = [e for e in events if e.get('segment_id') == sid and e['kind'] in ('source_draft', 'source_final')]
        finals = [e for e in source if e['kind'] == 'source_final']
        translations = [e for e in events if e.get('segment_id') == sid and e['kind'] == 'translation_final']
        row['first_visible_source_s'] = source[0]['emission_s']-row['start_s'] if source else None
        calls = [e for e in events if e.get('segment_id') == sid and e['kind'] == 'asr_call']
        row['maximum_input_age_on_return_s'] = max((e['emission_s']-e['start_s']-e['input_samples']/16000 for e in calls), default=0)
        row['preview_updates'] = sum(e['kind'] == 'source_draft' for e in source)
        row['max_visible_refresh_gap_s'] = max((b['emission_s']-a['emission_s'] for a, b in zip(source, source[1:])), default=0)
        row['rewritten_suffix_characters'] = 0
        for a, b in zip(source, source[1:]):
            common = 0
            for left, right in zip(a['text'], b['text']):
                if left != right:
                    break
                common += 1
            row['rewritten_suffix_characters'] += len(a['text'])-common
        row['final_source_after_end_s'] = finals[0]['emission_s']-row['start_s']-row['duration_s'] if finals else None
        row['translation_after_end_s'] = translations[0]['emission_s']-row['start_s']-row['duration_s'] if translations else None
        row['final_count'] = len(finals)
        row['translation_count'] = len(translations)
        row['post_final_source_events'] = sum(e['emission_s'] > finals[0]['emission_s'] for e in source) if finals else 0
    calls = [e for e in events if e['kind'] == 'asr_call']
    return dict(crops=crops, asr_compute_s_by_model={model: sum(e['compute_s'] for e in calls if e['model'] == model)
                for model in {e['model'] for e in calls}},
                qwen_peak_active_bytes=max((e.get('peak_active_bytes', 0) for e in calls), default=0),
                maximum_asr_queue_wait_s=max((e['queue_wait_s'] for e in calls), default=0),
                maximum_mt_queue_wait_s=max((e['queue_wait_s'] for e in events if e['kind'] == 'translation_final'), default=0))
