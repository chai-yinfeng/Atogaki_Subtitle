#!/usr/bin/env python3
"""Local file-clock two-pass demo: replaceable drafts, VAD, segment ASR, MT."""
import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import queue
import threading
import time
import wave

from harness import HyMT2
from run_zipformer import zipformer


class Events:
    def __init__(self, stream):
        self.stream = stream
        self.lock = threading.Lock()
        self.started = time.monotonic()
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
            if kind == 'translation_final' and self.final.get(sid) != data['source_revision']:
                raise ValueError('translation does not match finalized source revision')
            event = dict(kind=kind, emission_s=time.monotonic() - self.started, **data)
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
    p.add_argument('--audio', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--language', choices=['ja', 'zh', 'en', 'ko'], default='ja')
    p.add_argument('--draft', choices=['sensevoice', 'zipformer'], default='sensevoice')
    p.add_argument('--draft-model', type=Path, required=True)
    p.add_argument('--final', choices=['qwen', 'sensevoice'], default='qwen')
    p.add_argument('--final-model', type=Path, required=True)
    p.add_argument('--vad-model', type=Path, required=True)
    p.add_argument('--translation-url', help='Optional loopback llama-server /v1 URL')
    a = p.parse_args()
    if a.draft == 'zipformer' and a.language == 'ja':
        p.error('no verified Japanese Zipformer checkpoint; use a local rolling draft')
    a.output.mkdir(parents=True, exist_ok=False)
    import numpy as np
    import sherpa_onnx
    with wave.open(str(a.audio)) as audio:
        if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate()) != (1, 2, 16000):
            p.error('input must be mono PCM16 16kHz WAV')
        pcm = np.frombuffer(audio.readframes(audio.getnframes()), dtype='<i2').astype(np.float32) / 32768
    config = sherpa_onnx.VadModelConfig()
    config.silero_vad.model = str(a.vad_model)
    config.silero_vad.threshold = .5
    config.silero_vad.min_silence_duration = .5
    config.silero_vad.min_speech_duration = .25
    config.silero_vad.max_speech_duration = 12
    vad = sherpa_onnx.VoiceActivityDetector(config, buffer_size_in_seconds=60)
    drafts, finals = queue.Queue(maxsize=1), queue.Queue()
    draft_ready, final_ready = threading.Event(), threading.Event()
    errors, load_times = [], {}
    replaced, max_pending = 0, 0
    with (a.output / 'events.jsonl').open('w') as event_file:
        events = Events(event_file)

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
                        events.emit('source_draft', segment_id=job['segment_id'],
                                    start_s=job['start_s'], end_s=job['end_s'], text=text,
                                    compute_s=time.monotonic()-before, provisional=True,
                                    mode='rolling-redecode' if a.draft == 'sensevoice' else 'native-stream')
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
                while True:
                    job = finals.get()
                    if job is None:
                        break
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
                        after_audio_end_s=time.monotonic()-events.started-job['end_s'],
                        completion_reason=job['reason'], model_final=True, human_verified=False)
                    if translator:
                        t = time.monotonic()
                        translated = translator.translate(text)
                        events.emit('translation_final', segment_id=job['segment_id'],
                            source_revision=final_event['source_revision'], text=translated,
                            start_s=job['start_s'], end_s=job['end_s'],
                            compute_s=time.monotonic()-t,
                            after_audio_end_s=time.monotonic()-events.started-job['end_s'],
                            completion_reason=job['reason'])
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
        starts, last_end, last_snapshot = {}, 0, -1.0

        def snapshot(segment, received, closed=False, reason=None):
            sid = 'segment-' + str(segment.start)
            start = starts.setdefault(sid, max(last_end, segment.start-3200, 0))
            end = min(segment.start+len(segment.samples), received, len(pcm))
            return dict(segment_id=sid, start_s=start/16000, end_s=end/16000,
                        samples=pcm[start:end].copy(), closed=closed, reason=reason)

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
                vad.pop()

        for offset in range(0, len(pcm), 512):
            end = min(offset+512, len(pcm))
            time.sleep(max(0, events.started+end/16000-time.monotonic()))
            if errors:
                break
            vad.accept_waveform(pcm[offset:end])
            close_segments(end)
            interval = 1.0 if a.draft == 'sensevoice' else .032
            if vad.is_speech_detected() and end/16000-last_snapshot >= interval:
                job = snapshot(vad.current_segment, end)
                if len(job['samples']):
                    send_draft(job)
                    last_snapshot = end/16000
        vad.flush()
        close_segments(len(pcm), eof=True)
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
            audio_seconds=len(pcm)/16000, audio_sha256=hashlib.sha256(a.audio.read_bytes()).hexdigest(),
            draft=a.draft, final=a.final, language=a.language,
            draft_model=str(a.draft_model.resolve()), final_model=str(a.final_model.resolve()),
            vad_model_sha256=hashlib.sha256(a.vad_model.read_bytes()).hexdigest(),
            vad_config=str(config), pre_roll_s=.2, max_segment_s=12,
            sensevoice_draft_interval_s=1.0, draft_snapshots_replaced=replaced,
            peak_final_queue_depth=max_pending, load_times=load_times, counts=events.counts,
            sherpa_onnx=version('sherpa-onnx'), wall_s=time.monotonic()-events.started,
            note='file-clock demo; first nonempty draft is not verified-word latency; per-VAD-segment MT is provisional')
        (a.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        if errors:
            raise RuntimeError('; '.join(errors))


if __name__ == '__main__':
    main()
