#!/usr/bin/env python3
"""Local two-pass worker: PCM stream or file-clock replay, ASR and revisable MT."""
import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import queue
import threading
import time

from .mt import HyMT2
from .asr_models import zipformer
from .retranslation import Retranslation
from .pcm_input import PCMInput, AudioRing


class Events:
    def __init__(self, stream=None):
        self.stream = stream
        self.lock = threading.Lock()
        self.started = time.monotonic()
        self.audio_offset_s = 0.0
        self.revisions = {}
        self.final = {}
        self.last_draft = {}
        self.counts = {}

    def emit(self, kind, **data):
        with self.lock:
            sid = data.get('segment_id')
            if kind == 'source_draft':
                if sid in self.final or self.last_draft.get(sid) == data['text']:
                    return None
                self.last_draft[sid] = data['text']
            if kind in ('source_draft', 'source_final'):
                self.revisions[sid] = self.revisions.get(sid, 0) + 1
                data['source_revision'] = self.revisions[sid]
                if kind == 'source_final':
                    self.final[sid] = data['source_revision']
            if kind == 'translation_final':
                revisions = data.get('source_revisions', {sid: data.get('source_revision')})
                if not revisions or any(self.final.get(key) != revision or revision is None
                                        for key, revision in revisions.items()):
                    raise ValueError('translation does not match finalized source revision')
            if kind in ('translation_draft', 'translation_final'):
                data['after_audio_end_s'] = time.monotonic()-self.started-self.audio_offset_s-data.get('end_s', 0)
            event = dict(kind=kind, emission_s=time.monotonic() - self.started, **data)
            if self.stream is not None:
                self.stream.write(json.dumps(event, ensure_ascii=False) + '\n')
                self.stream.flush()
            self.counts[kind] = self.counts.get(kind, 0) + 1
            # Text remains in private output artifacts; stdout also permits live display.
            print(json.dumps(event, ensure_ascii=False), flush=True)
            return event


def sensevoice(path, language):
    import sherpa_onnx
    return sherpa_onnx.OfflineRecognizer.from_sense_voice(
        model=str(path / 'model.onnx'), tokens=str(path / 'tokens.txt'),
        provider='cpu', num_threads=1, language=language, use_itn=True)


def sense_text(model, samples):
    stream = model.create_stream()
    stream.accept_waveform(16000, samples)
    model.decode_stream(stream)
    return stream.result.text


