"""Revision-aware translation groups; independent of ASR/runtime and display UI."""
from dataclasses import dataclass, field
import threading


@dataclass
class Group:
    id: str
    start: float
    end: float
    segments: dict = field(default_factory=dict)
    revision: int = 0
    reason: str | None = None
    committed: bool = False
    last_request: float = float('-inf')
    displayed_request: int = 0


class Retranslation:
    """One in-flight MT request. Source updates coalesce into current group state.

    Audio boundaries seal membership, not text. Only all-final source revisions
    can produce a committed translation. A VAD endpoint alone never commits MT.
    """
    def __init__(self, emit, *, language='ja', drafts=True, interval=.8,
                 pause=1.2, max_seconds=20, max_chars=400):
        self.emit = emit
        self.language, self.drafts, self.interval = language, drafts, interval
        self.pause = pause
        self.max_seconds, self.max_chars = max_seconds, max_chars
        self.lock = threading.Lock()
        self.groups, self.segment_groups = [], {}
        self.active = None
        self.context = ()
        self.context_revision = 0
        self.request_id = 0
        self.group_id = 0
        self.inflight = None
        self.cache = None

    def _text(self, group):
        separator = ' ' if self.language in ('en', 'ko') else ''
        return separator.join(s['text'] for s in group.segments.values() if s['text'])

    def _seal(self, group, reason):
        if group.reason is None:
            group.reason = reason
            if self.active is group:
                self.active = None
            self.emit('translation_group_closed', group_id=group.id,
                      start_s=group.start, end_s=group.end, completion_reason=reason)

    def observe(self, sid, start, end, *, closed=False):
        """Capture registers ranges before dispatching recognition jobs."""
        with self.lock:
            if sid not in self.segment_groups:
                if self.active and start-self.active.end >= self.pause:
                    self._seal(self.active, 'long_pause')
                if self.active is None:
                    self.group_id += 1
                    self.active = Group(f'group-{self.group_id}', start, end)
                    self.groups.append(self.active)
                group = self.active
                group.segments[sid] = dict(text='', revision=0, final=False, closed=False)
                self.segment_groups[sid] = group
            group = self.segment_groups[sid]
            group.end = max(group.end, end)
            group.segments[sid]['closed'] |= closed
            if closed and group.reason is None:
                if group.end-group.start >= self.max_seconds:
                    self._seal(group, 'max_duration')
                elif len(self._text(group)) >= self.max_chars:
                    self._seal(group, 'max_chars')

    def update(self, event):
        if event is None:
            return
        with self.lock:
            group = self.segment_groups.get(event['segment_id'])
            if group is None:
                return  # A draft emitted before final ASR may arrive after MT commit.
            segment = group.segments[event['segment_id']]
            if group.committed or segment['final']:
                return
            segment.update(text=event['text'], revision=event['source_revision'],
                           final=event['kind'] == 'source_final')
            group.revision += 1
            # A character bound closes at an acoustic boundary, never midway
            # through a segment whose final ASR still covers the entire range.
            if (group.reason is None and all(s['closed'] for s in group.segments.values())
                    and len(self._text(group)) >= self.max_chars):
                self._seal(group, 'max_chars')

    def advance(self, seconds, *, speech=False, eof=False):
        with self.lock:
            group = self.active
            if group is None:
                return
            if eof:
                self._seal(group, 'input_eof')
            elif not speech and all(s['closed'] for s in group.segments.values()):
                quiet = seconds-group.end
                if quiet >= self.pause:
                    self._seal(group, 'long_pause')

    def take(self, now):
        with self.lock:
            if self.inflight is not None:
                raise RuntimeError('only one translation request may run')
            # Keep context deterministic: commit groups in audio order. There
            # is no unbounded queue of obsolete draft translation snapshots.
            group = next((g for g in self.groups if not g.committed), None)
            if group is None:
                return None
            final = group.reason is not None and all(s['final'] for s in group.segments.values())
            text = self._text(group)
            signature = (group.revision, self.context_revision)
            if not text or (not final and (not self.drafts or group.reason is not None
                    or now-group.last_request < self.interval
                    or getattr(group, 'requested_signature', None) == signature)):
                return None
            self.request_id += 1
            request = dict(request_id=self.request_id, group_id=group.id,
                           group_revision=group.revision,
                           source_revisions={sid: s['revision'] for sid, s in group.segments.items()},
                           source_text=text, context=self.context,
                           context_revision=self.context_revision, final=final,
                           start_s=group.start, end_s=group.end,
                           completion_reason=group.reason)
            group.last_request = now
            group.requested_signature = signature
            self.inflight = request
            return request.copy()

    def cached(self, request):
        key = (request['source_text'], request['context'])
        return self.cache[1] if self.cache and self.cache[0] == key else None

    def complete(self, request, text, compute_s=0):
        with self.lock:
            if self.inflight != request:
                raise ValueError('translation request is not in flight')
            self.inflight = None
            self.cache = ((request['source_text'], request['context']), text)
            group = next(g for g in self.groups if g.id == request['group_id'])
            current = request['group_revision'] == group.revision
            if group.committed or request['request_id'] <= group.displayed_request:
                return None
            if request['final'] and not current:
                return None
            group.displayed_request = request['request_id']
            data = {key: value for key, value in request.items() if key not in ('context', 'final')}
            event = self.emit('translation_final' if request['final'] else 'translation_draft',
                              **data, text=text, compute_s=compute_s,
                              stale_source=not current, provisional=not request['final'])
            if request['final']:
                group.committed = True
                # One bounded preceding group; immutable while translating
                # the next one. No cascading retranslation of old captions.
                self.context = ((request['source_text'][-1000:], text[-1000:]),)
                self.context_revision += 1
                # Completed groups no longer retain all segment texts/revisions.
                self.groups.remove(group)
                for sid in group.segments:
                    del self.segment_groups[sid]
            return event

    def pending(self):
        with self.lock:
            return len(self.groups)
