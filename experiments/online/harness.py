#!/usr/bin/env python3
"""Online-only adapters and serial translation-policy replay; no workspace writes."""
import argparse
import hashlib
import json
import math
import os
import re
from pathlib import Path
import time
import urllib.parse
import urllib.request

SCHEMA = 'atogaki.online.v1'


def finite(value):
    value = float(value)
    if not math.isfinite(value) or value < 0:
        raise ValueError('timestamps must be finite and nonnegative')
    return value


def normalize(lines, backend):
    """Upstream output is appended confirmed text, not replaceable hypotheses."""
    previous = 0.0
    for number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        if backend == 'whisper-streaming':
            match = re.fullmatch(r'\s*(\S+)\s+(\S+)\s+(\S+)[ \t](.*)', line.rstrip('\n'))
            if match is None:
                raise ValueError(f'line {number}: expected emission/start/end/text')
            emission, start, end = (finite(x) / 1000 for x in match.groups()[:3])
            text, final = match.group(4), False
        else:
            raw = json.loads(line)
            emission = finite(raw['emission_time'])
            final = raw.get('is_final', False)
            if not isinstance(final, bool):
                raise ValueError('is_final must be boolean')
            text = raw.get('text', '')
            start = finite(raw['start']) if text else None
            end = finite(raw['end']) if text else None
        if not isinstance(text, str):
            raise ValueError('text must be a string')
        if emission < previous:
            raise ValueError('emission times must be monotonic')
        if text and end < start:
            raise ValueError('audio end precedes start')
        previous = emission
        if text:
            yield dict(schema=SCHEMA, kind='source_append', backend=backend,
                       emission_s=emission, start_s=start, end_s=end, text=text)
        if final:
            yield dict(schema=SCHEMA, kind='source_boundary', backend=backend,
                       emission_s=emission)


class HyMT2:
    def __init__(self, base_url, model, target, timeout=120):
        parsed = urllib.parse.urlparse(base_url)
        if parsed.scheme != 'http' or parsed.hostname not in ('127.0.0.1', 'localhost', '::1'):
            raise ValueError('online experiment requires a local HTTP llama-server')
        if parsed.username or parsed.password or parsed.query or parsed.fragment:
            raise ValueError('credentials/query/fragment are not allowed in base URL')
        self.endpoint = base_url.rstrip('/') + '/chat/completions'
        self.model, self.target, self.timeout = model, target, timeout
        # Do not inherit system proxies for private transcript requests.
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                                                NoRedirect())

    def translate(self, source):
        body = dict(model=self.model, stream=False, temperature=0.0, max_tokens=1024,
                    messages=[dict(role='user', content=(
                        f'Translate the following segment into {self.target}, without additional explanation.\n\n'
                        + source))])
        headers = {'Content-Type': 'application/json'}
        key = os.environ.get('ATOGAKI_ONLINE_LLAMA_KEY')
        if key:
            headers['Authorization'] = 'Bearer ' + key
        request = urllib.request.Request(self.endpoint, json.dumps(body).encode(), headers)
        with self.client.open(request, timeout=self.timeout) as response:
            raw = json.load(response)
        if raw['choices'][0].get('finish_reason') != 'stop':
            raise ValueError('translation was not completed normally')
        result = raw['choices'][0]['message']['content']
        if not isinstance(result, str) or not result.strip():
            raise ValueError('empty translation')
        return result.strip()


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('redirects are disabled for local translation')