def main():
    p = argparse.ArgumentParser(description=__doc__)
    inputs = p.add_mutually_exclusive_group(required=True)
    inputs.add_argument('--audio', type=Path)
    inputs.add_argument('--pcm-stdin', action='store_true', help='Live mono PCM16 16kHz; EOF stops and drains')
    p.add_argument('--output', type=Path, help='Optional private event output; live mode defaults to stdout only')
    p.add_argument('--language', choices=['ja', 'zh', 'en', 'ko'], default='ja')
    p.add_argument('--draft', choices=['sensevoice', 'zipformer'], default='sensevoice')
    p.add_argument('--draft-model', type=Path, required=True)
    p.add_argument('--final', choices=['qwen', 'sensevoice'], default='qwen')
    p.add_argument('--final-model', type=Path, required=True)
    p.add_argument('--vad-model', type=Path, required=True)
    p.add_argument('--translation-url', help='Optional loopback llama-server /v1 URL')
    p.add_argument('--translation-drafts', action=argparse.BooleanOptionalAction, default=True,
                   help='Retranslate active source drafts (disable to translate only closed groups)')
    a = p.parse_args()
    if a.draft == 'zipformer' and a.language == 'ja':
        p.error('no verified Japanese Zipformer checkpoint; use a local rolling draft')
    if a.output:
        a.output.mkdir(parents=True, exist_ok=False)
    import numpy as np
    import sherpa_onnx
    source = PCMInput(a.audio)
    pcm = AudioRing()
    config = sherpa_onnx.VadModelConfig()
    config.silero_vad.model = str(a.vad_model)
    config.silero_vad.threshold = .5
    config.silero_vad.min_silence_duration = .5
    config.silero_vad.min_speech_duration = .25
    config.silero_vad.max_speech_duration = 12
    vad = sherpa_onnx.VoiceActivityDetector(config, buffer_size_in_seconds=60)
    drafts, finals = queue.Queue(maxsize=1), queue.Queue(maxsize=8)
    draft_ready, final_ready = threading.Event(), threading.Event()
    errors, load_times = [], {}
    replaced, max_pending = 0, 0
    from contextlib import nullcontext
    with ((a.output / 'events.jsonl').open('w') if a.output else nullcontext()) as event_file:
        events = Events(event_file)
        translation = Retranslation(events.emit, language=a.language, drafts=a.translation_drafts)
        audio_clock = dict(seconds=0, speech=False, eof=False)

        def draft_worker():
            try:
                t = time.monotonic()
                model = sensevoice(a.draft_model, a.language) if a.draft == 'sensevoice' else zipformer(a.draft_model)
                load_times['draft_s'] = time.monotonic() - t
                draft_ready.set()
                native_stream, native_id, consumed = None, None, 0
                while True:
                    job = drafts.get()
                    if job is None:
                        break
                    before = time.monotonic()
                    if a.draft == 'sensevoice':
                        text = sense_text(model, job['samples'])
                    else:
                        if native_id != job['segment_id']:
                            native_stream = model.create_stream()
                            native_id, consumed = job['segment_id'], 0
                        native_stream.accept_waveform(16000, job['samples'][consumed:])
                        consumed = len(job['samples'])
                        if job['closed']:
                            native_stream.accept_waveform(16000, np.zeros(10560, np.float32))
                            native_stream.input_finished()
                        while model.is_ready(native_stream):
                            model.decode_stream(native_stream)
                        text = model.get_result(native_stream)
                    if text:
                        draft_event = events.emit('source_draft', segment_id=job['segment_id'],
                                    start_s=job['start_s'], end_s=job['end_s'], text=text,
                                    compute_s=time.monotonic()-before, provisional=True,
                                    mode='rolling-redecode' if a.draft == 'sensevoice' else 'native-stream')
                        translation.update(draft_event)
            except Exception as error:
                errors.append(repr(error))
                events.emit('worker_error', worker='draft', error=repr(error))
                draft_ready.set()

        def final_worker():
            try:
                t = time.monotonic()
                if a.final == 'qwen':
                    import mlx.core as mx
                    from mlx_qwen3_asr import Session
                    model = Session(model=str(a.final_model.resolve()), dtype=mx.float16)
                    mx.synchronize()
                else:
                    model = sensevoice(a.final_model, a.language)
                translator = HyMT2(a.translation_url, 'Hy-MT2-1.8B-Q4_K_M', 'Simplified Chinese') if a.translation_url else None
                load_times['final_s'] = time.monotonic() - t
                final_ready.set()
                input_done = False
                while True:
                    # Final ASR always precedes pending MT. An already-running
                    # HTTP generation is not preempted; drafts have no FIFO.
                    translation.advance(**audio_clock)
                    try:
                        job = finals.get(timeout=.02) if not input_done else None
                        if job is None:
                            input_done = True
                    except queue.Empty:
                        job = None
                    if job is None:
                        request = translation.take(time.monotonic()) if translator else None
                        if request:
                            t = time.monotonic()
                            translated = translation.cached(request)
                            if translated is None:
                                translated = translator.translate(request['source_text'], request['context'],
                                                                  request['lookahead_source'])
                            translation.complete(request, translated, time.monotonic()-t)
                            continue
                        if input_done and (not translator or not translation.pending()):
                            break
                        if input_done:
                            raise RuntimeError('translation groups incomplete after final ASR drain')
                        continue
                    before = time.monotonic()
                    if a.final == 'qwen':
                        language = {'ja':'Japanese','zh':'Chinese','en':'English','ko':'Korean'}[a.language]
                        result = model.transcribe((job['samples'], 16000), language=language)
                        mx.synchronize()
                        if result.truncated:
                            raise RuntimeError('final ASR hit token limit')
                        text = result.text
                    else:
                        text = sense_text(model, job['samples'])
                    if not text.strip():
                        raise RuntimeError('empty final ASR output')
                    final_event = events.emit('source_final', segment_id=job['segment_id'],
                        start_s=job['start_s'], end_s=job['end_s'], text=text,
                        compute_s=time.monotonic()-before,
                        queue_wait_s=before-job['queued_at'],
                        after_audio_end_s=time.monotonic()-events.started-events.audio_offset_s-job['end_s'],
                        completion_reason=job['reason'], model_final=True, human_verified=False)
                    translation.update(final_event)
            except Exception as error:
                errors.append(repr(error))
                events.emit('worker_error', worker='final', error=repr(error))
                final_ready.set()

        draft_thread = threading.Thread(target=draft_worker, name='local-draft', daemon=True)
        final_thread = threading.Thread(target=final_worker, name='segment-final', daemon=True)
        draft_thread.start(); final_thread.start()
        draft_ready.wait(); final_ready.wait()
        if errors:
            raise RuntimeError('; '.join(errors))
        events.started = time.monotonic()
        events.emit('session_ready', sample_rate=16000, channels=1, sample_format='s16le')
        starts, last_end, last_snapshot = {}, 0, -1.0

        def snapshot(segment, received, closed=False, reason=None):
            sid = 'segment-' + str(segment.start)
            start = starts.setdefault(sid, max(last_end, segment.start-3200, 0))
            end = min(segment.start+len(segment.samples), received, pcm.end)
            if end > start:
                translation.observe(sid, start/16000, end/16000, closed=closed)
            return dict(segment_id=sid, start_s=start/16000, end_s=end/16000,
                        samples=pcm.snapshot(start, end), closed=closed, reason=reason)

        def send_draft(job):
            nonlocal replaced
            try:
                drafts.put_nowait(job)
            except queue.Full:
                try:
                    drafts.get_nowait()
                    replaced += 1
                except queue.Empty:
                    pass
                drafts.put_nowait(job)

        def close_segments(received, eof=False):
            nonlocal last_end, max_pending
            while not vad.empty():
                segment = vad.front
                reason = 'input_eof' if eof else 'vad_or_max_duration'
                job = snapshot(segment, received, True, reason)
                if len(job['samples']):
                    events.emit('segment_closed', segment_id=job['segment_id'],
                                start_s=job['start_s'], end_s=job['end_s'], reason=reason)
                    send_draft(job)
                    job['queued_at'] = time.monotonic()
                    finals.put(job)
                    max_pending = max(max_pending, finals.qsize())
                    last_end = int(round(job['end_s']*16000))
                    starts.pop(job['segment_id'], None)
                vad.pop()

        for chunk in source:
            pcm.append(chunk)
            end = pcm.end
            if not a.audio and end == len(chunk):
                # Model/permission startup is separate from the sample clock.
                # Estimate the first frame's origin from its arrival; this is
                # not hardware timestamp or reference-word latency.
                events.audio_offset_s = time.monotonic()-events.started-end/16000
                events.emit('audio_started', audio_clock_offset_s=events.audio_offset_s)
            if a.audio:
                time.sleep(max(0, events.started+end/16000-time.monotonic()))
            if errors:
                break
            vad.accept_waveform(chunk)
            close_segments(end)
            audio_clock.update(seconds=end/16000, speech=vad.is_speech_detected())
            translation.advance(**audio_clock)
            interval = 1.0 if a.draft == 'sensevoice' else .032
            if vad.is_speech_detected() and end/16000-last_snapshot >= interval:
                job = snapshot(vad.current_segment, end)
                if len(job['samples']):
                    send_draft(job)
                    last_snapshot = end/16000
        vad.flush()
        close_segments(pcm.end, eof=True)
        audio_clock.update(seconds=pcm.end/16000, speech=False, eof=True)
        translation.advance(**audio_clock)
        # A dead worker must not prevent ending the other worker or saving the summary.
        if draft_thread.is_alive():
            try:
                drafts.get_nowait()
            except queue.Empty:
                pass
            drafts.put_nowait(None)
        finals.put(None)
        draft_thread.join(); final_thread.join()
        summary = dict(status='failed' if errors else 'completed', errors=errors,
            audio_seconds=pcm.end/16000,
            audio_sha256=hashlib.sha256(a.audio.read_bytes()).hexdigest() if a.audio else None,
            input_mode="wav-replay" if a.audio else "pcm-pipe", audio_ring_seconds=30,
            audio_clock_offset_s=events.audio_offset_s,
            draft=a.draft, final=a.final, language=a.language,
            draft_model=str(a.draft_model.resolve()), final_model=str(a.final_model.resolve()),
            vad_model_sha256=hashlib.sha256(a.vad_model.read_bytes()).hexdigest(),
            vad_config=str(config), pre_roll_s=.2, max_segment_s=12,
            sensevoice_draft_interval_s=1.0, draft_snapshots_replaced=replaced,
            peak_final_queue_depth=max_pending, load_times=load_times, counts=events.counts,
            sherpa_onnx=version('sherpa-onnx'), wall_s=time.monotonic()-events.started,
            translation_drafts=a.translation_drafts,
            translation_group_policy=dict(pause_s=1.2,
                max_seconds=20, max_chars=400, draft_interval_s=.8,
                bounds='at final ASR boundaries; one segment may overshoot', context_groups=1,
                sentence_lookahead_chars=8, commit_wait_after_qwen_s=4.0),
            note='file-clock demo; first draft is not verified-word latency; translation groups revise independently of ASR segments')
        if a.output:
            (a.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        events.emit('session_stopped', status=summary['status'], audio_seconds=summary['audio_seconds'])
        if errors:
            raise RuntimeError('; '.join(errors))


if __name__ == '__main__':
    main()
