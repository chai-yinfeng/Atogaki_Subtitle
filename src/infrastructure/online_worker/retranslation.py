"""Revision-aware translation groups; independent of ASR/runtime and display UI."""
from dataclasses import dataclass, field
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
    committed: bool = False
    last_request: float = float('-inf')
    displayed_request: int = 0
    lookahead: str = ''


class Retranslation:
    """One in-flight MT request. Source updates coalesce into current group state.

    Audio boundaries seal membership, not text. Only all-final source revisions
    can produce a committed translation. Japanese keeps short pauses inside a
    revisable semantic block and requires finalized right context for a sentence.
    """
    def __init__(self, emit, *, language='ja', drafts=True, interval=.8,
                 pause=None, max_seconds=20, max_chars=400, commit_wait=None):
        self.emit = emit
        self.language, self.drafts, self.interval = language, drafts, interval
        self.pause = pause if pause is not None else (3.0 if language == 'ja' else 1.2)
        self.commit_wait = commit_wait if commit_wait is not None else (8.0 if language == 'ja' else 4.0)
        self.audio_seconds = 0.0
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
        return self._join(s['text'] for s in group.segments.values() if s['text'])

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
                group.segments[sid] = dict(text='', revision=0, final=False, closed=False,
                                           start=start, end=end, finalized_at=None)
                self.segment_groups[sid] = group
            group = self.segment_groups[sid]
            group.end = max(group.end, end)
            group.segments[sid]['closed'] |= closed
            group.segments[sid]['end'] = max(group.segments[sid]['end'], end)
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
            if segment['final']:
                segment['finalized_at'] = self.audio_seconds
            group.revision += 1
            # A character bound closes at an acoustic boundary, never midway
            # through a segment whose final ASR still covers the entire range.
            if (group.reason is None and all(s['closed'] for s in group.segments.values())
                    and len(self._text(group)) >= self.max_chars):
                self._seal(group, 'max_chars')

    def advance(self, seconds, *, speech=False, eof=False):
        with self.lock:
            self.audio_seconds = max(self.audio_seconds, seconds)
            group = self.active
            if group is None:
                return
            if eof:
                self._seal(group, 'input_eof')
            elif not speech and all(s['closed'] for s in group.segments.values()):
                quiet = seconds-group.end
                if quiet >= self.pause:
                    self._seal(group, 'long_pause')

    def _commit_prefix(self, group):
        """Commit whole finalized ASR units; never splice translated strings.

        A sentence candidate needs following text as reference. A finite wait
        also releases unfinished phrases, explicitly labelled as a timeout.
        """
        items = list(group.segments.items())
        boundary, reason = None, None
        prefix = []
        for index, (_, segment) in enumerate(items):
            if not segment['final']:
                break
            prefix.append(segment['text'])
            following_items = items[index+1:]
            if self.language == 'ja':
                # Draft right context can still reverse a negation/referent.
                # Never skip an unfinalized segment to peek at a later final.
                following_items = []
                for item in items[index+1:]:
                    if not item[1]['final']:
                        break
                    following_items.append(item)
            following = self._join(s['text'] for _, s in following_items)
            # Qwen can punctuate fragments such as 一人で。; a bare period
            # alone is insufficient. These bounds are heuristics, not syntax.
            sentence = re.search(r'[。！？.!?][」』"\')）]*$', segment['text'].strip())
            if self.language == 'ja':
                sentence = sentence and self._japanese_sentence(segment['text'], following)
            if sentence and len(''.join(prefix).strip()) >= 8 and len(following.strip()) >= 8:
                boundary, reason = index+1, ('japanese_sentence_with_final_lookahead'
                                             if self.language == 'ja' else 'sentence_with_lookahead')
                break
        if boundary is None and group.reason is None:
            ready = [s for _, s in items if s['final']]
            if ready and items[0][1]['final'] and self.audio_seconds-ready[0]['finalized_at'] >= self.commit_wait:
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
                          group.end, segments=tail, revision=1, reason=group.reason)
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
        if remainder.reason is None:
            self.active = remainder

    def _join(self, texts):
        return (' ' if self.language in ('en', 'ko') else '').join(texts)

    def _following_text(self, group):
        texts = []
        for segment in group.segments.values():
            if self.language == 'ja' and not segment['final']:
                break
            texts.append(segment['text'])
        return self._join(texts)

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

    def take(self, now):
        with self.lock:
            if self.inflight is not None:
                raise RuntimeError('only one translation request may run')
            # Keep context deterministic: commit groups in audio order. There
            # is no unbounded queue of obsolete draft translation snapshots.
            group = next((g for g in self.groups if not g.committed), None)
            if group is None:
                return None
            self._commit_prefix(group)
            # A hard/pause-closed block may already have later Qwen text. This
            # reference crosses group boundaries without translating it twice.
            if self.language == 'ja' and group.reason is not None:
                later = self.groups[self.groups.index(group)+1:]
                texts = []
                for next_group in later:
                    texts.append(self._following_text(next_group))
                    if any(not s['final'] for s in next_group.segments.values()):
                        break
                group.lookahead = self._join(texts)[:500]
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
                           source_text=text, context=self.context, lookahead_source=group.lookahead,
                           context_revision=self.context_revision, final=final,
                           start_s=group.start, end_s=group.end,
                           first_source_end_s=next(iter(group.segments.values()))['end'],
                           completion_reason=group.reason)
            group.last_request = now
            group.requested_signature = signature
            self.inflight = request
            return request.copy()

    def cached(self, request):
        key = (request['source_text'], request['context'], request['lookahead_source'])
        return self.cache[1] if self.cache and self.cache[0] == key else None

    def complete(self, request, text, compute_s=0):
        with self.lock:
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
            event = self.emit('translation_final' if request['final'] else 'translation_draft',
                              **data, text=text, compute_s=compute_s,
                              stale_source=not current, provisional=not request['final'])
            if request['final']:
                group.committed = True
                # Keep bounded rolling reference even when sentence commits
                # are short. Committed outputs themselves are never retranslated.
                previous_source, previous_translation = self.context[0] if self.context else ('', '')
                source_context = self._join(s for s in (previous_source, request['source_text']) if s)
                translation_context = ' '.join(s for s in (previous_translation, text) if s)
                self.context = ((source_context[-1000:], translation_context[-1000:]),)
                self.context_revision += 1
                # Completed groups no longer retain all segment texts/revisions.
                self.groups.remove(group)
                for sid in group.segments:
                    del self.segment_groups[sid]
            return event

    def pending(self):
        with self.lock:
            return len(self.groups)
