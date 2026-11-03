"""Local text translation adapter; audio never leaves the worker."""
import json
import os
import urllib.parse
import urllib.request


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

    def translate(self, source, context=()):
        if context:
            # Hy-MT2's documented background/source template keeps reference
            # material before the instruction, outside the text to translate.
            background = '\n'.join(f'Source: {src}\nTranslation: {dst}' for src, dst in context)
            prompt = (f'[Background Information]\n{background}\n\n'
                      f'Please translate the following text into {self.target}, taking the '
                      'provided background information into consideration. Only output the '
                      'translated result without any additional explanation.\n\n'
                      f'[Source Text]\n{source}')
        else:
            prompt = (f'Translate the following segment into {self.target}, '
                      f'without additional explanation.\n\n{source}')
        body = dict(model=self.model, stream=False, temperature=0.0, max_tokens=1024,
                    messages=[dict(role='user', content=prompt)])
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
