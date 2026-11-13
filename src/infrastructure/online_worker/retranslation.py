"""Revision-aware translation groups; independent of ASR/runtime and display UI."""
from dataclasses import dataclass, field
from contextlib import contextmanager
import threading
import re


@dataclass
class Group:
    id: str
    start: float
    end: float
    segments: dict = field(default_factory=dict)
    revision: int = 0
    reason: str | None = None
    range_reason: str | None = None
    committed: bool = False
    last_request: float = float('-inf')
    displayed_request: int = 0
    lookahead: str = ''
    failed_revision: int = -1
    final_failures: int = 0


class Retranslation:
    """One in-flight MT request. Source updates coalesce into current group state.

    Source/audio events determine membership; taking work never partitions it.
    Only all-final current source can commit. Following local drafts are optional
    reference, never part of the current request's translated coverage.
    """
    def __init__(self, emit, *, language='ja', drafts=True, interval=.8,
                 pause=None, max_seconds=20, max_chars=400, commit_wait=None):
        self.emit = emit
        self.language, self.drafts, self.interval = language, drafts, interval
        self.pause = pause if pause is not None else (3.0 if language == 'ja' else 1.2)
        self.commit_wait = commit_wait if commit_wait is not None else (8.0 if language == 'ja' else 4.0)
        self.audio_seconds = 0.0
        self.now = 0.0
        self.max_seconds, self.max_chars = max_seconds, max_chars
        self.lock = threading.Lock()
        self.groups, self.segment_groups = [], {}
        self.active = None
        self.retired_sources = []
        self.context_revision = 0
        self.request_id = 0
        self.group_id = 0
        self.inflight = None
        self.cache = None
        self.outbox = []

    @contextmanager
    def _transition(self):
        with self.lock:
            try:
                yield
            finally:
                pending, self.outbox = self.outbox, []
        # No model, HTTP or event-pipe I/O while owning the state lock.
        for kind, data in pending:
            self.emit(kind, **data)

    def _close_range(self, group, reason):
        if group.range_reason is None:
            group.range_reason = reason
            if self.active is group:
                self.active = None

    def _text(self, group):
        return self._join(s['text'] for s in group.segments.values() if s['text'])

    def _seal(self, group, reason):
        if group.reason is None:
            group.reason = reason
            if self.active is group:
                self.active = None
            group.revision += 1
            self.outbox.append(('translation_group_closed', dict(group_id=group.id,
                start_s=group.start, end_s=group.end, completion_reason=reason)))

    def _settle(self):
        while True:
            before = [(g.id, g.reason, tuple(g.segments)) for g in self.groups]
            for group in list(self.groups):
                if group.reason is not None:
                    continue
                self._commit_prefix(group)
                if (group.reason is None and group.range_reason is not None
                        and all(s['final'] for s in group.segments.values())):
                    self._seal(group, group.range_reason)
            if before == [(g.id, g.reason, tuple(g.segments)) for g in self.groups]:
                break

    def _before(self, now):
        if now is not None:
            # Expired cutoffs precede later input. Equal-time input wins ties.
            previous = self.now
            self.now = max(self.now, now - 1e-9)
            self._settle()
            self.now = max(previous, now)

    def observe(self, sid, start, end, *, closed=False, now=None):
        """Capture registers ranges before dispatching recognition jobs."""
        with self._transition():
            self._before(now)
            if sid not in self.segment_groups:
                if self.active and start-self.active.end >= self.pause:
                    self._close_range(self.active, 'long_pause')
                if self.active is None:
                    self.group_id += 1
                    self.active = Group(f'group-{self.group_id}', start, end)
                    self.groups.append(self.active)
                group = self.active
                group.segments[sid] = dict(text='', revision=0, final=False, closed=False,
                                           start=start, end=end, finalized_at=None)
                self.segment_groups[sid] = group
            group = self.segment_groups[sid]
            if group.reason is not None:
                return
            group.end = max(group.end, end)
            group.segments[sid]['closed'] |= closed
            group.segments[sid]['end'] = max(group.segments[sid]['end'], end)
            if closed and group.reason is None:
                if group.end-group.start >= self.max_seconds:
                    self._close_range(group, 'max_duration')
                elif len(self._text(group)) >= self.max_chars:
                    self._close_range(group, 'max_chars')
            self._settle()

    def update(self, event):
        if event is None:
            return
        with self._transition():
            self._before(event.get('emission_s'))
            group = self.segment_groups.get(event['segment_id'])
            if group is None:
                return  # A draft emitted before final ASR may arrive after MT commit.
            segment = group.segments[event['segment_id']]
            if group.committed or segment['final'] or event['source_revision'] <= segment['revision']:
                return
            segment.update(text=event['text'], revision=event['source_revision'],
                           final=event['kind'] == 'source_final')
            if segment['final']:
                segment['finalized_at'] = self.now
                self.context_revision += 1
            group.revision += 1
            # A character bound closes at an acoustic boundary, never midway
            # through a segment whose final ASR still covers the entire range.
            if (group.reason is None and all(s['closed'] for s in group.segments.values())
                    and len(self._text(group)) >= self.max_chars):
                self._close_range(group, 'max_chars')
            self._settle()

    def advance(self, seconds, *, speech=False, eof=False, now=None):
        with self._transition():
            self._before(seconds if now is None else now)
            self.audio_seconds = max(self.audio_seconds, seconds)
            group = self.active
            if group is not None:
                if eof:
                    self._close_range(group, 'input_eof')
                elif not speech and all(s['closed'] for s in group.segments.values()):
                    if seconds-group.end >= self.pause:
                        self._close_range(group, 'long_pause')
            self._settle()

    def _commit_prefix(self, group):
        """Commit whole finalized ASR units; never splice translated strings.

        A sentence candidate needs following text as reference. A finite wait
        also releases unfinished phrases, explicitly labelled as a timeout.
        """
        if group.reason is not None:
            return
        items = list(group.segments.items())
        boundary, reason = None, None
        prefix = []
        for index, (_, segment) in enumerate(items):
            if not segment['final']:
                break
            prefix.append(segment['text'])
            following_items = []
            for item in items[index+1:]:
                if not item[1]['text']:
                    break
                following_items.append(item)
            following = self._join(s['text'] for _, s in following_items)
            # Qwen can punctuate fragments such as 一人で。; a bare period
            # alone is insufficient. These bounds are heuristics, not syntax.
            sentence = re.search(r'[。！？.!?][」』"\')）]*$', segment['text'].strip())
            if self.language == 'ja':
                sentence = sentence and self._japanese_sentence(segment['text'], following)
            if sentence and len(''.join(prefix).strip()) >= 8 and len(following.strip()) >= 8:
                boundary, reason = index+1, ('japanese_sentence_with_lookahead'
                                             if self.language == 'ja' else 'sentence_with_lookahead')
                break
        if boundary is None and group.range_reason is None:
            ready = [s for _, s in items if s['final']]
            if ready and items[0][1]['final'] and self.now-ready[0]['finalized_at'] >= self.commit_wait:
                boundary = 0
                for _, segment in items:
                    if not segment['final']:
                        break
                    boundary += 1
                reason = 'japanese_timeout' if self.language == 'ja' else 'commit_timeout'
        if boundary is None:
            return
        if boundary == len(items):
            self._seal(group, reason)
            return
        tail = dict(items[boundary:])
        self.group_id += 1
        remainder = Group(f'group-{self.group_id}', next(iter(tail.values()))['start'],
                          group.end, segments=tail, revision=1, range_reason=group.range_reason)
        group.segments = dict(items[:boundary])
        group.end = items[boundary-1][1]['end']
        group.lookahead = self._following_text(remainder)[:500]
        group.revision += 1
        # Old group_id now owns just the committed prefix. Its final replaces
        # the former whole-group draft, and the tail gets a fresh identity.
        group.reason = None
        self._seal(group, reason)
        self.groups.insert(self.groups.index(group)+1, remainder)
        for sid in tail:
            self.segment_groups[sid] = remainder
        if remainder.range_reason is None:
            self.active = remainder

    def _join(self, texts):
        return (' ' if self.language in ('en', 'ko') else '').join(texts)

    def _following_text(self, group):
        texts = []
        for segment in group.segments.values():
            if not segment['text']:
                break
            texts.append(segment['text'])
        return self._join(texts)

    def _source_history(self, before=float('inf')):
        entries = list(self.retired_sources)
        for group in self.groups:
            prefix = []
            for segment in group.segments.values():
                if not segment['final']:
                    break
                prefix.append(segment)
            if prefix:
                entries.append(dict(start=group.start, end=prefix[-1]['end'],
                                    text=self._join(s['text'] for s in prefix)))
        entries = sorted((e for e in entries if e['end'] <= before), key=lambda e: e['start'])
        while entries and (len(entries) > 8 or sum(len(e['text']) for e in entries) > 2000):
            entries.pop(0)
        # Keep the existing adapter interface; an empty target denotes source
        # context only, never an empty translation that needs filling in.
        return tuple((e['text'], '') for e in entries)

    @property
    def context(self):
        with self.lock:
            return self._source_history()

    def _retire(self, group):
        self.retired_sources.append(dict(start=group.start, end=group.end, text=self._text(group)))
        while self.retired_sources and (len(self.retired_sources) > 8 or
                sum(len(e['text']) for e in self.retired_sources) > 2000):
            self.retired_sources.pop(0)
        self.groups.remove(group)
        for sid in group.segments:
            del self.segment_groups[sid]

    @staticmethod
    def _japanese_sentence(text, following):
        """Conservative surface hints, not a semantic-completeness classifier.

        ASR often punctuates particles/hesitations as sentences. Leave those
        inside the same MT Source Text, including common right-hand reversals.
        Bounds/EOF still release speech that these deliberately limited hints
        cannot classify. No additional model or per-frame parser is required.
        """
        body = re.sub(r'[。！？.!?」』"\')）\s]+$', '', text)
        predicate = re.search(r'(?:です|でした|でしょう|ます|ました|ません|だ|だった|だろう|'
                              r'ない|なかった|たい|たかった|[うくぐすつぬぶむるた])'
                              r'(?:よね|かな|よ|ね|な|か)?$', body)
        continuation = re.match(r'^[\s。、「『]*(?:わけ(?:では|じゃ)|という|のでは|んじゃ|とは|'
                                r'んです|のです|だけ|ほど|くらい|ぐらい|ため|ので|のに|'
                                r'けど|けれど|ですが|だが|ものの|と思|って)', following)
        return bool(predicate and following and not continuation)

    def _candidate(self, now):
        group = next((g for g in self.groups if not g.committed), None)
        if group is None:
            return None
        texts, right = [], []
        right_chars = 0
        for following in self.groups[self.groups.index(group)+1:]:
            for sid, segment in following.segments.items():
                if not segment['text']:
                    break
                separator = 1 if texts and self.language in ('en', 'ko') else 0
                if right_chars + separator >= 500:
                    break
                part = segment['text'][:500-right_chars-separator]
                texts.append(part)
                right_chars += len(part)+separator
                right.append((sid, segment))
            if right_chars >= 500 or any(not s['text'] for s in following.segments.values()):
                break
        final = group.reason is not None and all(s['final'] for s in group.segments.values())
        text = self._text(group)
        context = self._source_history(before=group.start)
        signature = (group.revision, context)
        if not text or (not final and (not self.drafts
                or now-group.last_request < self.interval
                or getattr(group, 'requested_signature', None) == signature)):
            return None
        confirmed = tuple((sid, s['revision']) for sid, s in group.segments.items() if s['final'])
        priority = ('final' if final else 'confirmed' if confirmed and
                    confirmed != getattr(group, 'requested_confirmed', ()) else 'draft')
        request = dict(request_id=self.request_id+1, group_id=group.id,
                       group_revision=group.revision,
                       source_revisions={sid: s['revision'] for sid, s in group.segments.items()},
                       source_text=text, context=context, lookahead_source=self._join(texts)[:500],
                       lookahead_revisions={sid: s['revision'] for sid, s in right},
                       lookahead_provisional=any(not s['final'] for _, s in right),
                       context_revision=self.context_revision, final=final,
                       start_s=group.start, end_s=group.end,
                       first_source_end_s=next(iter(group.segments.values()))['end'],
                       completion_reason=group.reason)
        return group, request, signature, confirmed, priority

    def priority(self, now):
        with self.lock:
            if self.inflight is not None:
                return None
            candidate = self._candidate(now)
            return candidate[-1] if candidate else None

    def stop_drafts(self):
        with self.lock:
            self.drafts = False

    def take(self, now):
        with self.lock:
            if self.inflight is not None:
                raise RuntimeError('only one translation request may run')
            candidate = self._candidate(now)
            if candidate is None:
                return None
            group, request, signature, confirmed, _ = candidate
            self.request_id = request['request_id']
            group.last_request = now
            group.requested_signature = signature
            group.requested_confirmed = confirmed
            self.inflight = request
            return request.copy()

    def cached(self, request):
        key = (request['source_text'], request['context'], request['lookahead_source'])
        return self.cache[1] if self.cache and self.cache[0] == key else None

    def complete(self, request, text, compute_s=0):
        with self._transition():
            if self.inflight != request:
                raise ValueError('translation request is not in flight')
            self.inflight = None
            self.cache = ((request['source_text'], request['context'], request['lookahead_source']), text)
            group = next(g for g in self.groups if g.id == request['group_id'])
            current = request['group_revision'] == group.revision
            if group.committed or request['request_id'] <= group.displayed_request:
                return None
            if request['final'] and not current:
                return None
            group.displayed_request = request['request_id']
            data = {key: value for key, value in request.items() if key not in ('context', 'final')}
            kind = 'translation_final' if request['final'] else 'translation_draft'
            data.update(text=text, compute_s=compute_s, stale_source=not current,
                        provisional=not request['final'])
            self.outbox.append((kind, data))
            if request['final']:
                group.committed = True
                self._retire(group)
            return dict(kind=kind, **data)

    def pending(self):
        with self.lock:
            return len(self.groups)

    def fail(self, request):
        """Drafts wait for a new revision; current finals get one bounded retry.

        A terminal failure retires only this group, without publishing a final,
        caching rejected text, or adding an unreliable pair to context.
        """
        with self._transition():
            if self.inflight != request:
                raise ValueError('translation request is not in flight')
            self.inflight = None
            group = next(g for g in self.groups if g.id == request['group_id'])
            if not request['final'] or request['group_revision'] != group.revision:
                return False
            if group.failed_revision != group.revision:
                group.failed_revision, group.final_failures = group.revision, 0
            group.final_failures += 1
            if group.final_failures < 2:
                return False
            data = {key: value for key, value in request.items() if key not in ('context', 'final')}
            self.outbox.append(('translation_failed', dict(**data, provisional=False)))
            self._retire(group)
            return True
