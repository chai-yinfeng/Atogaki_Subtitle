"""Research-only bounded recovery from ASR context copying; not wired to App.

No model/dependency imports. Empty-context retry does not establish semantic
correctness or speech presence. Preserve both attempts instead of salvaging a
prefix or deleting supplied vocabulary from an otherwise legitimate utterance.
"""
from dataclasses import asdict, is_dataclass
import time
import unicodedata


def normalized_context(text):
    return ''.join(char for char in unicodedata.normalize('NFKC', text).casefold()
                   if not char.isspace() and not unicodedata.category(char).startswith('P'))


def looks_like_context_copy(text, context):
    reference = normalized_context(context)
    return bool(reference and normalized_context(text) == reference)


def rejection_reason(result, context):
    if getattr(result, 'truncated', False):
        return 'token_limit'
    text = getattr(result, 'text', None)
    if not isinstance(text, str) or not text.strip():
        return 'empty_or_invalid_text'
    if context and looks_like_context_copy(text, context):
        return 'suspected_context_copy'
    return None


class ASRContextRecoveryError(ValueError):
    def __init__(self, reason, attempts):
        super().__init__(reason)
        self.attempts = attempts


def transcribe_with_recovery(transcribe, audio, *, language, context='',
                             synchronize=lambda: None, clock=time.monotonic):
    """Try the requested context, then at most once without context on bad output.

    transcribe is the existing Session.transcribe bound method. Runtime failures
    are recorded and raised immediately: an unhealthy model is not restarted.
    A legitimate spoken vocabulary list may match context; its unbiased retry
    is allowed to return the same text, avoiding a permanent content blacklist.
    """
    attempts = []
    context = context if context.strip() else ''
    for supplied in ([context, ''] if context else ['']):
        started = clock()
        result = None
        attempt = dict(context=supplied)
        try:
            result = transcribe(audio, language=language, context=supplied)
            synchronize()
        except Exception as error:
            attempt.update(compute_s=clock()-started, accepted=False,
                           rejection_reason='runtime_error', error=repr(error))
            if result is not None and is_dataclass(result):
                attempt['result'] = asdict(result)
            attempts.append(attempt)
            raise ASRContextRecoveryError('ASR runtime failed; no automatic retry', attempts) from error
        attempt.update(compute_s=clock()-started,
                       result=asdict(result) if is_dataclass(result) else dict(
                           text=getattr(result, 'text', None),
                           truncated=getattr(result, 'truncated', None),
                           finish_reason=getattr(result, 'finish_reason', None)))
        reason = rejection_reason(result, supplied)
        attempt.update(accepted=reason is None, rejection_reason=reason)
        attempts.append(attempt)
        if reason is None:
            return dict(result=result, attempts=attempts, used_fallback=len(attempts)>1,
                        total_compute_s=sum(a['compute_s'] for a in attempts))
    raise ASRContextRecoveryError('ASR output rejected after bounded attempts', attempts)
