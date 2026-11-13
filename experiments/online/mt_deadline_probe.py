"""Research-only loopback SSE transport probe; partial text is never a translation.

Closing a client is not proof of server cancellation. Callers must separately
observe the owned server's slot release and successful subsequent work.
"""
import http.client
import json
import math
import socket
import time
from urllib.parse import urlsplit


def request(base_url, payload, *, budget_s, stop_after_content=None):
    parsed = urlsplit(base_url)
    if (parsed.scheme != 'http' or parsed.hostname not in ('127.0.0.1', 'localhost', '::1')
            or parsed.username or parsed.password or parsed.query or parsed.fragment):
        raise ValueError('probe requires a credential-free loopback HTTP URL')
    if not math.isfinite(budget_s) or budget_s <= 0:
        raise ValueError('positive finite budget required')
    if stop_after_content is not None and (
            isinstance(stop_after_content, bool) or not isinstance(stop_after_content, int)
            or stop_after_content <= 0):
        raise ValueError('positive content-event limit required')
    if payload.get('stream') is not True:
        raise ValueError('probe requires stream=true')
    started = time.monotonic()
    deadline = started + budget_s
    connection = http.client.HTTPConnection(parsed.hostname, parsed.port, timeout=budget_s)
    response, transport = None, None
    records, content_events, first_content_s = [], 0, None
    status = 'unexpected_eof'
    try:
        connection.request('POST', parsed.path.rstrip('/') + '/chat/completions',
                           json.dumps(payload).encode(), {'Content-Type': 'application/json'})
        transport = connection.sock
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            status = 'deadline'
        else:
            transport.settimeout(remaining)
            response = connection.getresponse()
            if response.status != 200:
                raise ValueError(f'probe HTTP status {response.status}')
            if not response.getheader('Content-Type', '').startswith('text/event-stream'):
                raise ValueError('probe requires an SSE response')
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    status = 'deadline'
                    break
                transport.settimeout(remaining)
                line = response.readline()
                at = time.monotonic() - started
                # A response arriving after the total budget is not accepted,
                # even when the socket had buffered data or repeated heartbeats.
                if at >= budget_s:
                    status = 'deadline'
                    break
                if not line:
                    break
                if not line.startswith(b'data:'):
                    continue
                data = line[5:].strip()
                if data == b'[DONE]':
                    status = 'completed'
                    break
                record = json.loads(data)
                records.append(dict(elapsed_s=at, data=record))
                if any(c.get('delta', {}).get('content') for c in record.get('choices', [])):
                    content_events += 1
                    if first_content_s is None:
                        first_content_s = at
                    if stop_after_content is not None and content_events >= stop_after_content:
                        status = 'cancelled_after_content'
                        break
    except (TimeoutError, socket.timeout):
        status = 'deadline'
    finally:
        # Explicitly release both directions even if HTTPResponse still holds a
        # buffered file reference. No server-wide kill or shared-slot mutation.
        transport = transport or connection.sock
        if transport is not None:
            try:
                transport.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        if response is not None:
            response.close()
        connection.close()
    return dict(status=status, elapsed_s=time.monotonic()-started,
                budget_s=budget_s, first_content_s=first_content_s,
                content_events=content_events, records=records,
                note='Wire diagnostics only; no partial output is accepted as MT.')
