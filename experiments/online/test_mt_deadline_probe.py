import json
import unittest
from unittest.mock import Mock, patch

import mt_deadline_probe


def delta(text):
    return b'data: ' + json.dumps({'choices': [{'delta': {'content': text}}]}).encode() + b'\n'


class DeadlineProbeTests(unittest.TestCase):
    def run_probe(self, lines, *, budget=1, stop=None, status=200):
        clock = [0.0]
        response = Mock(status=status)
        response.getheader.return_value = 'text/event-stream; charset=utf-8'

        def read():
            elapsed, result = lines.pop(0) if lines else (0, b'')
            clock[0] += elapsed
            if isinstance(result, Exception):
                raise result
            return result

        response.readline.side_effect = read
        connection = Mock()
        connection.getresponse.return_value = response
        with patch.object(mt_deadline_probe.http.client, 'HTTPConnection', return_value=connection), \
                patch.object(mt_deadline_probe.time, 'monotonic', side_effect=lambda: clock[0]):
            result = mt_deadline_probe.request('http://127.0.0.1:1234/v1', {'stream': True},
                                               budget_s=budget, stop_after_content=stop)
        connection.sock.shutdown.assert_called_once()
        response.close.assert_called_once()
        connection.close.assert_called_once()
        return result, connection

    def test_content_cancellation_is_not_success_or_a_partial_translation(self):
        result, _ = self.run_probe([(0.1, delta('一')), (0.1, delta('二'))], stop=1)
        self.assertEqual(result['status'], 'cancelled_after_content')
        self.assertEqual(result['content_events'], 1)
        self.assertNotIn('text', result)

    def test_heartbeats_do_not_reset_total_read_budget(self):
        result, connection = self.run_probe([(0.4, b': heartbeat\n'),
                                              (0.4, delta('一')), (0.4, delta('二'))])
        self.assertEqual(result['status'], 'deadline')
        self.assertEqual(result['content_events'], 1)
        self.assertAlmostEqual(connection.sock.settimeout.call_args.args[0], .2)

    def test_buffered_done_after_deadline_is_not_completed(self):
        result, _ = self.run_probe([(1.1, b'data: [DONE]\n')])
        self.assertEqual(result['status'], 'deadline')

    def test_socket_timeout_releases_connection(self):
        result, _ = self.run_probe([(0.5, TimeoutError('read stalled'))])
        self.assertEqual(result['status'], 'deadline')

    def test_eof_without_done_remains_incomplete(self):
        result, _ = self.run_probe([(0.1, delta('一'))])
        self.assertEqual(result['status'], 'unexpected_eof')

    def test_done_is_transport_completion_only(self):
        result, _ = self.run_probe([(0.1, delta('一')), (0.1, b'data: [DONE]\n')])
        self.assertEqual(result['status'], 'completed')

    def test_header_timeout_still_shuts_down_transport(self):
        connection = Mock()
        connection.getresponse.side_effect = TimeoutError('no headers yet')
        with patch.object(mt_deadline_probe.http.client, 'HTTPConnection', return_value=connection):
            result = mt_deadline_probe.request('http://127.0.0.1/v1', {'stream': True}, budget_s=1)
        self.assertEqual(result['status'], 'deadline')
        connection.sock.shutdown.assert_called_once()
        connection.close.assert_called_once()

    def test_http_error_is_not_followed_or_accepted_and_closes_response(self):
        connection = Mock()
        response = connection.getresponse.return_value
        response.status = 302
        with patch.object(mt_deadline_probe.http.client, 'HTTPConnection', return_value=connection), \
                self.assertRaisesRegex(ValueError, 'HTTP status 302'):
            mt_deadline_probe.request('http://127.0.0.1/v1', {'stream': True}, budget_s=1)
        response.readline.assert_not_called()
        response.close.assert_called_once()
        connection.sock.shutdown.assert_called_once()
        connection.close.assert_called_once()

    def test_rejects_nonlocal_or_credential_urls_before_connecting(self):
        for url in ('https://127.0.0.1/v1', 'http://example.com/v1',
                    'http://user:password@localhost/v1', 'http://localhost/v1?key=x'):
            with self.subTest(url=url), self.assertRaises(ValueError):
                mt_deadline_probe.request(url, {'stream': True}, budget_s=1)

    def test_rejects_invalid_budget_and_nonstream_requests(self):
        for budget in (0, -1, float('inf'), float('nan')):
            with self.subTest(budget=budget), self.assertRaises(ValueError):
                mt_deadline_probe.request('http://localhost/v1', {'stream': True}, budget_s=budget)
        with self.assertRaises(ValueError):
            mt_deadline_probe.request('http://localhost/v1', {'stream': False}, budget_s=1)


if __name__ == '__main__':
    unittest.main()
