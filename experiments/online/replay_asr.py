#!/usr/bin/env python3
"""Independent realtime file-clock ASR baseline and causal VAD experiments (no MT)."""
import argparse
import importlib.metadata
import json
import logging
import math
from pathlib import Path
import re
import signal
import struct
import subprocess
import sys
import time
import wave

from run_asr import file_sha256
from vad_plan import decisions

SCHEMA = 'atogaki.asr-replay.v2'
ROOT = Path(__file__).resolve().parents[2]


class PythonEngine:
    def __init__(self, args):
        sys.path.insert(0, str(args.checkout.resolve()))
        if args.backend == 'whisper-streaming':
            from whisper_online import MLXWhisper, OnlineASRProcessor
            self.asr = MLXWhisper(lan=args.language, model_dir=str(args.model.resolve()))
            self.online = OnlineASRProcessor(self.asr, None, buffer_trimming=('segment', 15))
        else:
            from simulstreaming_whisper import SimulWhisperASR, SimulWhisperOnline
            self.asr = SimulWhisperASR(language=args.language, model_path=str(args.model.resolve()),
                                     cif_ckpt_path=None, frame_threshold=25, audio_max_len=30,
                                     audio_min_len=0, segment_length=args.step, beams=1,
                                     task='transcribe', decoder_type='greedy', never_fire=False,
                                     init_prompt=None, static_init_prompt=None,
                                     max_context_tokens=None, logdir=None)
            self.online = SimulWhisperOnline(self.asr)
        self.backend = args.backend
        self.committed = ''

    def warmup(self, audio):
        if self.backend == 'whisper-streaming':
            self.asr.transcribe(audio)
        else:
            self.asr.warmup(audio)
        self.reset(0)

    def reset(self, offset):
        self.online.init(offset=offset)
        self.committed = ''

    def insert(self, audio):
        self.online.insert_audio_chunk(audio)

    def run(self, final=False):
        raw = self.online.finish() if final else self.online.process_iter()
        if self.backend == 'whisper-streaming':
            start, end, text = raw
            draft = '' if final else self.online.to_flush(self.online.transcript_buffer.complete())[2]
        else:
            start, end, text = raw.get('start'), raw.get('end'), raw.get('text', '')
            draft = ''  # This upstream adapter exposes AlignAtt commits only.
        if text:
            self.committed += text
        return dict(text=self.committed + draft, append=text, start_s=start, end_s=end,
                    stage='flushed' if final else ('draft' if draft else 'committed'), rollover=False)

    def close(self):
        pass


