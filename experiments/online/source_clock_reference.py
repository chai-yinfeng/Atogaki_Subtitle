"""Offline reference for source-clock grouping, NOT a production scheduler.

Reuses production grouping hints; freezes membership at closure and final source
and right-context snapshots at readiness. A virtual MT consumer may be arbitrarily
busy without changing that plan. Recorded capture ranges are still reconstructed:
this is neither fresh VAD nor a resource-contention/GUI latency measurement.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path

from retranslation import Retranslation


def signature(snapshot):
    return (snapshot['final'], tuple(snapshot['source_revisions'].items()),
            snapshot['source_text'], snapshot['lookahead_source'])


def compile_plan(events, duration, *, commit_wait=8, max_seconds=20, tick=.1):
    if not math.isfinite(duration) or duration <= 0 or not math.isfinite(tick) or tick <= 0:
        raise ValueError('positive finite duration/tick required')
    if not math.isfinite(commit_wait) or commit_wait < 0 or not math.isfinite(max_seconds) or max_seconds <= 0:
        raise ValueError('finite nonnegative wait and positive max_seconds required')
    arrivals = [e for e in events if e['kind'] in ('source_draft', 'source_final', 'segment_closed')]
    times = [e['emission_s'] for e in arrivals]
    if any(not math.isfinite(t) or t < 0 for t in times) or times != sorted(times):
        raise ValueError('source arrivals must have finite monotonic emission_s')
    ranges = {e['segment_id']: (e['start_s'], e['end_s']) for e in arrivals if e['kind'] == 'segment_closed'}
    if any(not (0 <= lo < hi <= duration) for lo, hi in ranges.values()):
        raise ValueError('capture ranges must lie within the replay')
    expected = [e['segment_id'] for e in arrivals if e['kind'] == 'source_final']
    if len(expected) != len(set(expected)) or set(expected) != set(ranges):
        raise ValueError('each capture unit requires exactly one final source')
    if any(not e['text'].strip() for e in arrivals if e['kind'] == 'source_final'):
        raise ValueError('empty final source is not a translation reference')
    mt = Retranslation(lambda *_a, **_k: None, language='ja',
                       commit_wait=commit_wait, max_seconds=max_seconds)
    final_at, sealed, previous, ready, output = {}, {}, {}, set(), []
    end = max([duration, *times])
    clocks = sorted(set(times+[duration]+[round(i*tick, 9) for i in range(math.ceil(end/tick)+1)]))
    index = 0
    for at in clocks:
        # At equal timestamps all source updates precede timeout/quiet/EOF checks.
        mt.audio_seconds = min(at, duration)
        while index < len(arrivals) and arrivals[index]['emission_s'] <= at:
            event = arrivals[index]
            sid = event['segment_id']
            start, end_s = ranges[sid]
            mt.observe(sid, start, min(event['end_s'], end_s),
                       closed=event['kind'] in ('source_final', 'segment_closed'))
            if event['kind'] != 'segment_closed':
                mt.update(event)
            if event['kind'] == 'source_final':
                final_at[sid] = at
            index += 1
        speech = any(lo <= min(at, duration) < hi for lo, hi in ranges.values())
        mt.advance(min(at, duration), speech=speech, eof=at >= duration)
        # No MT result or history can participate in this source-only decision.
        while True:
            before = [(g.id, g.reason, tuple(g.segments)) for g in mt.groups]
            for group in list(mt.groups):
                if group.reason is None:
                    mt._commit_prefix(group)
            if before == [(g.id, g.reason, tuple(g.segments)) for g in mt.groups]:
                break
        for gi, group in enumerate(mt.groups):
            members = tuple(group.segments)
            if group.id in sealed and sealed[group.id] != members:
                raise AssertionError('closed membership changed')
            if group.reason is not None and group.id not in sealed:
                sealed[group.id] = members
                output.append(dict(kind='group_closed', emission_s=at, group_id=group.id,
                                   source_ids=list(members), completion_reason=group.reason))
            if group.id in ready:
                continue
            text = mt._text(group)
            final = group.reason is not None and all(s['final'] for s in group.segments.values())
            right, right_ids = [], []
            for following in mt.groups[gi+1:]:
                for sid, segment in following.segments.items():
                    if not segment['final']:
                        break
                    right.append(segment['text'])
                    right_ids.append(sid)
                if any(not s['final'] for s in following.segments.values()):
                    break
            snapshot = dict(kind='snapshot', emission_s=at, group_id=group.id, final=final,
                            source_revisions={sid: s['revision'] for sid, s in group.segments.items()},
                            source_text=text, lookahead_source=mt._join(right)[:500],
                            right_source_ids=right_ids, start_s=group.start, end_s=group.end,
                            first_source_end_s=next(iter(group.segments.values()))['end'],
                            completion_reason=group.reason)
            if final:
                assert all(final_at[sid] <= at for sid in (*members, *right_ids))
                ready.add(group.id)
            key = signature(snapshot)
            if previous.get(group.id) != key:
                output.append(snapshot)
                previous[group.id] = key
    finals = [e for e in output if e['kind'] == 'snapshot' and e['final']]
    coverage = [sid for e in finals for sid in e['source_revisions']]
    if len(coverage) != len(set(coverage)) or set(coverage) != set(expected):
        raise ValueError('source plan did not ready each source exactly once')
    return dict(events=output, source_segments=len(expected), source_ids=sorted(expected), duration_s=duration,
                settings=dict(commit_wait=commit_wait, max_seconds=max_seconds, tick=tick),
                notes=['Source updates before deadlines at equal timestamps.',
                       'Closure freezes membership; readiness freezes source/right context.',
                       'Recorded capture ranges reconstructed; no fresh VAD.',
                       'Full plan retained for offline audit, not a bounded live producer.'])


def consume(plan, cost, *, interval=.8, pending_limit=128):
    """Virtual single MT slot, coalesced drafts, ordered commits and two final attempts.

    cost(snapshot, attempt) returns (seconds, fails). Text is a synthetic digest,
    never a semantic judgment. Only events with emission_s <= now enter state.
    """
    events = plan['events']
    now, index, inflight = 0.0, 0, None
    latest, done, requested, attempts, last_start = {}, set(), {}, {}, {}
    history, requests, emitted, peak_pending = (), [], [], 0
    while index < len(events) or inflight or set(latest)-done:
        next_event = events[index]['emission_s'] if index < len(events) else float('inf')
        completion = inflight['finish_s'] if inflight else float('inf')
        candidates = [s for gid, s in latest.items() if gid not in done]
        oldest = min(candidates, key=lambda s: (s['start_s'], s['group_id'])) if candidates else None
        wake = float('inf')
        if oldest and not inflight:
            gid, key = oldest['group_id'], signature(oldest)
            if oldest['source_text'] and (oldest['final'] or requested.get(gid) != key):
                wake = now if oldest['final'] else max(now, last_start.get(gid, -math.inf)+interval)
        at = min(next_event, completion, wake)
        if not math.isfinite(at):
            raise ValueError('consumer stalled without a new revision or final')
        now = at
        # Source snapshots win ties with model completion, preventing an obsolete
        # whole-group draft from resurfacing after a simultaneous split/readiness.
        while index < len(events) and events[index]['emission_s'] <= now:
            event = events[index]
            if event['kind'] == 'snapshot':
                if event['group_id'] in done:
                    raise AssertionError('producer mutated a completed group')
                latest[event['group_id']] = event
            index += 1
        peak_pending = max(peak_pending, len(set(latest)-done))
        if peak_pending > pending_limit:
            raise ValueError('reference consumer pending limit exceeded; no sources silently dropped')
        if inflight and inflight['finish_s'] <= now:
            job, inflight = inflight, None
            snap, gid = job['snapshot'], job['snapshot']['group_id']
            current = signature(latest[gid]) == signature(snap)
            job['accepted'] = current and not job['fails']
            if snap['final']:
                if not current:
                    raise AssertionError('immutable ready snapshot changed')
                if job['fails'] and attempts[gid] < 2:
                    pass
                else:
                    done.add(gid)
                    emitted.append(dict(kind='translation_failed' if job['fails'] else 'translation_final',
                                        emission_s=now, group_id=gid, source_ids=list(snap['source_revisions'])))
                    if not job['fails']:
                        history = (*history, (snap['source_text'], job['text']))
                        while len(history) > 8 or sum(len(a)+len(b) for a, b in history) > 2000:
                            history = history[1:]
            elif job['accepted']:
                emitted.append(dict(kind='translation_draft', emission_s=now, group_id=gid))
        if inflight:
            continue
        candidates = [s for gid, s in latest.items() if gid not in done]
        if not candidates:
            continue
        snap = min(candidates, key=lambda s: (s['start_s'], s['group_id']))
        gid, key = snap['group_id'], signature(snap)
        if not snap['source_text']:
            continue
        if not snap['final'] and (requested.get(gid) == key or now+1e-9 < last_start.get(gid, -math.inf)+interval):
            continue
        attempts[gid] = attempts.get(gid, 0)+1 if snap['final'] else 0
        seconds, fails = cost(snap, attempts[gid])
        if not math.isfinite(seconds) or seconds < 0:
            raise ValueError('invalid synthetic compute cost')
        text = 'synthetic-'+hashlib.sha256(snap['source_text'].encode()).hexdigest()[:12]
        job = dict(snapshot=snap, context=history, start_s=now, finish_s=now+seconds,
                   fails=fails, text=text, accepted=False)
        requests.append(job)
        requested[gid], last_start[gid], inflight = key, now, job
    coverage = [sid for e in emitted if e['kind'] != 'translation_draft' for sid in e['source_ids']]
    if len(coverage) != len(set(coverage)) or set(coverage) != set(plan['source_ids']):
        raise AssertionError('consumer coverage is not exactly once including explicit failures')
    return dict(requests=requests, events=emitted, peak_pending=peak_pending,
                final_history=history, finished_s=now,
                notes=['Synthetic virtual compute only, no model or semantic scores.',
                       'Changed draft snapshots rejected; frozen finals retry at most twice.',
                       'Pending limit is an experiment guard, not production backpressure.'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--events', type=Path, required=True)
    parser.add_argument('--duration', type=float, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--wait', type=float, default=8)
    args = parser.parse_args()
    events = json.loads(args.events.read_text())
    plan = compile_plan(events, args.duration, commit_wait=args.wait)
    results = {str(seconds): consume(plan, lambda _s, _a, value=seconds: (value, False))
               for seconds in (0, .5, 2, 6, 12)}
    with args.output.open('x') as stream:
        json.dump(dict(plan=plan, consumers=results,
                       source_sha256=hashlib.sha256(args.events.read_bytes()).hexdigest()),
                  stream, ensure_ascii=False, indent=2)


if __name__ == '__main__':
    main()