def replay(events, translator, policy, interval, max_chars, clock=time.monotonic):
    """Serial policy experiment. ASR timestamps remain upstream observations."""
    text, revision, group, last_update = '', 0, 1, -math.inf
    source_end = 0.0

    def update(final, event_time):
        before = clock()
        result = translator.translate(text)
        duration = clock() - before
        return dict(schema=SCHEMA, kind='translation_replace', group_id=group,
                    source_revision=revision, text=result, source_text=text,
                    source_end_s=source_end, upstream_emission_s=event_time,
                    translation_compute_s=duration, final=final,
                    timing_mode='serial-policy-replay')

    last_emission = 0.0
    for event in events:
        if event.get('schema') != SCHEMA:
            raise ValueError('unsupported online event schema')
        event_time = finite(event['emission_s'])
        if event_time < last_emission:
            raise ValueError('event time moved backwards')
        last_emission = event_time
        kind = event['kind']
        if kind == 'source_append':
            chunk = event['text']
            if not isinstance(chunk, str) or not chunk:
                raise ValueError('source_append needs nonempty text')
            start, source_end = finite(event['start_s']), finite(event['end_s'])
            if source_end < start:
                raise ValueError('audio end precedes start')
            text += chunk
            revision += 1
            boundary = text.rstrip().endswith(('。', '！', '？', '.', '!', '?')) or len(text) >= max_chars
        elif kind == 'source_boundary':
            boundary = True
        else:
            raise ValueError(f'unsupported event kind: {kind}')
        if not text:
            continue
        if boundary:
            yield update(True, event_time)
            text, revision = '', 0
            group += 1
            last_update = -math.inf
        elif policy == 'revisable' and event_time - last_update >= interval:
            yield update(False, event_time)
            last_update = event_time
    # EOF flush is an explicit policy decision, not an upstream final signal.
    if text:
        yield update(True, last_emission)


def percentile(values, fraction):
    if not values:
        return None
    values = sorted(values)
    return values[max(0, math.ceil(len(values) * fraction) - 1)]


def report(events):
    latency, compute, groups = [], [], {}
    for event in events:
        if event['kind'] == 'source_append':
            # Timestamps are estimates from the provider, not gold alignment.
            latency.append(event['emission_s'] - event['end_s'])
        elif event['kind'] == 'translation_replace':
            compute.append(event['translation_compute_s'])
            groups.setdefault(event['group_id'], []).append(event)
    return dict(schema=SCHEMA, asr_updates=len(latency),
                asr_emission_minus_audio_end_s=dict(p50=percentile(latency, .5), p95=percentile(latency, .95)),
                translation_requests=len(compute), translation_compute_total_s=sum(compute),
                translation_compute_p95_s=percentile(compute, .95),
                translated_groups=len(groups),
                replacement_updates=sum(max(0, len(g) - 1) for g in groups.values()),
                timing_note='Serial translation replay cannot measure live pipeline latency or contention.')


def read_events(path):
    with path.open() as stream:
        for line in stream:
            if line.strip():
                yield json.loads(line)


def write_events(path, events):
    # Never silently overwrite evidence from another run.
    with path.open('x') as stream:
        for event in events:
            stream.write(json.dumps(event, ensure_ascii=False) + '\n')
            stream.flush()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    norm = sub.add_parser('normalize')
    norm.add_argument('--backend', choices=['whisper-streaming', 'simulstreaming'], required=True)
    for command in (norm,):
        command.add_argument('--input', type=Path, required=True)
        command.add_argument('--output', type=Path, required=True)
    trans = sub.add_parser('translate')
    trans.add_argument('--input', type=Path, required=True)
    trans.add_argument('--output', type=Path, required=True)
    trans.add_argument('--base-url', default='http://127.0.0.1:18080/v1')
    trans.add_argument('--model', required=True, help='loaded Hy-MT2 model ID')
    trans.add_argument('--target', default='Chinese')
    trans.add_argument('--policy', choices=['boundary', 'revisable'], default='revisable')
    trans.add_argument('--interval', type=float, default=5)
    trans.add_argument('--max-chars', type=int, default=240)
    stats = sub.add_parser('report')
    stats.add_argument('--input', type=Path, nargs='+', required=True)
    args = parser.parse_args()
    if args.command == 'normalize':
        with args.input.open() as stream:
            write_events(args.output, normalize(stream, args.backend))
    elif args.command == 'translate':
        if not math.isfinite(args.interval) or args.interval <= 0 or args.max_chars <= 0:
            parser.error('interval and max-chars must be positive')
        translator = HyMT2(args.base_url, args.model, args.target)
        write_events(args.output, replay(read_events(args.input), translator,
                                        args.policy, args.interval, args.max_chars))
        metadata = dict(schema=SCHEMA, backend='hy-mt2', model=args.model,
                        input_sha256=hashlib.sha256(args.input.read_bytes()).hexdigest(),
                        policy=args.policy, interval_s=args.interval, max_chars=args.max_chars,
                        target=args.target, timing_mode='serial-policy-replay')
        args.output.with_suffix(args.output.suffix + '.meta.json').write_text(
            json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
    else:
        print(json.dumps(report(event for path in args.input for event in read_events(path)),
                         ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