class CppEngine:
    """Official stream.cpp non-VAD window/rollover policy, with queued file input."""
    def __init__(self, args):
        self.args = args
        self.worker = subprocess.Popen([str(args.cpp_worker.resolve()), str(args.model.resolve()), args.language],
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        try:
            if json.loads(self.worker.stdout.readline()) != {'ready': True}:
                raise RuntimeError('C++ worker not ready')
            self.reset(0)
        except BaseException:
            self.close()
            raise

    def reset(self, offset):
        import numpy as np
        self.old = np.empty(0, dtype='float32')
        self.pending = []
        self.iteration = 0
        self.previous = ''

    def decode(self, audio):
        self.worker.stdin.write(struct.pack('<I', len(audio)))
        self.worker.stdin.write(audio.astype('<f4').tobytes())
        self.worker.stdin.flush()
        line = self.worker.stdout.readline()
        if not line:
            raise RuntimeError('C++ worker exited')
        return json.loads(line)['text']

    def warmup(self, audio):
        self.decode(audio)
        self.reset(0)

    def insert(self, audio):
        self.pending.append(audio)

    def run(self, final=False):
        import numpy as np
        if final:
            return dict(text=self.previous, append='', start_s=None, end_s=None,
                        stage='frozen', rollover=False)
        new = np.concatenate(self.pending)
        self.pending = []
        take = min(len(self.old), max(0, round((self.args.keep + self.args.length) * 16000) - len(new)))
        window = np.concatenate((self.old[-take:] if take else self.old[:0], new))
        self.previous = self.decode(window)
        self.old = window
        self.iteration += 1
        rollover = self.iteration % max(1, int(self.args.length / self.args.step) - 1) == 0
        if rollover:
            keep = round(self.args.keep * 16000)
            self.old = window[-keep:] if keep else window[:0]
        text = self.previous
        if rollover:
            self.previous = ''
        return dict(text=text, append='', start_s=None, end_s=None,
                    stage='draft', rollover=rollover)

    def close(self):
        if self.worker.poll() is None:
            self.worker.stdin.close()
            try:
                self.worker.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.worker.terminate()
                try:
                    self.worker.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.worker.kill()
                    self.worker.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['whisper-streaming', 'simulstreaming', 'whisper-cpp'], required=True)
    parser.add_argument('--checkout', type=Path)
    parser.add_argument('--cpp-worker', type=Path)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--audio', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--language', default='ja')
    parser.add_argument('--step', type=float, default=1)
    parser.add_argument('--length', type=float, default=5)
    parser.add_argument('--keep', type=float, default=.2)
    parser.add_argument('--vad', choices=['off', 'gate', 'endpoint'], default='off')
    parser.add_argument('--vad-plan', type=Path)
    parser.add_argument('--silence-ms', type=int, default=500)
    parser.add_argument('--max-wall-seconds', type=int, default=240)
    args = parser.parse_args()
    if not all(math.isfinite(x) for x in [args.step, args.length, args.keep]):
        parser.error('timing must be finite')
    if not 0 < args.step <= args.length <= 30 or not 0 <= args.keep <= args.step:
        parser.error('requires 0 < step <= length <= 30 and 0 <= keep <= step')
    if args.max_wall_seconds <= 0 or args.silence_ms <= 0:
        parser.error('timeouts must be positive')
    if args.backend == 'whisper-cpp' and not args.cpp_worker:
        parser.error('cpp-worker required')
    if args.backend != 'whisper-cpp' and not args.checkout:
        parser.error('checkout required')
    if args.vad != 'off' and not args.vad_plan:
        parser.error('VAD requires a causal probability plan')
    import numpy as np
    with wave.open(str(args.audio)) as wav:
        if (wav.getframerate(), wav.getnchannels(), wav.getsampwidth()) != (16000, 1, 2):
            parser.error('requires 16kHz mono PCM16')
        audio = np.frombuffer(wav.readframes(wav.getnframes()), dtype='<i2').astype('float32') / 32768
    duration = len(audio) / 16000
    digest = file_sha256(args.audio)
    actions = []
    plan = None
    if args.vad != 'off':
        plan = json.loads(args.vad_plan.read_text())
        if plan['audio_sha256'] != digest or not plan['causal']:
            parser.error('VAD plan must be causal and match audio digest')
        actions = list(decisions(plan['frames'], args.silence_ms / 1000))
    commit = None
    if args.checkout:
        commit = subprocess.check_output(['git', '-C', str(args.checkout), 'rev-parse', 'HEAD'], text=True).strip()
        dirty = subprocess.check_output(['git', '-C', str(args.checkout), 'status', '--porcelain'], text=True)
        if any('__pycache__' not in line and not line.endswith('.pyc') for line in dirty.splitlines()):
            parser.error('upstream checkout must be clean')
        pin = json.loads((ROOT / 'experiments/online/upstream-pins.json').read_text())[args.backend]['commit']
        if commit != pin:
            parser.error('upstream commit differs from pin')
    else:
        pins = dict(re.findall(r'^(WHISPER_\w+)="?([^"\n]+)"?$',
                              (ROOT / 'scripts/sidecar-versions.zsh').read_text(), re.M))
        commit = pins['WHISPER_COMMIT']
    model_files = [args.model / 'weights.npz', args.model / 'config.json'] if args.model.is_dir() else [args.model]
    args.output_dir.mkdir(parents=True, exist_ok=False)
    meta = dict(schema=SCHEMA, backend=args.backend, upstream_commit=commit,
                audio_sha256=digest, audio_duration_s=duration, timing_mode='realtime-file-clock-no-drop',
                input_policy='coalesce available PCM up to 30s; never drop; all times in original media clock',
                model_sha256={p.name: file_sha256(p) for p in model_files},
                model_path=str(args.model.resolve()), step_s=args.step, length_s=args.length, keep_s=args.keep,
                vad=args.vad, silence_ms=args.silence_ms, status='running',
                command=sys.argv, python=sys.executable, max_wall_seconds=args.max_wall_seconds,
                vad_plan_sha256=file_sha256(args.vad_plan) if args.vad_plan else None,
                vad_compute_total_s=plan['compute_total_s'] if plan else 0,
                vad_timing_note='Causal probabilities computed separately; replay never consumes future decisions. '
                                'VAD compute is not included in ASR clock; not a live VAD contention benchmark.',
                packages=sorted((d.metadata['Name'], d.version) for d in importlib.metadata.distributions()))
    if args.backend == 'whisper-cpp':
        meta['worker_sha256'] = file_sha256(args.cpp_worker)
        meta['precision_device'] = 'Q5_0/Metal; greedy; context disabled; stream window adapter, not SDL capture'
    else:
        meta['precision_device'] = 'fp16/MLX/Metal' if args.backend == 'whisper-streaming' else 'fp32/torch/CPU'
        lock = ROOT / 'experiments/online/envs' / args.backend / 'uv.lock'
        meta['uv_lock_sha256'] = file_sha256(lock)
    metadata_file = args.output_dir / 'run.json'
    metadata_file.write_text(json.dumps(meta, indent=2) + '\n')
    logging.basicConfig(level=logging.WARNING)
    before = time.monotonic()
    engine = None
    def expired(signum, frame):
        raise TimeoutError('ASR replay wall-time budget exceeded')
    signal.signal(signal.SIGALRM, expired)
    signal.signal(signal.SIGTERM, expired)
    signal.alarm(args.max_wall_seconds)
    try:
        engine = CppEngine(args) if args.backend == 'whisper-cpp' else PythonEngine(args)
        loaded = time.monotonic()
        engine.warmup(np.zeros(16000, dtype='float32'))
        start = time.monotonic()
        meta.update(load_s=loaded - before, warmup_s=start - loaded)
        with (args.output_dir / 'events.jsonl').open('x') as out:
            slot, pending, fed_end, position = 0, False, 0.0, 0.0
            active = args.vad == 'off'
            activated = active
            previous_display = ''
            action_index = 0
            def emit(kind, **data):
                out.write(json.dumps(dict(schema=SCHEMA, kind=kind, emission_s=time.monotonic() - start,
                                          **data), ensure_ascii=False) + '\n')
                out.flush()
            def insert_until(end, masked=False):
                nonlocal fed_end, pending
                lo, hi = round(fed_end * 16000), round(end * 16000)
                if hi > lo:
                    engine.insert(np.zeros(hi - lo, dtype='float32') if masked else audio[lo:hi])
                    pending = True
                fed_end = end
            def infer(final=False, reason=None):
                nonlocal pending, slot, previous_display
                t = time.monotonic()
                result = engine.run(final)
                cost = time.monotonic() - t
                emit('inference', audio_available_s=fed_end, compute_s=cost, final=final, reason=reason,
                     model_decode=(not final or args.backend == 'simulstreaming'))
                text = result['text']
                if result['append']:
                    emit('flush' if final else 'commit', text=result['append'],
                         start_s=result['start_s'], end_s=result['end_s'], slot=slot,
                         reason=reason, stage='flushed' if final else 'committed')
                if text != previous_display:
                    emit('display', text=text, stage=result['stage'], slot=slot,
                         audio_available_s=fed_end)
                    previous_display = text
                pending = False
                if result['rollover']:
                    emit('window_rollover', slot=slot, reason='periodic_window_not_stability_commit')
                    slot += 1
                    previous_display = ''
            while position < duration - 1e-9:
                target = min(duration, position + 30, max(position + args.step, time.monotonic() - start))
                delay = target - (time.monotonic() - start)
                if delay > 0:
                    time.sleep(delay)
                while action_index < len(actions) and actions[action_index]['decision_s'] <= target + 1e-9:
                    action = actions[action_index]
                    action_index += 1
                    emit('vad_decision', action=action['kind'],
                         **{k: v for k, v in action.items() if k != 'kind'})
                    if action['kind'] == 'start':
                        onset = action['audio_start_s']
                        if args.vad == 'endpoint' or not activated:
                            engine.reset(onset)
                            fed_end = onset
                            pending = False
                        elif onset > fed_end:
                            insert_until(onset, masked=True)
                        active = True
                        activated = True
                    else:
                        if active:
                            insert_until(action['decision_s'])
                            if pending:
                                infer(reason='vad_end_input')
                            if args.vad == 'endpoint':
                                infer(final=True, reason='vad_endpoint')
                                emit('segment_end', slot=slot, reason='vad_endpoint')
                                slot += 1
                                previous_display = ''
                        active = False
                if active:
                    insert_until(target)
                    if pending:
                        infer()
                position = target
            flush_start = time.monotonic()
            if pending:
                infer(reason='eof_pending')
            if activated and (active or args.vad != 'endpoint'):
                infer(final=True, reason='input_eof')
            emit('finish', flush_s=time.monotonic() - flush_start, reason='input_eof')
        meta['status'] = 'completed'
    except BaseException as error:
        meta.update(status='failed_or_cancelled', error_type=type(error).__name__, error=str(error))
        raise
    finally:
        signal.alarm(0)
        if engine:
            engine.close()
        meta['wall_time_s'] = time.monotonic() - before
        metadata_file.write_text(json.dumps(meta, indent=2) + '\n')


if __name__ == '__main__':
    main()
