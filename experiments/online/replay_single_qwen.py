#!/usr/bin/env python3
"""Controlled 1x causal replay: one Qwen+DFlash preview lane versus CPU drafts."""
import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import resource
import threading
import time

import _worker_path
from online_worker.pcm_input import PCMInput, AudioRing
from online_worker.stage_admission import StageAdmission
from online_worker.worker import Events, sensevoice, sense_text
from online_worker.mt import HyMT2
from live_asr_schedule import ASRMailbox, FinalMTQueue, replay_metrics


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--input-sha256', required=True)
    parser.add_argument('--model-dir', type=Path, required=True)
    parser.add_argument('--draft-dir', type=Path)
    parser.add_argument('--sensevoice-dir', type=Path)
    parser.add_argument('--mode', choices=['single-dflash', 'dual-control'], required=True)
    parser.add_argument('--audio-prefix-cache', action='store_true', help='Reuse only complete immutable audio blocks')
    parser.add_argument('--translation-url')
    parser.add_argument('--translation-model', default='Hy-MT2-1.8B-Q4_K_M')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if digest(args.inputs) != args.input_sha256:
        parser.error('frozen input hash mismatch')
    if args.mode == 'single-dflash' and not args.draft_dir:
        parser.error('single lane requires the pinned DFlash head')
    if args.mode == 'dual-control' and not args.sensevoice_dir:
        parser.error('dual control requires the installed CPU recognizer')
    if args.audio_prefix_cache and args.mode != 'single-dflash':
        parser.error('audio prefix cache is only supported by the isolated single DFlash lane')
    if version('mlx-qwen3-asr') != '0.4.4':
        parser.error('requires mlx-qwen3-asr 0.4.4')
    rows = json.loads(args.inputs.read_text())['rows']
    profile = json.loads((Path(__file__).parent / 'datasets/japanese-radio/confirmed-text-baseline.json').read_text())
    if [r['id'] for r in rows] != profile['active_segment_ids']:
        parser.error('only the current five confirmed crops may enter this run')
    for row in rows:
        if row['source_status'] != 'user_corrected_semantic_text' or digest(row['audio_path']) != row['audio_sha256']:
            parser.error('crop provenance mismatch')
    cpu_hashes = {str(p): digest(p) for p in args.sensevoice_dir.iterdir()
                  if p.name in ('model.onnx', 'tokens.txt')} if args.sensevoice_dir else {}
    args.output.mkdir(parents=True, exist_ok=False)
    metadata = dict(mode=args.mode, mt=bool(args.translation_url), input_sha256=args.input_sha256,
                    source_hashes={str(p): digest(p) for p in [Path(__file__), Path(__file__).with_name('qwen_speculative.py'),
                                 Path(__file__).with_name('live_asr_schedule.py'),
                                 Path(__file__).with_name('qwen_prefix_cache.py'),
                                 Path(_worker_path.__file__).parents[2] / 'src/infrastructure/online_worker/stage_admission.py']},
                    model_hashes={str(p): digest(p) for directory in [args.model_dir, *([args.draft_dir] if args.draft_dir else [])]
                                  for p in directory.glob('*.safetensors')},
                    packages={p: version(p) for p in ['mlx', 'mlx-metal', 'mlx-qwen3-asr', 'sherpa-onnx']},
                    conditions=dict(rate=1, refresh_seconds=1, max_new_tokens=512, target_dtype='float16', draft_dtype='bfloat16',
                                    cache_limit_bytes=512000000, boundaries='existing crop EOF; not natural VAD',
                                    history=[], lookahead='', scene='', translation='whole locked crop only',
                                    prefix_reuse=args.audio_prefix_cache, forced_text_prefix=False,
                                    sequence='back-to-back crops with independent semantic contexts',
                                    clocks='headless display-ready events; no native capture or paint'))
    metadata['model_hashes'].update(cpu_hashes)
    gpu_jobs, cpu_jobs = ASRMailbox(), ASRMailbox()
    mt_jobs = FinalMTQueue()
    admission = StageAdmission(mt_jobs if args.translation_url else None)
    gpu_ready, cpu_ready, running, asr_done = (threading.Event() for _ in range(4))
    errors, finalized, load_times = [], set(), {}
    publication = threading.Lock()
    with (args.output / 'events.jsonl').open('w') as stream:
        events = Events(stream)

        def fail(worker, error):
            errors.append(repr(error))
            gpu_jobs.finish(abort=True)
            cpu_jobs.finish(abort=True)
            admission.abort()
            events.emit('worker_error', worker=worker, error=repr(error))

        def publish(job, result, final, *, reused=False):
            with publication:
                if job['sid'] in finalized:
                    return
                if not result['text'].strip() or result.get('finish_reason') == 'length':
                    if final:
                        raise RuntimeError('invalid final ASR')
                    events.emit('preview_rejected', segment_id=job['sid'], reason=result.get('finish_reason', 'empty'))
                    return
                if final:
                    finalized.add(job['sid'])
                event = events.emit('source_final' if final else 'source_draft', segment_id=job['sid'],
                                    start_s=job['start_s'], end_s=job['start_s'] + job['end']/16000,
                                    text=result['text'], final_reused=reused, source_audio_samples=job['end'],
                                    compute_s=result['compute_s'], queue_wait_s=result['queue_wait_s'],
                                    after_input_end_s=time.monotonic()-events.started-job['start_s']-job['end']/16000,
                                    human_verified=False, completion_reason='crop_eof' if final else 'rolling_prefix')
                if final and args.translation_url:
                    mt_jobs.put(dict(sid=job['sid'], source_text=result['text'], end_s=event['end_s'],
                                     source_revision=event['source_revision']))

        def gpu_worker():
            try:
                import mlx.core as mx
                from mlx_qwen3_asr import Session
                from qwen_speculative import Target, Draft, transcribe
                before = time.monotonic()
                mx.set_cache_limit(512000000)
                session = Session(model=str(args.model_dir.resolve()), dtype=mx.float16)
                target = Target(session.model)
                draft = Draft.load(args.draft_dir, session.model.config.text_config, dtype=mx.bfloat16) if args.mode == 'single-dflash' else None
                if args.audio_prefix_cache:
                    from qwen_prefix_cache import CachedTranscriber
                    cached = CachedTranscriber(session, target, draft)
                else:
                    cached = None
                crop_id = None
                mx.synchronize()
                load_times['qwen_seconds'] = time.monotonic()-before
                gpu_ready.set()
                running.wait()
                while gpu_jobs.wait():
                    with admission.asr():
                        job = gpu_jobs.take()
                        if job is None:
                            continue
                        before = time.monotonic()
                        if before-events.started < job['start_s']+job['end']/16000:
                            raise AssertionError('ASR received future audio')
                        mx.reset_peak_memory()
                        if cached:
                            if crop_id != job['sid']:
                                cached.reset()
                                crop_id = job['sid']
                            result = cached.transcribe(job['samples'])
                        elif draft:
                            timings = {}
                            result = transcribe(session, target, draft, job['samples'], timings=timings)
                            result['timings'] = timings
                        else:
                            value = session.transcribe(job['samples'], language='Japanese', max_new_tokens=512, return_chunks=True)
                            result = dict(text=value.text, finish_reason=value.finish_reason)
                        mx.synchronize()
                        result.update(compute_s=time.monotonic()-before, queue_wait_s=before-job['queued_at'])
                        final = gpu_jobs.complete(job, result)
                        events.emit('asr_call', segment_id=job['sid'], model='qwen-dflash' if draft else 'qwen',
                                    final=final, input_samples=job['end'], start_s=job['start_s'],
                                    compute_s=result['compute_s'], queue_wait_s=result['queue_wait_s'],
                                    peak_active_bytes=mx.get_peak_memory(), active_bytes=mx.get_active_memory(),
                                    cache_bytes=mx.get_cache_memory(), finish_reason=result['finish_reason'],
                                    timings=result.get('timings'), audio_cache=result.get('audio_cache'))
                        publish(job, result, final)
                        mx.clear_cache()
            except Exception as error:
                fail('qwen', error)
            finally:
                gpu_ready.set()
                asr_done.set()

        def cpu_worker():
            try:
                before = time.monotonic()
                model = sensevoice(args.sensevoice_dir, 'ja')
                load_times['sensevoice_seconds'] = time.monotonic()-before
                cpu_ready.set()
                running.wait()
                while cpu_jobs.wait():
                    job = cpu_jobs.take()
                    if job is None:
                        continue
                    before = time.monotonic()
                    result = dict(text=sense_text(model, job['samples']), compute_s=time.monotonic()-before,
                                  queue_wait_s=before-job['queued_at'])
                    cpu_jobs.complete(job, result)
                    events.emit('asr_call', segment_id=job['sid'], model='sensevoice', final=False,
                                input_samples=job['end'], start_s=job['start_s'], compute_s=result['compute_s'],
                                queue_wait_s=result['queue_wait_s'])
                    publish(job, result, False)
            except Exception as error:
                fail('sensevoice', error)
            finally:
                cpu_ready.set()

        def mt_worker():
            try:
                translator = HyMT2(args.translation_url, args.translation_model, 'Simplified Chinese')
                running.wait()
                while not errors:
                    request = admission.take_mt()
                    if request:
                        before = time.monotonic()
                        try:
                            text = translator.translate(request['source_text'], (), '')
                            events.emit('translation_final', segment_id=request['sid'],
                                        source_revision=request['source_revision'], end_s=request['end_s'],
                                        text=text, compute_s=time.monotonic()-before,
                                        queue_wait_s=before-request['queued_at'], context=[], lookahead_source='')
                        finally:
                            admission.complete_mt()
                    elif asr_done.is_set() and mt_jobs.jobs.empty():
                        break
                    else:
                        time.sleep(.01)
            except Exception as error:
                fail('mt', error)

        gpu_thread = threading.Thread(target=gpu_worker, name='single-qwen-owner', daemon=True)
        cpu_thread = threading.Thread(target=cpu_worker, name='sensevoice-control', daemon=True) if args.mode == 'dual-control' else None
        mt_thread = threading.Thread(target=mt_worker, name='controlled-mt', daemon=True) if args.translation_url else None
        gpu_thread.start()
        if cpu_thread:
            cpu_thread.start()
        else:
            cpu_ready.set()
        if mt_thread:
            mt_thread.start()
        if not gpu_ready.wait(180) or not cpu_ready.wait(180):
            raise RuntimeError('ASR model readiness timeout')
        if errors:
            running.set()
            raise RuntimeError('; '.join(errors))
        metadata['load_times'] = load_times
        (args.output / 'metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')
        events.started = time.monotonic()
        running.set()
        offset = 0.0
        for row in rows:
            if errors:
                break
            source = PCMInput(row['audio_path'])
            ring = AudioRing(seconds=30)
            events.emit('crop_start', segment_id=row['id'], start_s=offset, duration_s=source.frames/16000)
            next_refresh = 16000
            for samples in source:
                due = events.started+offset+(ring.end+len(samples))/16000
                time.sleep(max(0, due-time.monotonic()))
                ring.append(samples)
                if ring.end >= next_refresh and ring.end < source.frames:
                    job = dict(sid=row['id'], start_s=offset, end=ring.end, samples=ring.snapshot(0, ring.end), queued_at=time.monotonic())
                    (gpu_jobs if args.mode == 'single-dflash' else cpu_jobs).preview(job)
                    next_refresh = (ring.end//16000+1)*16000
                if errors:
                    break
            if errors:
                break
            job = dict(sid=row['id'], start_s=offset, end=ring.end, samples=ring.snapshot(0, ring.end), queued_at=time.monotonic())
            events.emit('crop_end', segment_id=row['id'], end_s=offset+ring.end/16000)
            cpu_jobs.discard(row['id'])
            reused = gpu_jobs.close(job)
            if reused is not None:
                publish(job, reused, True, reused=True)
            offset += ring.end/16000
        gpu_jobs.finish(abort=bool(errors))
        cpu_jobs.finish(abort=bool(errors))
        threads = [t for t in [gpu_thread, cpu_thread, mt_thread] if t]
        deadline = time.monotonic()+45
        for thread in threads:
            thread.join(timeout=max(0, deadline-time.monotonic()))
        alive = [t.name for t in threads if t.is_alive()]
        if alive:
            admission.abort()
            gpu_jobs.finish(abort=True)
            cpu_jobs.finish(abort=True)
        summary = dict(elapsed_s=time.monotonic()-events.started, source_audio_s=offset,
                       finals=sorted(finalized), counts=events.counts, errors=errors, live_threads=alive,
                       replaced_previews=gpu_jobs.replaced if args.mode == 'single-dflash' else cpu_jobs.replaced,
                       final_reuses=gpu_jobs.final_reuses, final_upgrades=gpu_jobs.final_upgrades,
                       process_max_rss_bytes=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
        stream.flush()
        recorded = [json.loads(line) for line in (args.output / 'events.jsonl').read_text().splitlines()]
        summary['metrics'] = replay_metrics(recorded)
        (args.output / 'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        if errors or alive or len(finalized) != len(rows):
            raise RuntimeError('controlled replay did not complete; inspect summary')


if __name__ == '__main__':
    main()
