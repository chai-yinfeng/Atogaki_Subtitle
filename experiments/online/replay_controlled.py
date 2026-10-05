#!/usr/bin/env python3
"""v3 ASR-only controlled replay: independent feed/decode clocks, explicit tail actions."""
import argparse
import json
import math
import resource
import subprocess
import importlib.metadata
import signal
import sys
import time
import wave
from pathlib import Path
from controlled_clock import FeedClock, next_decode
from controlled_engine import ControlledPython, ControlledCpp
from run_asr import file_sha256

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = 'atogaki.asr-replay.v3'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--backend', choices=['whisper-streaming', 'simulstreaming', 'whisper-cpp'], required=True)
    p.add_argument('--model-root', type=Path, required=True)
    p.add_argument('--case', type=Path, required=True)
    p.add_argument('--frozen-config', type=Path)
    p.add_argument('--output-dir', type=Path, required=True)
    p.add_argument('--decode-ms', type=int, choices=[500, 1000], default=500)
    p.add_argument('--vad', choices=['off', 'cached', 'live'], default='off')
    p.add_argument('--vad-plan', type=Path)
    p.add_argument('--tail-decode', action='store_true')
    p.add_argument('--finish-on-pause', action='store_true')
    p.add_argument('--reset-on-pause', action='store_true')
    p.add_argument('--mask-silence', action='store_true')
    p.add_argument('--silence-ms', type=int, choices=[250, 500, 800], default=500)
    p.add_argument('--device', choices=['mps', 'cpu'], default='mps')
    p.add_argument('--trimming', type=int, choices=[5, 15], default=15)
    p.add_argument('--frame-threshold', type=int, choices=[10, 25, 40], default=25)
    p.add_argument('--length', type=int, choices=[3, 5, 8], default=5)
    p.add_argument('--max-wall-seconds', type=int)
    a = p.parse_args()
    if a.reset_on_pause and not a.finish_on_pause or a.finish_on_pause and not a.tail_decode:
        p.error('tail action ladder requires decode -> finish -> reset')
    if a.vad == 'off' and any([a.tail_decode, a.finish_on_pause, a.mask_silence]):
        p.error('pause actions require VAD')
    case = json.loads(a.case.read_text())
    if case['role'] not in ['development', 'holdout', 'long_regression', 'failure_regression', 'smoke']:
        p.error('unknown sample role')
    if case['role'] == 'holdout':
        if not a.frozen_config or file_sha256(a.frozen_config) != case.get('frozen_config_sha256'):
            p.error('holdout requires the matching frozen candidate configuration')
        candidate = json.loads(a.frozen_config.read_text())['candidates'][a.backend]
        if (candidate['size'], candidate['interval'], candidate.get('trimming',15), candidate.get('frame_threshold',25), candidate.get('length',5)) != (a.model_root.name, a.decode_ms, a.trimming, a.frame_threshold, a.length):
            p.error('holdout candidate differs from freeze')
    audio_path = a.case.parent / 'audio.wav'
    if file_sha256(audio_path) != case['audio_sha256']:
        p.error('case PCM digest mismatch')
    import numpy as np
    with wave.open(str(audio_path)) as w:
        if (w.getframerate(), w.getnchannels(), w.getsampwidth()) != (16000, 1, 2):
            p.error('16k mono PCM16 required')
        audio = np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype('float32') / 32768
    duration = len(audio) / 16000
    a.step, a.keep, a.language = a.decode_ms / 1000, .2, 'ja'
    upstream = ROOT / 'local-artifacts/online/upstream'
    a.checkout = upstream / ('whisper_streaming' if a.backend == 'whisper-streaming' else 'SimulStreaming')
    a.cpp_worker = ROOT / 'local-artifacts/online/whisper-cpp-build/infer-worker'
    a.model = a.model_root / ('mlx' if a.backend == 'whisper-streaming' else 'ggml-model.bin' if a.backend == 'whisper-cpp' else a.model_root.name + '.pt')
    provenance = json.loads((a.model_root / 'provenance.json').read_text())
    if a.backend != 'whisper-cpp':
        pin = json.loads((Path(__file__).parent / 'upstream-pins.json').read_text())[a.backend]['commit']
        actual = subprocess.check_output(['git', '-C', str(a.checkout), 'rev-parse', 'HEAD'], text=True).strip()
        dirty = subprocess.check_output(['git', '-C', str(a.checkout), 'status', '--porcelain'], text=True)
        if actual != pin or any('__pycache__' not in l and not l.endswith('.pyc') for l in dirty.splitlines()):
            p.error('upstream differs from clean pin')
    for name, sha in provenance['artifacts'].items():
        if file_sha256(a.model_root / name) != sha:
            p.error('model provenance mismatch: ' + name)
    plan = json.loads(a.vad_plan.read_text()) if a.vad_plan else None
    if a.vad == 'cached' and (not plan or not plan['causal'] or plan['audio_sha256'] != case['audio_sha256']):
        p.error('matching causal VAD plan required')
    files = ['replay_controlled.py', 'controlled_clock.py', 'controlled_engine.py', 'replay_asr.py', 'vad_plan.py', 'live_vad_worker.py']
    meta = dict(schema=SCHEMA, status='loading', case=case, audio_sha256=case['audio_sha256'], audio_duration_s=duration,
                config={k: str(v) if isinstance(v, Path) else v for k, v in vars(a).items()}, feed_ms=32,
                lock_hashes={b: file_sha256(ROOT / 'experiments/online/envs' / b / 'uv.lock') for b in ['whisper-streaming','simulstreaming']},
                packages=sorted((d.metadata['Name'], d.version) for d in importlib.metadata.distributions()),
                provenance=provenance, source_hashes={f: file_sha256(Path(__file__).parent / f) for f in files},
                reference_status='unreviewed', dropped_audio_s=0, command=sys.argv,
                upstream_pins=json.loads((Path(__file__).parent / 'upstream-pins.json').read_text()),
                worker_sha256=file_sha256(a.cpp_worker) if a.backend == 'whisper-cpp' else None,
                vad_plan_sha256=file_sha256(a.vad_plan) if a.vad_plan else None,
                decode_contract='greedy/beam1; temperature0; no temperature fallback; empty initial prompt; native context rules',
                peak_memory_note='macOS process ru_maxrss bytes; excludes CPP child/Metal allocations reported separately')
    a.output_dir.mkdir(parents=True, exist_ok=False)
    metadata = a.output_dir / 'run.json'
    metadata.write_text(json.dumps(meta, indent=2) + '\n')
    engine, feed, vad_worker = None, None, None
    before = time.monotonic()
    def cancel(*_):
        raise TimeoutError('cancelled or replay budget exceeded')
    signal.signal(signal.SIGALRM, cancel)
    signal.signal(signal.SIGTERM, cancel)
    signal.alarm(a.max_wall_seconds or math.ceil(duration + 120))
    try:
        engine = ControlledCpp(a) if a.backend == 'whisper-cpp' else ControlledPython(a)
        loaded = time.monotonic()
        engine.warmup(np.zeros(16000, dtype='float32'))
        live = None
        if a.vad == 'live':
            python = ROOT / 'experiments/online/envs/simulstreaming/.venv/bin/python'
            vad_worker = subprocess.Popen([str(python), str(Path(__file__).parent / 'live_vad_worker.py')],
                                           stdin=subprocess.PIPE, stdout=subprocess.PIPE)
            ready = json.loads(vad_worker.stdout.readline())
            if not ready['ready']:
                raise RuntimeError('live VAD not ready')
            meta['live_vad_model_sha256'] = ready['model_sha256']
            def live(data, lo, hi):
                t = time.monotonic()
                vad_worker.stdin.write(np.pad(data, (0, 512-len(data))).astype('<f4').tobytes())
                vad_worker.stdin.flush()
                line = vad_worker.stdout.readline()
                if not line:
                    raise RuntimeError('live VAD exited')
                result = json.loads(line)
                return dict(start_s=lo, end_s=hi, **result, service_s=time.monotonic()-t)
        if hasattr(engine, 'sync'):
            engine.sync()
        start = time.monotonic()
        engine.preprocessing_s = 0.0
        meta.update(load_s=loaded - before, warmup_s=start - loaded, runtime_audit=engine.audit)
        feed = FeedClock(audio, plan=plan if a.vad == 'cached' else None, live=live, silence_s=a.silence_ms / 1000)
        with (a.output_dir / 'events.jsonl').open('x') as out:
            def emit(kind, **data):
                out.write(json.dumps(dict(schema=SCHEMA, kind=kind, emission_s=time.monotonic() - start, **data), ensure_ascii=False) + '\n')
                out.flush()
            fed, slot, action_index, previous, due = 0, 0, 0, '', a.step
            active = a.vad == 'off'
            activated = active
            pending = False
            finalized_at = -1
            def insert(hi, mask=False):
                nonlocal fed, pending, finalized_at
                if hi > fed:
                    chunk = audio[fed:hi].copy()
                    if mask:
                        # Only decisions delivered in this snapshot may affect masking.
                        lower = 0
                        for event in actions[:action_index]:
                            if event['kind'] == 'start':
                                upper = round(event['audio_start_s'] * 16000)
                                chunk[max(0, lower - fed):max(0, min(hi, upper) - fed)] = 0
                            else:
                                lower = round(event['decision_s'] * 16000)
                        if not active:
                            chunk[max(0, lower - fed):] = 0
                    engine.insert(chunk)
                    fed, pending, finalized_at = hi, True, -1
            def infer(final=False, reason='scheduled'):
                nonlocal pending, previous, slot, finalized_at
                t = time.monotonic()
                result = engine.run(final)
                emit('inference', audio_available_s=fed / 16000, compute_s=time.monotonic() - t,
                     model_decode=(not final or a.backend == 'simulstreaming'), final=final, reason=reason)
                if result['append']:
                    emit('flush' if final else 'commit', text=result['append'], slot=slot,
                         start_s=result['start_s'], end_s=result['end_s'], reason=reason)
                if result['text'] != previous:
                    emit('display', text=result['text'], slot=slot, stage=result['stage'], audio_available_s=fed / 16000)
                    previous = result['text']
                if result['rollover']:
                    emit('window_rollover', slot=slot, reason='freeze_not_algorithm_commit')
                    slot, previous = slot + 1, ''
                pending = False
                if final:
                    finalized_at = fed
            feed.start(start)
            while True:
                cursor, actions, done = feed.snapshot()
                while action_index < len(actions):
                    action = actions[action_index]
                    action_index += 1
                    emit('vad_decision', **{('action' if k == 'kind' else k): v for k, v in action.items()})
                    if action['kind'] == 'end' and a.tail_decode:
                        boundary = min(cursor, round(action['decision_s'] * 16000))
                        insert(boundary)
                        if pending:
                            infer(reason='tail_decode')
                        if a.finish_on_pause:
                            infer(True, 'pause_finish')
                            emit('segment_end', slot=slot)
                        if a.reset_on_pause:
                            slot, previous = slot + 1, ''
                            engine.reset(fed / 16000)
                    active = action['kind'] == 'start'
                    activated = activated or active
                now = time.monotonic() - start
                eof_cursor = cursor
                if done and a.vad != 'off' and not active:
                    eof_cursor = max(fed, round(actions[-1]['decision_s'] * 16000)) if activated else 0
                if (now >= due and active) or done:
                    hi = min(eof_cursor if done else cursor, fed + 30 * 16000)
                    insert(hi, mask=a.mask_silence)
                    if pending:
                        infer(reason='eof_pending' if done else 'scheduled')
                    due = next_decode(time.monotonic() - start, a.step)
                emit_needed = done and fed == eof_cursor
                if emit_needed:
                    t = time.monotonic()
                    if activated and finalized_at != fed:
                        infer(True, 'input_eof')
                    emit('finish', flush_s=time.monotonic() - t)
                    break
                time.sleep(.005)
        meta.update(status='completed', preprocessing_s=engine.preprocessing_s,
                    vad_compute_s=feed.compute_s, consumed_audio_s=fed / 16000, delivered_audio_s=feed.cursor / 16000,
                    asr_unprocessed_trailing_silence_s=duration - fed / 16000,
                    peak_rss_bytes=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                    child_peak_rss_bytes=None, child_peak_note='worker peak observed after shutdown; separate from parent, not additive total RAM')
        if a.backend == 'simulstreaming' and a.device == 'mps':
            import torch
            meta['mps_allocated_bytes'] = torch.mps.current_allocated_memory()
            meta['mps_driver_allocated_bytes'] = torch.mps.driver_allocated_memory()
        elif a.backend == 'whisper-streaming':
            import mlx.core as mx
            meta['mlx_peak_bytes'] = mx.get_peak_memory()
        if a.vad == 'live':
            (a.output_dir / 'live-vad.json').write_text(json.dumps(dict(frames=feed.frames, compute_total_s=feed.compute_s)) + '\n')
    except BaseException as e:
        meta.update(status='failed_or_cancelled', error_type=type(e).__name__, error=str(e))
        raise
    finally:
        signal.alarm(0)
        try:
            # Terminate the private worker first so a blocked producer read unblocks.
            if vad_worker:
                vad_worker.terminate()
                try:
                    vad_worker.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    vad_worker.kill()
                    vad_worker.wait()
            if feed:
                feed.close()
        finally:
            if engine:
                engine.close()
        if a.backend == 'whisper-cpp':
            meta['cpp_library_hashes'] = {str(f.resolve()): file_sha256(f.resolve()) for f in (ROOT / 'local-artifacts/online/whisper-cpp-build').rglob('*.dylib')}
        meta['child_peak_rss_bytes'] = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        meta['wall_time_s'] = time.monotonic() - before
        metadata.write_text(json.dumps(meta, indent=2) + '\n')


if __name__ == '__main__':
    main()
